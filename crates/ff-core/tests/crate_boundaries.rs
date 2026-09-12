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
fn no_crate_but_ff_collect_mentions_kube_in_its_source() {
    // The manifest check above is necessary but not sufficient. `ff-api`
    // depends on `ff-collect`, so it receives `kube` transitively and could
    // construct a client through the re-exported path without ever declaring
    // the dependency. This checks the source itself.
    let mut violations = Vec::new();
    let mut scanned = 0usize;

    for entry in fs::read_dir(crates_dir()).expect("crates/ is readable") {
        let path = entry.expect("directory entry is readable").path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        if KUBE_CLIENT_OWNERS.contains(&name.as_str()) || !path.join("Cargo.toml").is_file() {
            continue;
        }
        for file in rust_sources(&path.join("src")) {
            scanned += 1;
            let text = fs::read_to_string(&file).expect("source is readable");
            for (lineno, line) in text.lines().enumerate() {
                let code = line.trim_start();
                if code.starts_with("//") || code.starts_with("*") {
                    continue;
                }
                if code.contains("kube::") || code.contains("k8s_openapi::") {
                    violations.push(format!(
                        "{}:{}: {}",
                        file.display(),
                        lineno + 1,
                        code.trim()
                    ));
                }
            }
        }
    }

    // A scan that finds no files would pass silently and enforce nothing.
    assert!(
        scanned >= 3,
        "expected to scan several source files, scanned {scanned} — the walk is broken"
    );
    assert!(
        violations.is_empty(),
        "only {KUBE_CLIENT_OWNERS:?} may reference the Kubernetes client in source \
         (ADR-0008). Take a ClusterSnapshot instead. Violations:\n  {}",
        violations.join("\n  ")
    );
}

/// Every `.rs` file under a directory.
fn rust_sources(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(rust_sources(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
    found
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
