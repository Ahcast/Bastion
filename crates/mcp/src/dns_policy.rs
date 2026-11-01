use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use url::{Host, Url};

#[derive(Debug, PartialEq, Eq)]
pub struct DnsPolicyError;

/// Candidate addresses for a future SSRF-hardened client metadata fetch.
/// This value alone performs no network I/O and is not sufficient to fetch
/// safely: the transport must pin one of these addresses, preserve the DNS
/// name for TLS, reject redirects/proxies and enforce time and byte limits.
pub struct PublicDnsCandidates {
    hostname: String,
    addresses: Vec<SocketAddr>,
}

impl PublicDnsCandidates {
    pub fn hostname(&self) -> &str {
        &self.hostname
    }
    pub fn addresses(&self) -> &[SocketAddr] {
        &self.addresses
    }
}

/// Conservative initial profile: a DNS hostname over standard HTTPS, with
/// only public IPv4 answers. Mixed public/private or IPv6 answer sets fail
/// closed; this restriction must be documented in compatibility testing.
pub fn validate_public_dns_candidates(
    client_id: &str,
    addresses: &[SocketAddr],
) -> Result<PublicDnsCandidates, DnsPolicyError> {
    crate::metadata::validate_client_identifier(client_id).map_err(|_| DnsPolicyError)?;
    let url = Url::parse(client_id).map_err(|_| DnsPolicyError)?;
    let Some(Host::Domain(hostname)) = url.host() else {
        return Err(DnsPolicyError);
    };
    if hostname.eq_ignore_ascii_case("localhost")
        || hostname.ends_with(".localhost")
        || url.port_or_known_default() != Some(443)
        || addresses.is_empty()
        || addresses.len() > 16
    {
        return Err(DnsPolicyError);
    }
    if addresses.iter().any(|socket| {
        socket.port() != 443 || !matches!(socket.ip(), IpAddr::V4(ip) if public_ipv4(ip))
    }) {
        return Err(DnsPolicyError);
    }
    Ok(PublicDnsCandidates {
        hostname: hostname.to_owned(),
        addresses: addresses.to_vec(),
    })
}

fn public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    match (a, b, c) {
        (0, _, _) | (10, _, _) | (127, _, _) | (224..=255, _, _) => false,
        (100, 64..=127, _) | (169, 254, _) | (172, 16..=31, _) | (192, 168, _) => false,
        (192, 0, 0)
        | (192, 0, 2)
        | (192, 88, 99)
        | (198, 18..=19, _)
        | (198, 51, 100)
        | (203, 0, 113) => false,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_private_special_and_ambiguous_resolution() {
        let id = "https://client.example/metadata.json";
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.169.254",
            "172.16.1.1",
            "192.168.1.1",
            "100.64.0.1",
            "192.0.2.1",
            "198.18.0.1",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
        ] {
            let addr: SocketAddr = format!("{ip}:443").parse().unwrap();
            assert!(validate_public_dns_candidates(id, &[addr]).is_err(), "{ip}");
        }
        let public: SocketAddr = "8.8.8.8:443".parse().unwrap();
        let private: SocketAddr = "10.0.0.1:443".parse().unwrap();
        let ipv6: SocketAddr = "[::1]:443".parse().unwrap();
        assert!(validate_public_dns_candidates(id, &[]).is_err());
        assert!(validate_public_dns_candidates(id, &[public, private]).is_err());
        assert!(validate_public_dns_candidates(id, &[ipv6]).is_err());
        assert!(validate_public_dns_candidates(id, &["8.8.8.8:444".parse().unwrap()]).is_err());
        assert!(validate_public_dns_candidates("https://127.0.0.1/doc", &[public]).is_err());
        assert!(validate_public_dns_candidates("https://localhost/doc", &[public]).is_err());
        let candidates = validate_public_dns_candidates(id, &[public]).unwrap();
        assert_eq!(candidates.hostname(), "client.example");
        assert_eq!(candidates.addresses(), &[public]);
    }
}
