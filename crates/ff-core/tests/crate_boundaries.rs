//! Enforces ADR-0008: only `ff-collect` may construct a Kubernetes client.
//!
//! "Read-only mode cannot mutate the cluster" is a load-bearing safety claim.
//! If any crate can build a client, that claim rests on every contributor
//! remembering it forever, which is not a guarantee. This test turns it into a
//! one-line check that fails the build.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

/// Crates permitted to depend on `kube`.
///
/// `ff-exec` will join this list when — and only when — ADR-0012 is revisited
/// and an execution adapter exists. Adding a name here should require an ADR.
const KUBE_CLIENT_OWNERS: &[&str] = &["ff-collect"];

fn crates_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR is <workspace>/crates/ff-core
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("ff-core lives inside crates/")
        .to_path_buf()
}

#[test]
fn only_ff_collect_may_depend_on_kube() {
    let mut violations = Vec::new();

    for entry in fs::read_dir(crates_dir()).expect("crates/ is readable") {
        let path = entry.expect("directory entry is readable").path();
        let manifest = path.join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        if KUBE_CLIENT_OWNERS.contains(&name.as_str()) {
            continue;
        }

        let text = fs::read_to_string(&manifest).expect("manifest is readable");
        for line in text.lines() {
            let trimmed = line.trim();
            let declares_kube = trimmed.starts_with("kube ")
                || trimmed.starts_with("kube=")
                || trimmed.starts_with("kube.")
                || trimmed.starts_with("k8s-openapi");
            if declares_kube {
                violations.push(format!("{name}: {trimmed}"));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "only {KUBE_CLIENT_OWNERS:?} may depend on the Kubernetes client (ADR-0008). \
         Take a ClusterSnapshot instead. Violations:\n  {}",
        violations.join("\n  ")
    );
}

#[test]
fn ff_core_has_no_io_dependencies() {
    // ff-core is types only. If it grows a runtime, a client, or a filesystem
    // dependency, the analyzers stop being testable without a cluster and the
    // crate graph stops enforcing anything.
    let manifest = crates_dir().join("ff-core").join("Cargo.toml");
    let text = fs::read_to_string(manifest).expect("ff-core manifest is readable");

    const FORBIDDEN: &[&str] = &["tokio", "kube", "axum", "reqwest", "sqlx", "hyper"];
    for forbidden in FORBIDDEN {
        for line in text.lines() {
            let trimmed = line.trim();
            assert!(
                !(trimmed.starts_with(&format!("{forbidden} "))
                    || trimmed.starts_with(&format!("{forbidden}="))
                    || trimmed.starts_with(&format!("{forbidden}."))),
                "ff-core must hold types only; found a dependency on {forbidden}"
            );
        }
    }
}
