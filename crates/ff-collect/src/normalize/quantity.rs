//! Parsing Kubernetes resource quantities into integers.
//!
//! Kubernetes writes CPU and memory as strings: `100m`, `0.5`, `2`, `128Mi`,
//! `1Gi`, `1e3`. All of them are exactly representable as integers, and this
//! module never reaches for a float — `clippy::float_arithmetic` is denied
//! workspace-wide because snapshot identity is a content hash that must agree
//! across architectures.
//!
//! Parsing `0.5` as 500 millicores without floating point means doing the
//! decimal shift by hand. That is the bulk of the code below.
//!
//! One genuine ambiguity in the format: `E` is the exa suffix, while `e` starts
//! an exponent. `1E` is 10^18; `1e3` is 1000. Both are handled, and both are
//! tested.

use ff_core::{Bytes, Millicores};

/// Why a quantity could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantityParseError {
    /// The input that failed.
    pub input: String,
    /// What was wrong with it.
    pub reason: &'static str,
}

/// A parsed quantity: a scaled integer.
///
/// Held as `i128` during parsing because `1E` is 10^18, which already fills
/// most of an `i64`, and an intermediate step could overflow before the final
/// saturating conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Scaled {
    /// Value in units of 10^-3 of the base unit, so integer CPU maths is exact.
    milli_units: i128,
}

/// Parse a Kubernetes quantity into thousandths of the base unit.
fn parse_scaled(input: &str) -> Result<Scaled, QuantityParseError> {
    let err = |reason: &'static str| QuantityParseError {
        input: input.to_owned(),
        reason,
    };

    let s = input.trim();
    if s.is_empty() {
        return Err(err("empty quantity"));
    }

    // Split the numeric part from the suffix.
    let split_at = s
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+'))
        .map_or(s.len(), |(i, _)| i);
    let (number, suffix) = s.split_at(split_at);
    if number.is_empty() {
        return Err(err("no numeric part"));
    }

    // Decimal digits, kept as an integer plus a decimal shift.
    let negative = number.starts_with('-');
    let digits = number.trim_start_matches(['-', '+']);
    let (int_part, frac_part) = match digits.split_once('.') {
        Some((i, f)) => (i, f),
        None => (digits, ""),
    };
    if int_part.is_empty() && frac_part.is_empty() {
        return Err(err("no digits"));
    }
    if !int_part.chars().all(|c| c.is_ascii_digit())
        || !frac_part.chars().all(|c| c.is_ascii_digit())
    {
        return Err(err("non-numeric character in quantity"));
    }

    let mut value: i128 = 0;
    for c in int_part.chars().chain(frac_part.chars()) {
        let d = i128::from(c as u8 - b'0');
        value = value
            .checked_mul(10)
            .and_then(|v| v.checked_add(d))
            .ok_or_else(|| err("quantity too large"))?;
    }
    // `value` is now the digits with the decimal point removed; it is therefore
    // 10^frac_len times too large.
    let mut decimal_shift: i32 = -i32::try_from(frac_part.len())
        .map_err(|_| err("too many fractional digits"))?;

    // Suffix: binary multiplier, decimal power of ten, or an exponent.
    let mut binary_shift: u32 = 0;
    match suffix {
        "" => {}
        "m" => decimal_shift -= 3,
        "k" => decimal_shift += 3,
        "M" => decimal_shift += 6,
        "G" => decimal_shift += 9,
        "T" => decimal_shift += 12,
        "P" => decimal_shift += 15,
        "E" => decimal_shift += 18,
        "Ki" => binary_shift = 10,
        "Mi" => binary_shift = 20,
        "Gi" => binary_shift = 30,
        "Ti" => binary_shift = 40,
        "Pi" => binary_shift = 50,
        "Ei" => binary_shift = 60,
        other => {
            // Scientific notation: `e` or `E` followed by a signed exponent.
            let rest = other
                .strip_prefix('e')
                .or_else(|| other.strip_prefix('E'))
                .ok_or_else(|| err("unrecognised quantity suffix"))?;
            let exp: i32 = rest.parse().map_err(|_| err("invalid exponent"))?;
            decimal_shift += exp;
        }
    }

    // Convert to thousandths of the base unit.
    decimal_shift += 3;

    if binary_shift > 0 {
        value = value
            .checked_shl(binary_shift)
            .ok_or_else(|| err("quantity too large"))?;
    }

    if decimal_shift >= 0 {
        let factor = 10i128
            .checked_pow(u32::try_from(decimal_shift).map_err(|_| err("exponent too large"))?)
            .ok_or_else(|| err("quantity too large"))?;
        value = value
            .checked_mul(factor)
            .ok_or_else(|| err("quantity too large"))?;
    } else {
        let factor = 10i128
            .checked_pow(u32::try_from(-decimal_shift).map_err(|_| err("exponent too small"))?)
            .ok_or_else(|| err("quantity too small"))?;
        // Round away from zero so a request is never understated. Understating
        // a request would overstate available headroom, which is the direction
        // that causes an outage.
        let rounded = if value >= 0 {
            value
                .checked_add(factor - 1)
                .ok_or_else(|| err("quantity too large"))?
        } else {
            value
                .checked_sub(factor - 1)
                .ok_or_else(|| err("quantity too small"))?
        };
        value = rounded / factor;
    }

    Ok(Scaled {
        milli_units: if negative { -value } else { value },
    })
}

