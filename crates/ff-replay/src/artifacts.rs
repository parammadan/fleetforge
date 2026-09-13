//! Safe, read-only access to bundle artifacts.
//!
//! Two rules, both enforced here rather than at the HTTP layer:
//!
//! **Artifacts are addressed by name, never by path.** A caller asks for
//! `"03-preflight-before.json"`; they cannot ask for `"../../.aws/credentials"`
//! or an absolute path, because the name is looked up in a manifest built at
//! load time and anything not in that manifest does not exist. There is no
//! string concatenation between caller input and a filesystem path.
//!
//! **Content is redacted before it leaves this module.** The bundle was
//! scrubbed when captured, but a second pass costs little and the failure mode
//! — a bearer token rendered into a browser — is severe enough to be worth
//! belt and braces.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::ReplayError;
use crate::schema::{ArtifactKind, ArtifactRef};

/// Patterns replaced before an artifact is returned.
///
/// Each is a field name whose *value* is replaced, not the field itself: a
/// reader should be able to see that a token was present without seeing it.
const REDACT_KEYS: &[&str] = &[
    "token",
    "access_token",
    "accessToken",
    "client-certificate-data",
    "client-key-data",
    "certificate-authority-data",
    "aws_secret_access_key",
    "aws_access_key_id",
    "password",
    "secret",
];

/// The artifacts a bundle exposes, addressable by name.
#[derive(Debug, Clone)]
pub struct ArtifactStore {
    root: PathBuf,
    manifest: BTreeMap<String, ArtifactRef>,
}

impl ArtifactStore {
    /// Index every readable file in `root`, one level deep.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be listed.
    pub fn index(root: &Path) -> Result<Self, ReplayError> {
        let entries = std::fs::read_dir(root).map_err(|e| ReplayError::Io {
            path: root.display().to_string(),
            reason: e.to_string(),
        })?;

        let mut manifest = BTreeMap::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            // Dotfiles are never exposed. Nothing in a bundle needs to be one,
            // and credential files usually are.
            if name.starts_with('.') {
                continue;
            }
            let bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let sha256 = file_sha256(&path).unwrap_or_else(|_| "unavailable".to_owned());
            manifest.insert(
                name.to_owned(),
                ArtifactRef {
                    name: name.to_owned(),
                    description: describe(name),
                    kind: kind_of(name),
                    bytes,
                    sha256,
                },
            );
        }

        Ok(Self {
            root: root.to_path_buf(),
            manifest,
        })
    }

    /// Every artifact, for the manifest endpoint.
    #[must_use]
    pub fn list(&self) -> Vec<ArtifactRef> {
        self.manifest.values().cloned().collect()
    }

    /// Whether a name is known.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.manifest.contains_key(name)
    }

    /// Metadata for one artifact.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ArtifactRef> {
        self.manifest.get(name)
    }

    /// Read an artifact's content, redacted.
    ///
    /// `limit_bytes` truncates large files — the complete event log is 1.5 MB
    /// and a browser does not need all of it to show one entry.
    ///
    /// # Errors
    ///
    /// Returns [`ReplayError::UnknownArtifact`] for any name not in the
    /// manifest, which includes every traversal attempt.
    pub fn read(&self, name: &str, limit_bytes: usize) -> Result<String, ReplayError> {
        // The only way to a path is through the manifest. A name containing
        // `..`, `/`, or a drive letter simply is not a key.
        let entry = self
            .manifest
            .get(name)
            .ok_or(ReplayError::UnknownArtifact)?;
        let path = self.root.join(&entry.name);

        // Belt and braces: even having come from the manifest, the resolved
        // path must still sit inside the bundle.
        let canonical_root = self.root.canonicalize().map_err(|e| ReplayError::Io {
            path: self.root.display().to_string(),
            reason: e.to_string(),
        })?;
        let canonical = path
            .canonicalize()
            .map_err(|_| ReplayError::UnknownArtifact)?;
        if !canonical.starts_with(&canonical_root) {
            return Err(ReplayError::UnknownArtifact);
        }

        let raw = std::fs::read_to_string(&canonical).map_err(|e| ReplayError::Io {
            path: entry.name.clone(),
            reason: e.to_string(),
        })?;

        let truncated = if raw.len() > limit_bytes {
            let mut cut = limit_bytes.min(raw.len());
            while cut > 0 && !raw.is_char_boundary(cut) {
                cut -= 1;
            }
            format!(
                "{}\n\n[truncated: {} of {} bytes shown]",
                &raw[..cut],
                cut,
                raw.len()
            )
        } else {
            raw
        };

        Ok(redact(&truncated))
    }
}

/// Replace the value of any sensitive-looking key.
///
/// Deliberately textual rather than structural: it runs over JSON, JSONL, YAML,
/// and plain logs alike, and a bundle contains all four.
#[must_use]
pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let mut redacted = line.to_owned();
        for key in REDACT_KEYS {
            // Matches `"token": "value"`, `token: value`, `token=value`.
            if let Some(idx) = redacted.find(key) {
                let after = &redacted[idx + key.len()..];
                let sep = after.find([':', '=']);
                if let Some(sep) = sep {
                    let value_start = idx + key.len() + sep + 1;
                    let rest = &redacted[value_start..];
                    let trimmed = rest.trim_start();
                    // Only redact something that looks like a real value.
                    if trimmed.len() > 8 {
                        let end = rest.find([',', '\n']).unwrap_or(rest.len());
                        redacted =
                            format!("{}[REDACTED]{}", &redacted[..value_start], &rest[end..]);
                    }
                }
            }
        }
        out.push_str(&redacted);
        out.push('\n');
    }
    out
}

