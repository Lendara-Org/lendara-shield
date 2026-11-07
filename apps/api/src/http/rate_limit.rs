//! Per-client rate limit for the public endpoints that cost Shield something:
//! onboarding (an on-chain ASP write paid by the operator) and intent creation
//! (a deposit opened at the anchor). Fixed window per client IP, in memory.
//!
//! The client is the TCP peer. Behind a reverse proxy every request shares
//! the proxy's address, so the limit becomes global; `X-Forwarded-For` is
//! not trusted because any client can set it.

use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::error::ApiError;

/// Above this many tracked clients, expired windows are dropped.
const PRUNE_ABOVE: usize = 10_000;

/// Requests allowed per client and window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimit {
    pub requests: u32,
    pub window: Duration,
}

/// Shared limiter state; `None` limit lets everything through.
#[derive(Debug, Clone, Default)]
pub struct RateLimiter {
    limit: Option<RateLimit>,
    windows: Arc<Mutex<HashMap<IpAddr, (Instant, u32)>>>,
}

impl RateLimiter {
    #[must_use]
    pub fn new(limit: Option<RateLimit>) -> Self {
        Self {
            limit,
            windows: Arc::default(),
        }
    }

    /// Counts one request from `client` at `now`; `Err` with the seconds
    /// until its window resets when it is over the limit.
    fn check(&self, client: IpAddr, now: Instant) -> Result<(), u64> {
        let Some(limit) = self.limit else {
            return Ok(());
        };
        let mut windows = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
        if windows.len() > PRUNE_ABOVE {
            windows.retain(|_, (start, _)| now.duration_since(*start) < limit.window);
        }
        let (start, count) = windows.entry(client).or_insert((now, 0));
        if now.duration_since(*start) >= limit.window {
            (*start, *count) = (now, 0);
        }
        if *count >= limit.requests {
            let left = limit.window.saturating_sub(now.duration_since(*start));
            return Err(left.as_secs().max(1));
        }
        *count += 1;
        Ok(())
    }
}

/// Middleware: answers `429 rate_limited` once the client is over its limit.
pub async fn limit(State(limiter): State<RateLimiter>, request: Request, next: Next) -> Response {
    // No peer address (in-process tests): one shared bucket.
    let client = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED), |info| info.0.ip());
    match limiter.check(client, Instant::now()) {
        Ok(()) => next.run(request).await,
        Err(retry_after_secs) => ApiError::TooManyRequests { retry_after_secs }.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: Duration = Duration::from_secs(60);

    fn ip(last: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(203, 0, 113, last))
    }

    #[test]
    fn refuses_a_client_over_its_limit_until_the_window_resets() {
        let limiter = RateLimiter::new(Some(RateLimit {
            requests: 2,
            window: MINUTE,
        }));
        let t0 = Instant::now();
        assert_eq!(limiter.check(ip(1), t0), Ok(()));
        assert_eq!(limiter.check(ip(1), t0), Ok(()));
        assert_eq!(limiter.check(ip(1), t0 + Duration::from_secs(20)), Err(40));
        // Other clients have their own window.
        assert_eq!(limiter.check(ip(2), t0), Ok(()));
        assert_eq!(limiter.check(ip(1), t0 + MINUTE), Ok(()));
    }

    #[test]
    fn retry_after_is_never_zero() {
        let limiter = RateLimiter::new(Some(RateLimit {
            requests: 1,
            window: MINUTE,
        }));
        let t0 = Instant::now();
        assert_eq!(limiter.check(ip(1), t0), Ok(()));
        assert_eq!(
            limiter.check(ip(1), t0 + Duration::from_millis(59_999)),
            Err(1)
        );
    }

    #[test]
    fn no_limit_lets_everything_through() {
        let limiter = RateLimiter::new(None);
        let t0 = Instant::now();
        for _ in 0..1_000 {
            assert_eq!(limiter.check(ip(1), t0), Ok(()));
        }
    }

    #[test]
    fn expired_windows_are_pruned() {
        let limiter = RateLimiter::new(Some(RateLimit {
            requests: 1,
            window: MINUTE,
        }));
        let t0 = Instant::now();
        for n in 0..=PRUNE_ABOVE {
            let addr = IpAddr::V6(std::net::Ipv6Addr::from(u128::try_from(n).unwrap_or(0)));
            assert_eq!(limiter.check(addr, t0), Ok(()));
        }
        assert_eq!(limiter.check(ip(1), t0 + MINUTE), Ok(()));
        let tracked = limiter
            .windows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len();
        assert_eq!(tracked, 1, "only the live window is kept");
    }
}