/// Parse a CPU quantity into millicores.
///
/// # Errors
///
/// Returns an error if the string is not a valid Kubernetes quantity.
pub fn parse_cpu(input: &str) -> Result<Millicores, QuantityParseError> {
    let scaled = parse_scaled(input)?;
    Ok(Millicores(clamp_i64(scaled.milli_units)))
}

/// Parse a memory or storage quantity into bytes.
///
/// Sub-byte values round up, for the same reason requests round away from
/// zero: never understate what a workload needs.
///
/// # Errors
///
/// Returns an error if the string is not a valid Kubernetes quantity.
pub fn parse_bytes(input: &str) -> Result<Bytes, QuantityParseError> {
    let scaled = parse_scaled(input)?;
    let bytes = if scaled.milli_units >= 0 {
        (scaled.milli_units + 999) / 1000
    } else {
        (scaled.milli_units - 999) / 1000
    };
    Ok(Bytes(clamp_i64(bytes)))
}

/// Saturate rather than wrap or panic. A cluster with an absurd declared
/// quantity should produce a clamped number and keep working.
fn clamp_i64(v: i128) -> i64 {
    i64::try_from(v).unwrap_or(if v > 0 { i64::MAX } else { i64::MIN })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn cpu_millicores() {
        assert_eq!(parse_cpu("100m").unwrap(), Millicores(100));
        assert_eq!(parse_cpu("1500m").unwrap(), Millicores(1500));
    }

    #[test]
    fn cpu_whole_and_fractional_cores() {
        assert_eq!(parse_cpu("1").unwrap(), Millicores(1000));
        assert_eq!(parse_cpu("2").unwrap(), Millicores(2000));
        // The case that would need a float if we let it.
        assert_eq!(parse_cpu("0.5").unwrap(), Millicores(500));
        assert_eq!(parse_cpu("0.1").unwrap(), Millicores(100));
        assert_eq!(parse_cpu("1.25").unwrap(), Millicores(1250));
        assert_eq!(parse_cpu("0.001").unwrap(), Millicores(1));
    }

    #[test]
    fn sub_milli_cpu_rounds_up_never_down() {
        // Understating a request overstates headroom, and overstated headroom
        // is what causes a drain to wedge.
        assert_eq!(parse_cpu("0.0001").unwrap(), Millicores(1));
        assert_eq!(parse_cpu("0.0019").unwrap(), Millicores(2));
    }

    #[test]
    fn binary_memory_suffixes() {
        assert_eq!(parse_bytes("128Mi").unwrap(), Bytes(128 * 1024 * 1024));
        assert_eq!(parse_bytes("1Gi").unwrap(), Bytes(1024 * 1024 * 1024));
        assert_eq!(parse_bytes("64Ki").unwrap(), Bytes(64 * 1024));
        assert_eq!(parse_bytes("1Ti").unwrap(), Bytes(1024i64.pow(4)));
    }

    #[test]
    fn decimal_memory_suffixes() {
        assert_eq!(parse_bytes("1k").unwrap(), Bytes(1000));
        assert_eq!(parse_bytes("1M").unwrap(), Bytes(1_000_000));
        assert_eq!(parse_bytes("2G").unwrap(), Bytes(2_000_000_000));
    }

    #[test]
    fn plain_integers_are_bytes() {
        assert_eq!(parse_bytes("1024").unwrap(), Bytes(1024));
        assert_eq!(parse_bytes("0").unwrap(), Bytes(0));
    }

    #[test]
    fn exa_suffix_and_exponent_notation_are_different_things() {
        // `E` is the exa suffix; `e` starts an exponent. Conflating them is
        // off by fifteen orders of magnitude.
        assert_eq!(parse_bytes("1E").unwrap(), Bytes(1_000_000_000_000_000_000));
        assert_eq!(parse_bytes("1e3").unwrap(), Bytes(1000));
        assert_eq!(parse_bytes("1.5e3").unwrap(), Bytes(1500));
        assert_eq!(parse_cpu("1e-3").unwrap(), Millicores(1));
    }

    #[test]
    fn absurd_values_saturate_rather_than_panic() {
        let huge = parse_bytes("9999999Ei");
        assert!(huge.is_err() || huge.unwrap().0 == i64::MAX);
    }

    #[test]
    fn malformed_input_is_rejected_not_guessed() {
        for bad in ["", "abc", "12Qi", "1.2.3", "m", "--5"] {
            assert!(parse_cpu(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn whitespace_is_tolerated() {
        assert_eq!(parse_cpu(" 100m ").unwrap(), Millicores(100));
    }
}
