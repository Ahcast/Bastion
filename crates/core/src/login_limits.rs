use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::CoreError;

const WINDOW: Duration = Duration::from_secs(15 * 60);
const ATTEMPTS: u32 = 30;
const MAX_IPS: usize = 4096;

/// Per-process admission control, shared by CoreService clones. No database work
/// or attacker-controlled unbounded allocation is needed to reject a request.
#[derive(Default)]
pub(crate) struct LoginLimits(Mutex<HashMap<IpAddr, (Instant, u32)>>);

impl LoginLimits {
    pub(crate) fn admit(&self, ip: &str) -> Result<(), CoreError> {
        self.admit_at(ip.parse().map_err(|_| CoreError::Invalid)?, Instant::now())
    }

    fn admit_at(&self, ip: IpAddr, now: Instant) -> Result<(), CoreError> {
        let mut entries = self.0.lock().map_err(|_| CoreError::Overloaded)?;
        if !entries.contains_key(&ip) && entries.len() >= MAX_IPS {
            entries.retain(|_, (start, _)| now.duration_since(*start) < WINDOW);
            if entries.len() >= MAX_IPS {
                return Err(CoreError::Overloaded);
            }
        }
        let (start, count) = entries.entry(ip).or_insert((now, 0));
        if now.duration_since(*start) >= WINDOW {
            *start = now;
            *count = 0;
        }
        if *count >= ATTEMPTS {
            return Err(CoreError::Locked);
        }
        *count += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_attempts_without_extending_the_window_or_blocking_other_ips() {
        let limits = LoginLimits::default();
        let now = Instant::now();
        let ip = "192.0.2.1".parse().unwrap();
        for _ in 0..ATTEMPTS {
            limits.admit_at(ip, now).unwrap();
        }
        assert!(matches!(
            limits.admit_at(ip, now + WINDOW / 2),
            Err(CoreError::Locked)
        ));
        limits.admit_at("192.0.2.2".parse().unwrap(), now).unwrap();
        limits.admit_at(ip, now + WINDOW).unwrap();
    }

    #[test]
    fn bounded_storage_fails_closed_and_reclaims_expired_entries() {
        let limits = LoginLimits::default();
        let now = Instant::now();
        for n in 0..MAX_IPS {
            limits
                .admit_at(IpAddr::from((n as u32).to_be_bytes()), now)
                .unwrap();
        }
        let extra = "203.0.113.1".parse().unwrap();
        assert!(matches!(
            limits.admit_at(extra, now),
            Err(CoreError::Overloaded)
        ));
        limits.admit_at(extra, now + WINDOW).unwrap();
        assert_eq!(limits.0.lock().unwrap().len(), 1);
    }
}
