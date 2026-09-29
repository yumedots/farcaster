use crate::agents::Backend;
use crate::modules::sessions::activity::{AgentLifecycle, AgentOutcome};
use std::path::PathBuf;

use serde::Serialize;

use super::WorkerInput;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PeerMessage {
    pub(crate) from: String,
    pub(crate) message: String,
}

impl PeerMessage {
    const PROMPT_PREFIX: &'static str = "Message from Farcaster worker ";
    const LEGACY_PROMPT_PREFIX: &'static str = "Message from Farcaster peer ";

    pub(crate) fn from_prompt(prompt: &str) -> Option<Self> {
        let (heading, message) = prompt.split_once("\n\n")?;
        let from = [Self::PROMPT_PREFIX, Self::LEGACY_PROMPT_PREFIX]
            .into_iter()
            .find_map(|prefix| heading.strip_prefix(prefix))?
            .strip_suffix(':')?;
        if !valid_worker_name(from) {
            return None;
        }
        Some(Self {
            from: from.to_owned(),
            message: message.to_owned(),
        })
    }
}

pub(crate) fn valid_worker_name(name: &str) -> bool {
    (1..=48).contains(&name.len())
        && name.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && matches!(byte, b'-' | b'_'))
        })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorkerStatus {
    Idle,
    Stopped,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkerSnapshot {
    pub(crate) id: String,
    pub(crate) backend: Backend,
    pub(crate) project: PathBuf,
    pub(crate) session_locator: Option<String>,
    pub(crate) status: WorkerStatus,
    pub(crate) output: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) pending_input: Option<WorkerInput>,
}

impl WorkerSnapshot {
    pub(crate) fn lifecycle(&self) -> AgentLifecycle {
        match self.status {
            WorkerStatus::Idle if self.output.is_some() => {
                AgentLifecycle::Completed(AgentOutcome::Complete)
            }
            WorkerStatus::Idle => AgentLifecycle::Unknown,
            WorkerStatus::Stopped => AgentLifecycle::Completed(AgentOutcome::Incomplete),
        }
    }
}

#[cfg(test)]
#[path = "workers_tests.rs"]
mod tests;
