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
