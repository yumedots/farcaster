use std::path::PathBuf;

use crate::agents::Backend;
use crate::modules::sessions::activity::{AgentLifecycle, AgentOutcome};

use super::*;

#[test]
fn peer_prompt_reads_structured_origin() {
    assert_eq!(
        PeerMessage::from_prompt(
            "Message from Farcaster worker diff-review:\n\nreview complete\nwith details"
        ),
        Some(PeerMessage {
            from: "diff-review".into(),
            message: "review complete\nwith details".into(),
        })
    );
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
        (
            WorkerStatus::Idle,
            Some("done"),
            AgentLifecycle::Completed(AgentOutcome::Complete),
        ),
        (WorkerStatus::Idle, None, AgentLifecycle::Unknown),
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
