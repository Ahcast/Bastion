use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use ipnet::IpNet;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Process-local defense in depth. A shared ingress limit is still required
/// across replicas, and the proxy list must only contain trusted ingress hops.
#[derive(Clone)]
pub struct DraftPerimeter {
    proxies: Arc<Vec<IpNet>>,
    window: Duration,
    limit: u32,
    capacity: usize,
    entries: Arc<Mutex<HashMap<IpAddr, (Instant, u32)>>>,
}
impl DraftPerimeter {
    pub fn new(proxies: Vec<IpNet>, window: Duration, limit: u32, capacity: usize) -> Option<Self> {
        if window.is_zero()
            || window > Duration::from_secs(3600)
            || limit == 0
            || capacity == 0
            || capacity > 100_000
        {
            return None;
        }
        Some(Self {
            proxies: Arc::new(proxies),
            window,
            limit,
            capacity,
            entries: Arc::new(Mutex::new(HashMap::new())),
        })
    }
    fn client_ip(&self, request: &Request) -> Option<IpAddr> {
        let peer = request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()?
            .0
            .ip();
        let trusted = |ip: &IpAddr| self.proxies.iter().any(|net| net.contains(ip));
        if !trusted(&peer) {
            return Some(peer);
        }
        let mut headers = request.headers().get_all("x-forwarded-for").iter();
        let Some(value) = headers.next() else {
            return Some(peer);
        };
        if headers.next().is_some() || value.as_bytes().len() > 1024 {
            return None;
        }
        let chain = value
            .to_str()
            .ok()?
            .split(',')
            .take(17)
            .map(|s| s.trim().parse::<IpAddr>())
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        if chain.len() > 16 {
            return None;
        }
        let mut client = peer;
        for ip in chain.into_iter().rev() {
            if !trusted(&client) {
                break;
            }
            client = ip;
        }
        Some(client)
    }
    fn admit(&self, client: IpAddr) -> bool {
        let now = Instant::now();
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some((start, count)) = entries.get_mut(&client) {
            if now.duration_since(*start) >= self.window {
                *start = now;
                *count = 1;
                return true;
            }
            if *count >= self.limit {
                return false;
            }
            *count += 1;
            return true;
        }
        if entries.len() >= self.capacity {
            entries.retain(|_, (start, _)| now.duration_since(*start) < self.window);
            if entries.len() >= self.capacity {
                return false;
            }
        }
        entries.insert(client, (now, 1));
        true
    }
}
pub async fn perimeter_guard(
    State(guard): State<DraftPerimeter>,
    request: Request,
    next: Next,
) -> Response {
    let Some(ip) = guard.client_ip(&request) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if !guard.admit(ip) {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    fn request(peer: &str, forwarded: Option<&str>) -> Request<Body> {
        let mut r = Request::new(Body::empty());
        r.extensions_mut()
            .insert(ConnectInfo(SocketAddr::new(peer.parse().unwrap(), 1000)));
        if let Some(value) = forwarded {
            r.headers_mut()
                .insert("x-forwarded-for", value.parse().unwrap());
        }
        r
    }
    #[test]
    fn only_trusted_proxies_can_supply_the_client_address() {
        let guard = DraftPerimeter::new(
            vec!["10.0.0.0/8".parse().unwrap()],
            Duration::from_secs(60),
            2,
            2,
        )
        .unwrap();
        assert_eq!(
            guard
                .client_ip(&request("203.0.113.9", Some("192.0.2.1")))
                .unwrap()
                .to_string(),
            "203.0.113.9"
        );
        assert_eq!(
            guard
                .client_ip(&request(
                    "10.0.0.1",
                    Some("192.0.2.50, 203.0.113.9, 10.0.0.2")
                ))
                .unwrap()
                .to_string(),
            "203.0.113.9"
        );
        assert!(
            guard
                .client_ip(&request("10.0.0.1", Some("invalid")))
                .is_none()
        );
        let mut duplicate = request("10.0.0.1", Some("192.0.2.1"));
        duplicate
            .headers_mut()
            .append("x-forwarded-for", "192.0.2.2".parse().unwrap());
        assert!(guard.client_ip(&duplicate).is_none());
        assert!(guard.client_ip(&Request::new(Body::empty())).is_none());
    }
    #[tokio::test]
    async fn middleware_enforces_limits_and_requires_connection_info() {
        use axum::{Router, routing::get};
        use tower::ServiceExt;
        let guard = DraftPerimeter::new(vec![], Duration::from_millis(30), 1, 4).unwrap();
        let router = Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(guard, perimeter_guard));
        assert_eq!(
            router
                .clone()
                .oneshot(Request::new(Body::empty()))
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
        let make = || request("192.0.2.1", Some("203.0.113.1"));
        assert_eq!(
            router.clone().oneshot(make()).await.unwrap().status(),
            StatusCode::OK
        );
        assert_eq!(
            router.clone().oneshot(make()).await.unwrap().status(),
            StatusCode::TOO_MANY_REQUESTS
        );
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert_eq!(
            router.oneshot(make()).await.unwrap().status(),
            StatusCode::OK
        );
    }
    #[test]
    fn quotas_and_capacity_fail_closed() {
        let guard = DraftPerimeter::new(vec![], Duration::from_secs(60), 2, 1).unwrap();
        let a = "192.0.2.1".parse().unwrap();
        assert!(guard.admit(a));
        assert!(guard.admit(a));
        assert!(!guard.admit(a));
        assert!(!guard.admit("192.0.2.2".parse().unwrap()));
    }
}
