//! The append-only JSONL log.
//!
//! Append-only is structural here, not enforced: the writer opens the file in
//! append mode and exposes no way to rewrite a line. That is the whole audit
//! property at this milestone — hash-chained tamper evidence arrives with the
//! execution controller, which is when there is a mutation worth tampering with
//! (ADR-0018, `THREAT_MODEL.md` R6).

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::Utc;
use ff_core::Mode;

use crate::error::RecordError;
use crate::event::{LogEntry, RecordedEvent, RunId};

/// Writes events to a JSONL file.
///
/// Cheap to clone behind an `Arc`; the file handle is behind a mutex so
/// concurrent writers cannot interleave half-lines.
#[derive(Debug)]
pub struct EventLog {
    path: PathBuf,
    run_id: RunId,
    mode: Mode,
    inner: Mutex<Inner>,
}

#[derive(Debug)]
struct Inner {
    file: File,
    seq: u64,
}

impl EventLog {
    /// Open or create a log file for a run.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened for appending.
    pub fn open(path: impl Into<PathBuf>, run_id: RunId, mode: Mode) -> Result<Self, RecordError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| RecordError::Io {
                path: parent.display().to_string(),
                reason: e.to_string(),
            })?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| RecordError::Io {
                path: path.display().to_string(),
                reason: e.to_string(),
            })?;

        // Continue the sequence if the file already has lines, so a restart
        // does not silently reset numbering and make a gap look like a
        // duplicate.
        let seq = read(&path).map(|entries| entries.len() as u64).unwrap_or(0);

        Ok(Self {
            path,
            run_id,
            mode,
            inner: Mutex::new(Inner { file, seq }),
        })
    }

    /// The file being written.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The run identifier.
    #[must_use]
    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }

    /// Append one event.
    ///
    /// Failures are returned rather than swallowed, but callers in the live
    /// path log and continue: losing the recording is bad, and taking the
    /// collector down with it is worse.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or the write fails.
    pub fn append(&self, event: RecordedEvent) -> Result<u64, RecordError> {
        let mut guard = self.inner.lock().map_err(|_| RecordError::Poisoned)?;
        guard.seq = guard.seq.saturating_add(1);

        let entry = LogEntry {
            seq: guard.seq,
            recorded_at: Utc::now(),
            run_id: self.run_id.clone(),
            mode: self.mode,
            event,
        };

        let mut line = serde_json::to_string(&entry)?;
        line.push('\n');
        guard
            .file
            .write_all(line.as_bytes())
            .map_err(|e| RecordError::Io {
                path: self.path.display().to_string(),
                reason: e.to_string(),
            })?;
        // Flushed per line so `tail -f` works during a live run, which is half
        // the point of a text log.
        guard.file.flush().map_err(|e| RecordError::Io {
            path: self.path.display().to_string(),
            reason: e.to_string(),
        })?;

        Ok(guard.seq)
    }

    /// Append, logging rather than propagating a failure.
    pub fn record(&self, event: RecordedEvent) {
        if let Err(err) = self.append(event) {
            tracing::warn!(error = %err, "failed to record event");
        }
    }
}

/// Read every entry from a log file.
///
/// A malformed line is skipped with a warning rather than failing the whole
/// read: a truncated final line from a killed process should not make the
/// preceding hour unreadable.
///
/// # Errors
///
/// Returns an error if the file cannot be read.
pub fn read(path: impl AsRef<Path>) -> Result<Vec<LogEntry>, RecordError> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|e| RecordError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;

    let mut entries = Vec::new();
    let mut malformed = 0usize;
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = match line {
            Ok(l) => l,
            Err(_) => {
                malformed += 1;
                continue;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<LogEntry>(&line) {
            Ok(entry) => entries.push(entry),
            Err(err) => {
                malformed += 1;
                tracing::warn!(line = index + 1, error = %err, "skipping malformed log line");
            }
        }
    }

    if malformed > 0 {
        tracing::warn!(
            malformed,
            path = %path.display(),
            "some log lines could not be parsed; the report will be incomplete"
        );
    }

    Ok(entries)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::event::WorkloadKey;

    fn log(dir: &tempfile::TempDir) -> EventLog {
        EventLog::open(
            dir.path().join("events.jsonl"),
            RunId::new("run-test"),
            Mode::Live,
        )
        .unwrap()
    }

    #[test]
    fn entries_are_sequential_and_readable() {
        let dir = tempfile::tempdir().unwrap();
        let events = log(&dir);
        events
            .append(RecordedEvent::RunStarted {
                description: "test".into(),
                cluster_id: "c1".into(),
            })
            .unwrap();
        events
            .append(RecordedEvent::PodRemoved {
                namespace: "demo".into(),
                name: "web-1".into(),
                node: Some("n1".into()),
                workload: Some(WorkloadKey {
                    namespace: "demo".into(),
                    name: "web".into(),
                }),
            })
            .unwrap();

        let entries = read(events.path()).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].seq, 1);
        assert_eq!(entries[1].seq, 2);
        assert_eq!(entries[0].run_id.as_str(), "run-test");
    }

    #[test]
    fn every_line_records_its_mode() {
        // A log read back from disk is replayed data. Without the mode on every
        // line, a reader cannot tell that from live state (ADR-0020).
        let dir = tempfile::tempdir().unwrap();
        let events = log(&dir);
        events
            .append(RecordedEvent::RunEnded {
                reason: "done".into(),
            })
            .unwrap();

        let raw = std::fs::read_to_string(events.path()).unwrap();
        assert!(raw.contains("\"mode\":\"live\""), "{raw}");
        assert_eq!(read(events.path()).unwrap()[0].mode, Mode::Live);
    }

    #[test]
    fn the_file_is_one_json_object_per_line() {
        // The format is the deliverable: a reviewer must be able to jq it.
        let dir = tempfile::tempdir().unwrap();
        let events = log(&dir);
        for i in 0..3 {
            events
                .append(RecordedEvent::NodeChanged {
                    name: format!("n{i}"),
                    ready: true,
                    unschedulable: false,
                    bottlerocket_version: None,
                })
                .unwrap();
        }
        let raw = std::fs::read_to_string(events.path()).unwrap();
        let lines: Vec<&str> = raw.lines().collect();
        assert_eq!(lines.len(), 3);
        for line in lines {
            let parsed: serde_json::Value = serde_json::from_str(line).expect("each line is JSON");
            assert!(parsed.get("seq").is_some());
            assert!(parsed.get("event").is_some());
        }
    }

    #[test]
    fn reopening_continues_the_sequence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        {
            let events = EventLog::open(&path, RunId::new("r"), Mode::Live).unwrap();
            events
                .append(RecordedEvent::RunEnded { reason: "a".into() })
                .unwrap();
        }
        let reopened = EventLog::open(&path, RunId::new("r"), Mode::Live).unwrap();
        let seq = reopened
            .append(RecordedEvent::RunEnded { reason: "b".into() })
            .unwrap();
        assert_eq!(seq, 2, "a restart must not reset numbering");
    }

    #[test]
    fn a_truncated_final_line_does_not_lose_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        {
            let events = EventLog::open(&path, RunId::new("r"), Mode::Live).unwrap();
            events
                .append(RecordedEvent::RunEnded { reason: "a".into() })
                .unwrap();
        }
        // Simulate a process killed mid-write.
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"{\"seq\":2,\"recorded_at\"").unwrap();

        let entries = read(&path).unwrap();
        assert_eq!(entries.len(), 1, "the complete line survives");
    }
}
