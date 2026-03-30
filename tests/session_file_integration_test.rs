use ssh_tunnel_manager::services::session_file::{SessionFileStore, SessionRecord};
use std::process::{Child, Command};
use std::sync::Mutex;

/// Holds a child process so we can both get its PID and later wait/kill it.
struct TrackedProcess {
    child: Mutex<Child>,
    pid: u32,
}

/// Helper: create a long-running child process that we can signal.
fn spawn_sleeper() -> TrackedProcess {
    let child = Command::new("sleep")
        .arg("300")
        .spawn()
        .expect("failed to spawn sleep process");
    let pid = child.id();
    TrackedProcess {
        child: Mutex::new(child),
        pid,
    }
}

/// Kill process and wait for it to be fully reaped (not zombie).
fn kill_and_reap(proc: &TrackedProcess) {
    unsafe {
        libc::kill(proc.pid as i32, libc::SIGTERM);
    }
    // Wait to reap the child so it doesn't stay as zombie
    let _ = proc.child.lock().unwrap().wait();
}

fn make_record_for(proc: &TrackedProcess) -> SessionRecord {
    make_record(proc.pid)
}

fn make_record(pid: u32) -> SessionRecord {
    SessionRecord {
        session_id: uuid::Uuid::new_v4(),
        connection_name: "test-conn".into(),
        host: "example.com".into(),
        port: 22,
        username: "user".into(),
        pid,
        started_at: chrono::Utc::now(),
        forwarding_descriptions: vec!["127.0.0.1:8080 → localhost:80".into()],
    }
}

#[test]
fn test_list_active_shows_live_process() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionFileStore::new(dir.path()).unwrap();

    let proc = spawn_sleeper();
    let record = make_record_for(&proc);
    store.write(&record).unwrap();

    let active = store.list_active().unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].session_id, record.session_id);
    assert_eq!(active[0].pid, proc.pid);

    // Cleanup
    kill_and_reap(&proc);
}

#[test]
fn test_list_active_cleans_up_after_kill() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionFileStore::new(dir.path()).unwrap();

    let proc = spawn_sleeper();
    let record = make_record_for(&proc);
    let session_id = record.session_id;
    store.write(&record).unwrap();

    // Verify it's there first
    assert!(store.find(session_id).unwrap().is_some());

    // Kill and reap the process
    kill_and_reap(&proc);

    // list_active should now return empty and clean up the stale file
    let active = store.list_active().unwrap();
    assert!(active.is_empty());

    // The file should have been removed by list_active
    let session_file = dir.path().join("sessions").join(format!("{session_id}.json"));
    assert!(!session_file.exists());
}

#[test]
fn test_find_returns_none_for_dead_process() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionFileStore::new(dir.path()).unwrap();

    let proc = spawn_sleeper();
    let record = make_record_for(&proc);
    let session_id = record.session_id;
    store.write(&record).unwrap();

    kill_and_reap(&proc);

    // find should return None and clean up the file
    assert!(store.find(session_id).unwrap().is_none());
}

#[test]
fn test_disconnect_via_sigterm() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionFileStore::new(dir.path()).unwrap();

    // Spawn a child that we will SIGTERM
    let proc = spawn_sleeper();
    let record = make_record_for(&proc);
    let session_id = record.session_id;
    store.write(&record).unwrap();

    // Simulate what `disconnect` command does: find + kill
    let found = store.find(session_id).unwrap();
    assert!(found.is_some());
    let found = found.unwrap();

    unsafe {
        let ret = libc::kill(found.pid as i32, libc::SIGTERM);
        assert_eq!(ret, 0, "SIGTERM should succeed for live process");
    }

    // Reap the child
    let _ = proc.child.lock().unwrap().wait();

    // Now the process is dead; next list_active should clean up
    let active = store.list_active().unwrap();
    assert!(active.is_empty());
}

#[test]
fn test_multiple_sessions_mixed_alive_dead() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionFileStore::new(dir.path()).unwrap();

    // Spawn two processes
    let proc_alive = spawn_sleeper();
    let proc_dead = spawn_sleeper();

    let record_alive = make_record_for(&proc_alive);
    let record_dead = make_record_for(&proc_dead);
    let alive_id = record_alive.session_id;

    store.write(&record_alive).unwrap();
    store.write(&record_dead).unwrap();

    // Kill one and reap it
    kill_and_reap(&proc_dead);

    // list_active should only show the alive one
    let active = store.list_active().unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].session_id, alive_id);

    // Cleanup
    kill_and_reap(&proc_alive);
}

#[test]
fn test_session_record_serialization_excludes_nothing() {
    // Verify SessionRecord round-trips correctly through JSON
    let record = SessionRecord {
        session_id: uuid::Uuid::new_v4(),
        connection_name: "my-server".into(),
        host: "10.0.0.1".into(),
        port: 2222,
        username: "admin".into(),
        pid: 12345,
        started_at: chrono::Utc::now(),
        forwarding_descriptions: vec![
            "127.0.0.1:3306 → localhost:3306".into(),
            "127.0.0.1:6379 → localhost:6379".into(),
        ],
    };

    let json = serde_json::to_string(&record).unwrap();
    let deserialized: SessionRecord = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.session_id, record.session_id);
    assert_eq!(deserialized.connection_name, "my-server");
    assert_eq!(deserialized.host, "10.0.0.1");
    assert_eq!(deserialized.port, 2222);
    assert_eq!(deserialized.username, "admin");
    assert_eq!(deserialized.pid, 12345);
    assert_eq!(deserialized.forwarding_descriptions.len(), 2);
}
