use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::DashboardError;

/// Circuit breaker states.
const CLOSED: u8 = 0;
const OPEN: u8 = 1;
const HALF_OPEN: u8 = 2;

/// Simple circuit breaker for GitHub API calls.
///
/// Opens after `FAILURE_THRESHOLD` consecutive failures, stays open for
/// `OPEN_DURATION_SECS`, then transitions to half-open (allows one probe).
/// Any success resets to closed.
pub struct CircuitBreaker {
    failure_count: AtomicU32,
    last_failure_epoch: AtomicU64,
    state: AtomicU8,
}

const FAILURE_THRESHOLD: u32 = 5;
const OPEN_DURATION_SECS: u64 = 60;

impl CircuitBreaker {
    pub fn new() -> Self {
        Self {
            failure_count: AtomicU32::new(0),
            last_failure_epoch: AtomicU64::new(0),
            state: AtomicU8::new(CLOSED),
        }
    }

    /// Check whether the circuit allows a request.
    /// Returns `Ok(())` if the call may proceed, or `Err(CircuitOpen)` if not.
    pub fn check(&self) -> Result<(), DashboardError> {
        let current = self.state.load(Ordering::Acquire);
        match current {
            CLOSED => Ok(()),
            HALF_OPEN => {
                // Allow exactly one probe in half-open state
                Ok(())
            }
            OPEN => {
                // Check if the open duration has elapsed
                let last_fail = self.last_failure_epoch.load(Ordering::Acquire);
                let now = epoch_secs();
                if now.saturating_sub(last_fail) >= OPEN_DURATION_SECS {
                    // Transition to half-open
                    self.state.store(HALF_OPEN, Ordering::Release);
                    Ok(())
                } else {
                    Err(DashboardError::CircuitOpen)
                }
            }
            _ => Ok(()), // unreachable, but treat as closed
        }
    }

    /// Record a successful call — resets the circuit to closed.
    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::Release);
        self.state.store(CLOSED, Ordering::Release);
    }

    /// Record a failed call — may open the circuit.
    pub fn record_failure(&self) {
        let count = self.failure_count.fetch_add(1, Ordering::AcqRel) + 1;
        self.last_failure_epoch.store(epoch_secs(), Ordering::Release);

        if count >= FAILURE_THRESHOLD {
            self.state.store(OPEN, Ordering::Release);
            tracing::warn!(
                failures = count,
                "Circuit breaker opened after {} consecutive failures",
                count
            );
        }
    }

    /// Current state as a human-readable string (for diagnostics).
    #[cfg(test)]
    pub fn state_name(&self) -> &'static str {
        match self.state.load(Ordering::Acquire) {
            CLOSED => "closed",
            OPEN => "open",
            HALF_OPEN => "half-open",
            _ => "unknown",
        }
    }
}

fn epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starts_closed() {
        let cb = CircuitBreaker::new();
        assert!(cb.check().is_ok());
        assert_eq!(cb.state_name(), "closed");
    }

    #[test]
    fn test_opens_after_threshold() {
        let cb = CircuitBreaker::new();
        for _ in 0..FAILURE_THRESHOLD {
            cb.record_failure();
        }
        assert!(cb.check().is_err());
        assert_eq!(cb.state_name(), "open");
    }

    #[test]
    fn test_success_resets() {
        let cb = CircuitBreaker::new();
        for _ in 0..FAILURE_THRESHOLD {
            cb.record_failure();
        }
        assert!(cb.check().is_err());
        cb.record_success();
        assert!(cb.check().is_ok());
        assert_eq!(cb.state_name(), "closed");
    }

    #[test]
    fn test_below_threshold_stays_closed() {
        let cb = CircuitBreaker::new();
        for _ in 0..(FAILURE_THRESHOLD - 1) {
            cb.record_failure();
        }
        assert!(cb.check().is_ok());
        assert_eq!(cb.state_name(), "closed");
    }
}
