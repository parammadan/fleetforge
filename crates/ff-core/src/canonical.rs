//! Canonical serialization and content hashing.
//!
//! A [`SnapshotId`] must be byte-identical for the same cluster state, on any
//! architecture, in any process. Three things could break that, and each is
//! handled here.
//!
//! **Key order.** `serde_json` emits struct fields in declaration order and map
//! keys in insertion order. Canonicalization re-sorts every object key.
//!
//! **Fact order.** A watch delivers events in whatever order they arrive.
//! [`ClusterSnapshot::new`](crate::ClusterSnapshot::new) sorts facts before
//! hashing.
//!
//! **Observation time.** The same cluster state observed twice is the *same*
//! state and must carry the same identifier, so timestamps recording *when
//! FleetForge looked* are stripped. Timestamps that are genuinely part of
//! cluster state — a node condition's last transition, an event's first
//! occurrence — are kept.
//!
//! Floating point would be a fourth hazard, which is why the model has none:
//! `clippy::float_arithmetic` is denied workspace-wide and quantities are
//! integers.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::Result;
use crate::snapshot::SnapshotId;

/// Field names stripped before hashing.
///
/// These record *when FleetForge observed something*, not what the cluster
/// contains. Deliberately distinctive names — a generic `at` would risk
/// stripping a field that does carry meaning.
///
/// Note what is **not** here: `mode` is hashed, so a fixture-derived snapshot
/// can never collide with a live one. That matters beyond tidiness — an
/// approval bound to a plan hash must not validate against a different data
/// mode.
const VOLATILE_FIELDS: &[&str] = &[
    "snapshot_id",
    "taken_at",
    "observed_at",
    "synced_at",
    "degraded_since",
    "last_current_at",
    "age_seconds",
];

/// Compute the content hash of any serializable value.
///
/// # Errors
///
/// Returns [`FleetForgeError::Canonicalization`](crate::FleetForgeError::Canonicalization)
/// if the value cannot be serialized.
pub(crate) fn content_hash<T: Serialize>(value: &T) -> Result<SnapshotId> {
    let mut json = serde_json::to_value(value)?;
    strip_volatile(&mut json);

    let mut buf = String::new();
    write_canonical(&json, &mut buf);

    let digest = Sha256::digest(buf.as_bytes());
    Ok(SnapshotId::from_hex_unchecked(hex::encode(digest)))
}

/// Recursively remove volatile fields.
fn strip_volatile(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.retain(|k, _| !VOLATILE_FIELDS.contains(&k.as_str()));
            for v in map.values_mut() {
                strip_volatile(v);
            }
        }
        serde_json::Value::Array(items) => {
            for v in items {
                strip_volatile(v);
            }
        }
        _ => {}
    }
}

/// Write a value in canonical form: object keys sorted, no insignificant
/// whitespace, numbers as `serde_json` renders them.
///
/// `serde_json::Map` preserves insertion order by default, so sorting here is
/// what makes the output independent of how the value was built.
fn write_canonical(value: &serde_json::Value, out: &mut String) {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json_string(key, out);
                out.push(':');
                if let Some(v) = map.get(*key) {
                    write_canonical(v, out);
                }
            }
            out.push('}');
        }
        serde_json::Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        serde_json::Value::String(s) => write_json_string(s, out),
        serde_json::Value::Null => out.push_str("null"),
        serde_json::Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        serde_json::Value::Number(n) => out.push_str(&n.to_string()),
    }
}

/// Write a JSON string literal with the escapes the specification requires.
fn write_json_string(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn object_key_order_does_not_affect_output() {
        let a: serde_json::Value = serde_json::from_str(r#"{"b":1,"a":2}"#).unwrap();
        let b: serde_json::Value = serde_json::from_str(r#"{"a":2,"b":1}"#).unwrap();
        let (mut sa, mut sb) = (String::new(), String::new());
        write_canonical(&a, &mut sa);
        write_canonical(&b, &mut sb);
        assert_eq!(sa, sb);
        assert_eq!(sa, r#"{"a":2,"b":1}"#);
    }

    #[test]
    fn array_order_is_preserved() {
        // Arrays are ordered data. Callers sort them deliberately before
        // hashing; canonicalization must not silently reorder them.
        let v: serde_json::Value = serde_json::from_str("[3,1,2]").unwrap();
        let mut s = String::new();
        write_canonical(&v, &mut s);
        assert_eq!(s, "[3,1,2]");
    }

    #[test]
    fn volatile_fields_are_stripped_at_every_depth() {
        let mut v: serde_json::Value = serde_json::from_str(
            r#"{"observed_at":"x","inner":{"taken_at":"y","keep":1},
                "list":[{"synced_at":"z","also_keep":2}]}"#,
        )
        .unwrap();
        strip_volatile(&mut v);
        let mut s = String::new();
        write_canonical(&v, &mut s);
        assert_eq!(s, r#"{"inner":{"keep":1},"list":[{"also_keep":2}]}"#);
    }

    #[test]
    fn control_characters_are_escaped() {
        let mut s = String::new();
        write_json_string("a\u{1}b\"c", &mut s);
        let expected = concat!(r#""a\"#, "u0001", r#"b\"c""#);
        assert_eq!(s, expected);
    }
}
