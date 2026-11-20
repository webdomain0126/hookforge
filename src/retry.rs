use std::time::Duration;

use crate::delivery::DeliveryStatus;

/// After this many failed attempts, we give up.
pub const MAX_ATTEMPTS: i32 = 5;

/// How long to wait after a failed attempt.
/// 30s, 60s, 120s, 240s... but never more than 1 hour.
pub fn backoff(attempt: i32) -> Duration {
    let doublings = (attempt - 1).clamp(0, 20) as u32;
    let seconds = 30u64.saturating_mul(1u64 << doublings);
    Duration::from_secs(seconds.min(3600))
}

/// What status a delivery gets after a failed attempt.
pub fn after_failure(attempts: i32) -> DeliveryStatus {
    if attempts >= MAX_ATTEMPTS {
        DeliveryStatus::Dead
    } else {
        DeliveryStatus::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles() {
        assert_eq!(backoff(1), Duration::from_secs(30));
        assert_eq!(backoff(2), Duration::from_secs(60));
        assert_eq!(backoff(3), Duration::from_secs(120));
    }

    #[test]
    fn backoff_is_capped_at_one_hour() {
        assert_eq!(backoff(50), Duration::from_secs(3600));
    }

    #[test]
    fn early_failure_retries() {
        assert_eq!(after_failure(1), DeliveryStatus::Pending);
    }

    #[test]
    fn fifth_failure_is_dead() {
        assert_eq!(after_failure(5), DeliveryStatus::Dead);
    }

    #[test]
    fn fourth_failure_still_retries() {
        assert_eq!(after_failure(4), DeliveryStatus::Pending);
    }
}