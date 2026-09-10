//! Virtual time. The core never reads a clock; drivers supply `Instant`s.

use core::fmt;
use core::ops::{Add, AddAssign, Mul, Sub};

use serde::{Deserialize, Serialize};

/// A point in time, microseconds since the driver's epoch.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
pub struct Instant(pub u64);

/// A span of time in microseconds.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
pub struct Duration(pub u64);

impl Instant {
    /// The epoch.
    pub const ZERO: Instant = Instant(0);
    /// Microseconds since epoch.
    pub fn as_micros(self) -> u64 {
        self.0
    }
    /// Seconds as f64.
    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 / 1e6
    }
    /// `self − earlier`, saturating.
    pub fn duration_since(self, earlier: Instant) -> Duration {
        Duration(self.0.saturating_sub(earlier.0))
    }
    /// Checked addition.
    pub fn checked_add(self, d: Duration) -> Option<Instant> {
        self.0.checked_add(d.0).map(Instant)
    }
}

impl Duration {
    /// Zero.
    pub const ZERO: Duration = Duration(0);
    /// From milliseconds.
    pub const fn from_millis(ms: u64) -> Self {
        Duration(ms * 1000)
    }
    /// From seconds.
    pub const fn from_secs(s: u64) -> Self {
        Duration(s * 1_000_000)
    }
    /// From microseconds.
    pub const fn from_micros(us: u64) -> Self {
        Duration(us)
    }
    /// From fractional seconds.
    pub fn from_secs_f64(s: f64) -> Self {
        Duration((s * 1e6).round().max(0.0) as u64)
    }
    /// Microseconds.
    pub fn as_micros(self) -> u64 {
        self.0
    }
    /// Milliseconds (truncating).
    pub fn as_millis(self) -> u64 {
        self.0 / 1000
    }
    /// Milliseconds as f64.
    pub fn as_millis_f64(self) -> f64 {
        self.0 as f64 / 1000.0
    }
    /// Seconds as f64.
    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 / 1e6
    }
    /// Clamp.
    pub fn clamp(self, lo: Duration, hi: Duration) -> Duration {
        Duration(self.0.clamp(lo.0, hi.0))
    }
    /// Scale by a float.
    pub fn mul_f64(self, f: f64) -> Duration {
        Duration((self.0 as f64 * f).round().max(0.0) as u64)
    }
    /// Integer division.
    pub fn div_by(self, n: u64) -> Duration {
        Duration(self.0 / n.max(1))
    }
}

impl Add<Duration> for Instant {
    type Output = Instant;
    fn add(self, d: Duration) -> Instant {
        Instant(self.0.saturating_add(d.0))
    }
}
impl AddAssign<Duration> for Instant {
    fn add_assign(&mut self, d: Duration) {
        self.0 = self.0.saturating_add(d.0);
    }
}
impl Sub<Instant> for Instant {
    type Output = Duration;
    fn sub(self, o: Instant) -> Duration {
        self.duration_since(o)
    }
}
impl Add for Duration {
    type Output = Duration;
    fn add(self, o: Duration) -> Duration {
        Duration(self.0.saturating_add(o.0))
    }
}
impl Sub for Duration {
    type Output = Duration;
    fn sub(self, o: Duration) -> Duration {
        Duration(self.0.saturating_sub(o.0))
    }
}
impl Mul<u64> for Duration {
    type Output = Duration;
    fn mul(self, n: u64) -> Duration {
        Duration(self.0.saturating_mul(n))
    }
}
impl fmt::Display for Instant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.3}s", self.as_secs_f64())
    }
}
impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 >= 1_000_000 {
            write!(f, "{:.3}s", self.as_secs_f64())
        } else {
            write!(f, "{:.1}ms", self.as_millis_f64())
        }
    }
}