fn file_sha256(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}

fn kind_of(name: &str) -> ArtifactKind {
    if name.ends_with(".jsonl") {
        ArtifactKind::Jsonl
    } else if name.ends_with(".json") {
        ArtifactKind::Json
    } else if name.ends_with(".md") {
        ArtifactKind::Markdown
    } else {
        ArtifactKind::Text
    }
}

/// A human description, so the explorer is navigable without opening files.
fn describe(name: &str) -> String {
    let d = match name {
        "00-CONCLUSIONS.md" => "What actually happened, with accurate conclusions",
        "01-snapshot-before.json" => "FleetForge snapshot before the first uncordon",
        "02-environment-before.json" => "Environment and collection status before recovery",
        "03-preflight-before.json" => "Preflight result showing the PDB blocker",
        "04-pdb-before.json" => "PodDisruptionBudget state before recovery",
        "05-pods-before.txt" => "Pod placement before recovery",
        "06-nodes-before.json" => "Kubernetes node objects before recovery",
        "07-brupop-before.json" => "Brupop BottlerocketShadow state before recovery",
        "08-k8s-events-before.txt" => "Kubernetes events before recovery",
        "09-brupop-controller.log" => "Brupop controller log",
        "10-brupop-agent.log" => "Brupop agent log",
        "12-recovery-timeline.txt" => "Observed timeline of the first uncordon",
        "13-snapshot-after.json" => "FleetForge snapshot after the first uncordon",
        "15-preflight-after.json" => "Preflight result after the first uncordon",
        "21-evidence-report.md" => "Generated evidence report, including prediction scoring",
        "22-node-194-before-uncordon.json" => "Node 101-194 immediately before the second uncordon",
        "23-brupop-194-before-uncordon.json" => {
            "Brupop shadow for 101-194 before the second uncordon"
        }
        "24-uncordon-194-recovery.txt" => "Observed recovery after the second uncordon",
        "25-nodes-final.json" => "Kubernetes node objects at the end",
        "27-pdb-final.json" => "PodDisruptionBudget state at the end",
        "28-brupop-final.json" => "Brupop state at the end",
        "29-snapshot-final.json" => "FleetForge snapshot at the end",
        "31-traffic-post-recovery.txt" => {
            "Post-recovery traffic sampler output (failed validation)"
        }
        "33-kube-proxy.log" => "kube-proxy logs",
        "34-aws-node.log" => "VPC CNI (aws-node) logs",
        "35-fleetforge-events-complete.jsonl" => "Complete FleetForge event log — the replay spine",
        _ => "",
    };
    if d.is_empty() {
        format!("Captured artifact: {name}")
    } else {
        d.to_owned()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, ArtifactStore) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.json"), r#"{"x":1}"#).unwrap();
        std::fs::write(dir.path().join("b.log"), "hello\n").unwrap();
        std::fs::write(dir.path().join(".secret"), "nope").unwrap();
        let s = ArtifactStore::index(dir.path()).unwrap();
        (dir, s)
    }

    #[test]
    fn dotfiles_are_never_indexed() {
        let (_d, s) = store();
        assert!(!s.contains(".secret"));
        assert_eq!(s.list().len(), 2);
    }

    #[test]
    fn traversal_attempts_are_indistinguishable_from_missing_files() {
        let (_d, s) = store();
        for attempt in [
            "../../../etc/passwd",
            "..",
            "/etc/passwd",
            "./a.json",
            "a.json/../../b.log",
            "subdir/a.json",
        ] {
            let err = s.read(attempt, 1024).unwrap_err();
            assert!(
                matches!(err, ReplayError::UnknownArtifact),
                "{attempt} produced {err:?}"
            );
        }
    }

    #[test]
    fn known_artifacts_read_with_a_hash() {
        let (_d, s) = store();
        assert!(s.read("a.json", 1024).unwrap().contains("\"x\":1"));
        let meta = s.get("a.json").unwrap();
        assert_eq!(meta.sha256.len(), 64);
        assert_eq!(meta.kind, ArtifactKind::Json);
    }

    #[test]
    fn oversized_artifacts_are_truncated_and_say_so() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("big.txt"), "x".repeat(5000)).unwrap();
        let s = ArtifactStore::index(dir.path()).unwrap();
        let out = s.read("big.txt", 100).unwrap();
        assert!(out.contains("truncated"));
        assert!(out.len() < 500);
    }

    #[test]
    fn credentials_are_redacted_but_their_presence_is_visible() {
        let text = r#"users: [{name: r, user: {token: abcdefghijklmnopqrstuvwxyz012345}}]"#;
        let out = redact(text);
        assert!(out.contains("token"), "the field stays: {out}");
        assert!(out.contains("[REDACTED]"), "{out}");
        assert!(!out.contains("abcdefghijklmnop"), "{out}");
    }

    #[test]
    fn short_values_are_left_alone_so_ordinary_text_survives() {
        // "secret: no" is prose, not a credential.
        let out = redact("secret: no");
        assert!(out.contains("no"));
    }
}
