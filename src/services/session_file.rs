use crate::utils::error::{Result, SshToolError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Persistent session record written to disk by a running `connect` process.
/// Other CLI invocations (`sessions`, `disconnect`) read these files to
/// discover active sessions and send signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: uuid::Uuid,
    pub connection_name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub pid: u32,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub forwarding_descriptions: Vec<String>,
}

/// Directory-based session file store.
pub struct SessionFileStore {
    dir: PathBuf,
}

impl SessionFileStore {
    /// Create a store under `<config_dir>/sessions/`.
    pub fn new(config_dir: &Path) -> Result<Self> {
        let dir = config_dir.join("sessions");
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }
        Ok(Self { dir })
    }

    /// Write a session record to disk.
    pub fn write(&self, record: &SessionRecord) -> Result<()> {
        let path = self.dir.join(format!("{}.json", record.session_id));
        let data = serde_json::to_string_pretty(record)
            .map_err(|e| SshToolError::ConfigError(format!("Failed to serialize session: {}", e)))?;
        fs::write(&path, data)?;
        Ok(())
    }

    /// Remove a session file.
    pub fn remove(&self, session_id: uuid::Uuid) -> Result<()> {
        let path = self.dir.join(format!("{session_id}.json"));
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// List all session records whose owning process is still alive.
    pub fn list_active(&self) -> Result<Vec<SessionRecord>> {
        let mut records = Vec::new();

        let entries = match fs::read_dir(&self.dir) {
            Ok(e) => e,
            Err(_) => return Ok(records),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json")
                && let Ok(data) = fs::read_to_string(&path)
                && let Ok(record) = serde_json::from_str::<SessionRecord>(&data)
            {
                if is_process_alive(record.pid) {
                    records.push(record);
                } else {
                    let _ = fs::remove_file(&path);
                }
            }
        }

        Ok(records)
    }

    /// Find a session record by ID (only if process alive).
    pub fn find(&self, session_id: uuid::Uuid) -> Result<Option<SessionRecord>> {
        let path = self.dir.join(format!("{session_id}.json"));
        if !path.exists() {
            return Ok(None);
        }
        let data = fs::read_to_string(&path)?;
        let record: SessionRecord = serde_json::from_str(&data)
            .map_err(|e| SshToolError::ConfigError(format!("Bad session file: {}", e)))?;

        if is_process_alive(record.pid) {
            Ok(Some(record))
        } else {
            let _ = fs::remove_file(&path);
            Ok(None)
        }
    }
}

/// Check whether a process with the given PID is still running.
fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // kill(pid, 0) checks existence without sending a signal
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        // Fallback: assume alive (Windows would need a different approach)
        let _ = pid;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionFileStore::new(dir.path()).unwrap();

        let record = SessionRecord {
            session_id: uuid::Uuid::new_v4(),
            connection_name: "test".into(),
            host: "example.com".into(),
            port: 22,
            username: "user".into(),
            pid: std::process::id(),
            started_at: chrono::Utc::now(),
            forwarding_descriptions: vec!["127.0.0.1:8080 → localhost:80".into()],
        };

        store.write(&record).unwrap();

        let active = store.list_active().unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].session_id, record.session_id);

        store.remove(record.session_id).unwrap();
        let active = store.list_active().unwrap();
        assert!(active.is_empty());
    }

    #[test]
    fn test_stale_session_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionFileStore::new(dir.path()).unwrap();

        let record = SessionRecord {
            session_id: uuid::Uuid::new_v4(),
            connection_name: "stale".into(),
            host: "example.com".into(),
            port: 22,
            username: "user".into(),
            pid: 999999999, // very unlikely to be a real process
            started_at: chrono::Utc::now(),
            forwarding_descriptions: vec![],
        };

        store.write(&record).unwrap();

        // Should be filtered out as stale and file removed
        let active = store.list_active().unwrap();
        assert!(active.is_empty());
    }
}
