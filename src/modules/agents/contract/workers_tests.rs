use std::path::PathBuf;

use crate::agents::Backend;
use crate::modules::sessions::activity::{AgentLifecycle, AgentOutcome};

use super::*;

#[test]
fn peer_prompt_round_trips_structured_origin() {
    let peer = PeerMessage {
        from: "diff-review".into(),
        message: "review complete\nwith details".into(),
    };
    assert_eq!(PeerMessage::from_prompt(&peer.prompt()), Some(peer));
    assert!(PeerMessage::from_prompt("Message from Farcaster worker bad id:\n\nno").is_none());
    assert!(PeerMessage::from_prompt("ordinary user message").is_none());
    assert_eq!(
        PeerMessage::from_prompt("Message from Farcaster peer worker-7:\n\nlegacy")
            .map(|message| message.from),
        Some("worker-7".into())
    );
}

fn snapshot(status: WorkerStatus, output: Option<&str>) -> WorkerSnapshot {
    WorkerSnapshot {
        id: "worker-1".into(),
        backend: Backend::Pi,
        project: PathBuf::from("/project"),
        session_locator: None,
        status,
        output: output.map(str::to_owned),
        error: None,
        pending_input: None,
    }
}

#[test]
fn every_worker_status_projects_to_an_agent_lifecycle() {
    let cases = [
        (WorkerStatus::Pending, None, AgentLifecycle::Working),
        (WorkerStatus::Running, None, AgentLifecycle::Working),
        (WorkerStatus::NeedsInput, None, AgentLifecycle::NeedsInput),
        (
            WorkerStatus::Idle,
            Some("done"),
            AgentLifecycle::Completed(AgentOutcome::Complete),
        ),
        (WorkerStatus::Idle, None, AgentLifecycle::Unknown),
        (
            WorkerStatus::Failed,
            None,
            AgentLifecycle::Completed(AgentOutcome::Failed),
        ),
        (
            WorkerStatus::Stopped,
            None,
            AgentLifecycle::Completed(AgentOutcome::Incomplete),
        ),
    ];
    for (status, output, expected) in cases {
        assert_eq!(
            snapshot(status, output).lifecycle(),
            expected,
            "status {status:?} with output {output:?}"
        );
    }
}

#[test]
fn only_terminal_worker_statuses_are_terminal() {
    for status in [
        WorkerStatus::Pending,
        WorkerStatus::Running,
        WorkerStatus::Idle,
        WorkerStatus::NeedsInput,
    ] {
        assert!(!status.terminal(), "{status:?} should not be terminal");
    }
    for status in [WorkerStatus::Failed, WorkerStatus::Stopped] {
        assert!(status.terminal(), "{status:?} should be terminal");
    }
}
