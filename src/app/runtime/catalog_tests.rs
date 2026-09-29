use crate::agents::Backend;
use std::path::Path;

use super::*;

fn summary(path: &Path) -> SessionSummary {
    SessionSummary::from_cached(
        "external".into(),
        path.to_path_buf(),
        PathBuf::from("/project"),
        "External".into(),
        String::new(),
        String::new(),
        None,
        SystemTime::now(),
        0,
        crate::sessions::UsageSummary::default(),
        false,
        false,
        String::new(),
    )
}

#[test]
fn import_preview_skips_sessions_already_in_the_catalog() {
    let known = HashSet::from([PathBuf::from("/sessions/known.jsonl")]);
    let known_session = summary(Path::new("/sessions/known.jsonl"));
    let mut unknown = summary(Path::new("/sessions/new.jsonl"));
    unknown.id = "new".into();

    let candidates = unknown_import_candidates(vec![known_session, unknown.clone()], &known);

    assert_eq!(candidates, vec![unknown]);
}

#[test]
fn import_preview_skips_nested_child_workers() {
    let parent = summary(Path::new("/sessions/parent.jsonl"));
    let mut child = summary(Path::new("/sessions/child.jsonl"));
    child.id = "child".into();
    child.parent_session = Some("parent".into());

    let candidates = unknown_import_candidates(vec![parent.clone(), child], &HashSet::new());

    assert_eq!(candidates, vec![parent]);
}

#[test]
fn pool_snapshot_native_id_can_match_a_synthetic_child_locator() {
    let mut child = summary(Path::new("/locators/codex-cli/native-child"));
    child.id = "native-child".into();
    child.harness = Backend::Codex;
    child.parent_session = Some("parent".into());
    let snapshot = agents::WorkerSnapshot {
        id: "worker-1".into(),
        backend: Backend::Codex,
        project: child.project.clone(),
        session_locator: Some("native-child".into()),
        status: agents::WorkerStatus::Idle,
        output: Some("done".into()),
        error: None,
        pending_input: None,
    };

    let matched = session_for_worker_snapshot(std::slice::from_ref(&child), &snapshot)
        .expect("native id should match the stored backend id");
    let activity = AgentActivity::from_worker_snapshot(matched, snapshot.lifecycle());

    assert_eq!(
        activity.lifecycle,
        crate::agent_activity::AgentLifecycle::Completed(
            crate::agent_activity::AgentOutcome::Complete
        )
    );
}

#[test]
fn pool_snapshot_native_id_is_scoped_by_backend_and_project() {
    let mut wrong_project = summary(Path::new("/other/child"));
    wrong_project.id = "shared-child".into();
    wrong_project.harness = Backend::Codex;
    wrong_project.parent_session = Some("other-parent".into());
    let mut wrong_backend = summary(Path::new("/project/pi-child"));
    wrong_backend.id = "shared-child".into();
    wrong_backend.parent_session = Some("pi-parent".into());
    let mut expected = summary(Path::new("/project/codex-child"));
    expected.id = "shared-child".into();
    expected.harness = Backend::Codex;
    expected.parent_session = Some("codex-parent".into());
    let snapshot = agents::WorkerSnapshot {
        id: "worker-1".into(),
        backend: Backend::Codex,
        project: expected.project.clone(),
        session_locator: Some("shared-child".into()),
        status: agents::WorkerStatus::Idle,
        output: None,
        error: None,
        pending_input: None,
    };
    wrong_project.project = PathBuf::from("/other-project");
    let sessions = [wrong_project, wrong_backend, expected.clone()];

    let matched = session_for_worker_snapshot(&sessions, &snapshot).expect("scoped child");

    assert_eq!(matched.path, expected.path);
    assert_eq!(matched.harness, expected.harness);
    assert_eq!(matched.project, expected.project);
}

#[test]
fn idle_pool_snapshot_without_a_settled_output_stays_unknown() {
    let mut child = summary(Path::new("/sessions/child.jsonl"));
    child.parent_session = Some("parent".into());
    let snapshot = agents::WorkerSnapshot {
        id: "worker-1".into(),
        backend: child.harness,
        project: child.project.clone(),
        session_locator: Some(child.path.to_string_lossy().into_owned()),
        status: agents::WorkerStatus::Idle,
        output: None,
        error: None,
        pending_input: None,
    };

    let activity = AgentActivity::from_worker_snapshot(&child, snapshot.lifecycle());

    assert_eq!(
        activity.lifecycle,
        crate::agent_activity::AgentLifecycle::Unknown
    );
}

#[test]
fn normalized_native_failure_becomes_a_terminal_child_activity() {
    let child = serde_json::json!({
        "harness": "codex-cli",
        "id": "child",
        "path": "/locators/codex-cli/child",
        "project": "/project",
        "title": "Reviewer",
        "first_user_message": null,
        "parent_session": "parent",
        "message_count": null,
        "model": null,
        "thinking_level": null,
        "service_tier": null,
        "usage": null,
        "is_running": false,
        "outcome": "failed"
    });
    let metadata: agents::SessionMetadata =
        serde_json::from_value(child.clone()).expect("native child metadata");

    let activity = native_child_activity(&child, &metadata);

    assert_eq!(
        activity.lifecycle,
        crate::agent_activity::AgentLifecycle::Completed(
            crate::agent_activity::AgentOutcome::Failed
        )
    );
    assert!(activity.limited);
}
