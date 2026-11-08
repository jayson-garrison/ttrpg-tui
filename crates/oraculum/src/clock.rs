//! The real clock. Core never reads the system time (A§6).

use oraculum_core::clock::Clock;

/// Reads `std::time::SystemTime`; a clock set before 1970 reads as 0.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_secs(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    }
}
