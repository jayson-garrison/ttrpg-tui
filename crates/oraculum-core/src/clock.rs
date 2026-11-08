//! Time source. Core never reads the system clock; the binary supplies one.

/// Unix epoch seconds, UTC.
pub trait Clock {
    fn now_secs(&self) -> u64;
}

/// A clock that always returns the same instant. For tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedClock(pub u64);

impl Clock for FixedClock {
    fn now_secs(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_clock_returns_its_value() {
        let c = FixedClock(1_700_000_000);
        assert_eq!(c.now_secs(), 1_700_000_000);
        assert_eq!(c.now_secs(), 1_700_000_000);
    }

    #[test]
    fn usable_as_trait_object() {
        let c: &dyn Clock = &FixedClock(7);
        assert_eq!(c.now_secs(), 7);
    }
}
