use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use hmac::{Hmac, Mac};
use ipnet::IpNet;
use sha2::Sha256;
use sqlx::MySqlPool;
use std::sync::atomic::{AtomicU64, Ordering};
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
    shared: Option<SharedPerimeter>,
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
            shared: None,
        })
    }
    /// The HMAC key must be a secret shared across all instances of this
    /// deployment. Rotating it starts fresh quota buckets.
    pub fn with_shared(
        mut self,
        pool: MySqlPool,
        key: [u8; 32],
        window_seconds: u32,
        limit: u32,
    ) -> Option<Self> {
        if key.iter().all(|byte| *byte == 0)
            || !(1..=3600).contains(&window_seconds)
            || limit == 0
            || limit > 1_000_000
        {
            return None;
        }
        self.shared = Some(SharedPerimeter {
            pool,
            key,
            window_seconds,
            limit,
            ticks: Arc::new(AtomicU64::new(0)),
        });
        Some(self)
    }
    pub(crate) fn has_shared_limit(&self) -> bool {
        self.shared.is_some()
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
#[derive(Clone)]
struct SharedPerimeter {
    pool: MySqlPool,
    key: [u8; 32],
    window_seconds: u32,
    limit: u32,
    ticks: Arc<AtomicU64>,
}
impl SharedPerimeter {
    async fn admit(&self, ip: IpAddr) -> Result<bool, sqlx::Error> {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.key).expect("HMAC accepts 32-byte keys");
        mac.update(b"bastion/mcp/perimeter-ip/v1\0");
        mac.update(ip.to_string().as_bytes());
        let hash = mac.finalize().into_bytes();
        // Bounded incremental cleanup of old buckets. Keeping at least two
        // hours avoids deleting an active bucket for any supported window.
        if self.ticks.fetch_add(1, Ordering::Relaxed) % 128 == 0 {
            sqlx::query("DELETE FROM oauth_perimeter_quotas WHERE window_start < UTC_TIMESTAMP(6)-INTERVAL 2 HOUR LIMIT 256")
                .execute(&self.pool).await?;
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO oauth_perimeter_quotas (client_hash,window_start,requests) VALUES (?,UTC_TIMESTAMP(6),1) ON DUPLICATE KEY UPDATE requests=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL ? SECOND,1,LEAST(requests+1,?)),window_start=IF(window_start<=UTC_TIMESTAMP(6)-INTERVAL ? SECOND,UTC_TIMESTAMP(6),window_start)")
            .bind(hash.as_slice()).bind(self.window_seconds).bind(self.limit + 1).bind(self.window_seconds).execute(&mut *tx).await?;
        let count: u32 = sqlx::query_scalar(
            "SELECT requests FROM oauth_perimeter_quotas WHERE client_hash=? FOR UPDATE",
        )
        .bind(hash.as_slice())
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(count <= self.limit)
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
    let ip = ip.to_canonical();
    if !guard.admit(ip) {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    if let Some(shared) = &guard.shared {
        match shared.admit(ip).await {
            Ok(true) => {}
            Ok(false) => return StatusCode::TOO_MANY_REQUESTS.into_response(),
            Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        }
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
