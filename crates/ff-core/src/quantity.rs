//! Integer resource quantities.
//!
//! Kubernetes expresses CPU in cores with a milli suffix and memory in bytes
//! with binary suffixes. Both are representable exactly as integers, and both
//! feed a content hash that must agree across architectures, so neither is ever
//! stored as a float.

use std::fmt;

use serde::{Deserialize, Serialize};

/// CPU, in thousandths of a core. `1000` is one core.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Millicores(pub i64);

impl Millicores {
    /// Zero CPU.
    pub const ZERO: Self = Self(0);

    /// Construct from whole cores.
    #[must_use]
    pub const fn from_cores(cores: i64) -> Self {
        Self(cores.saturating_mul(1000))
    }

    /// Saturating addition. Aggregating a cluster's worth of requests must not
    /// wrap, and a panic in an analyzer is not an acceptable failure mode.
    #[must_use]
    pub const fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    /// Saturating subtraction.
    #[must_use]
    pub const fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }
}

impl fmt::Display for Millicores {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}m", self.0)
    }
}

/// Memory or storage, in bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Bytes(pub i64);

impl Bytes {
    /// Zero bytes.
    pub const ZERO: Self = Self(0);

    /// Construct from mebibytes.
    #[must_use]
    pub const fn from_mib(mib: i64) -> Self {
        Self(mib.saturating_mul(1024 * 1024))
    }

    /// Construct from gibibytes.
    #[must_use]
    pub const fn from_gib(gib: i64) -> Self {
        Self(gib.saturating_mul(1024 * 1024 * 1024))
    }

    /// Saturating addition.
    #[must_use]
    pub const fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    /// Saturating subtraction.
    #[must_use]
    pub const fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }
}

impl fmt::Display for Bytes {
    /// Renders with a binary suffix, truncating rather than rounding, so the
    /// displayed value never overstates available capacity.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const GIB: i64 = 1024 * 1024 * 1024;
        const MIB: i64 = 1024 * 1024;
        const KIB: i64 = 1024;
        match self.0 {
            b if b.abs() >= GIB => write!(f, "{}Gi", b / GIB),
            b if b.abs() >= MIB => write!(f, "{}Mi", b / MIB),
            b if b.abs() >= KIB => write!(f, "{}Ki", b / KIB),
            b => write!(f, "{b}"),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn millicores_display_uses_kubernetes_suffix() {
        assert_eq!(Millicores::from_cores(2).to_string(), "2000m");
        assert_eq!(Millicores(250).to_string(), "250m");
    }

    #[test]
    fn bytes_display_truncates_and_never_overstates() {
        // 1.9 GiB must read as 1Gi, never 2Gi: an operator reading capacity
        // headroom should be under-promised, not over-promised.
        assert_eq!(Bytes(GIB_TEST + GIB_TEST / 2).to_string(), "1Gi");
        assert_eq!(Bytes::from_mib(512).to_string(), "512Mi");
        assert_eq!(Bytes(999).to_string(), "999");
    }
    const GIB_TEST: i64 = 1024 * 1024 * 1024;

    #[test]
    fn arithmetic_saturates_instead_of_wrapping() {
        assert_eq!(
            Millicores(i64::MAX).saturating_add(Millicores(1)).0,
            i64::MAX
        );
        assert_eq!(Bytes::ZERO.saturating_sub(Bytes(5)).0, -5);
    }
}
