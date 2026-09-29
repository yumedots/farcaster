use super::*;
use crate::agents::Backend;
use crate::agents::{WorkerInput, WorkerInputResponse};
use std::sync::mpsc;

const INPUT_PREFIX: &str = "farcaster-worker-input-";

pub(crate) fn is_child_input_id(id: &str) -> bool {
    id.starts_with(INPUT_PREFIX)
}

pub(super) struct PendingInput {
    pub(super) parent_id: String,
    input: WorkerInput,
    original_id: String,
    delivered: bool,
    responses: mpsc::Sender<WorkerInputResponse>,
}

pub(super) struct ExpiredInput {
    parent: CallerSession,
    id: String,
}

impl CallerRegistry {
    pub(crate) fn take_child_inputs(
        &self,
        project: &Path,
        backend: Backend,
        session: &str,
    ) -> Vec<WorkerInput> {
        let project = canonical_project(project);
        let Ok(callers) = self.callers.lock() else {
            return Vec::new();
        };
        let Some(parent) = callers.values().find(|caller| {
            caller.project == project
                && caller.backend == backend
                && caller.session.as_deref() == Some(session)
                && caller.parent_worker_id.is_none()
        }) else {
            return Vec::new();
        };
        let Ok(mut inputs) = self.inputs.lock() else {
            return Vec::new();
        };
        inputs
            .iter_mut()
            .filter_map(|pending| {
                if pending.parent_id != parent.worker_id || pending.delivered {
                    return None;
                }
                pending.delivered = true;
                Some(pending.input.clone())
            })
            .collect()
    }

    pub(crate) fn respond_to_child_input(
        &self,
        mut response: WorkerInputResponse,
    ) -> Result<(), String> {
        let mut inputs = self
            .inputs
            .lock()
            .map_err(|_| "worker input registry is unavailable")?;
        let index = inputs
            .iter()
            .position(|pending| pending.input.id == response.id)
            .ok_or("worker input request is no longer available")?;
        let pending = inputs.remove(index);
        response.id = pending.original_id;
        pending
            .responses
            .send(response)
            .map_err(|_| "worker is no longer available".into())
    }

    pub(crate) fn take_expired_child_inputs(
        &self,
        project: &Path,
        backend: Backend,
        session: &str,
    ) -> Vec<String> {
        let parent = CallerSession {
            project: canonical_project(project),
            backend: backend.to_owned(),
            session: session.to_owned(),
        };
        let Ok(mut expired) = self.expired_inputs.lock() else {
            return Vec::new();
        };
        let mut ids = Vec::new();
        let mut index = 0;
        while index < expired.len() {
            if expired[index].parent == parent {
                ids.push(expired.remove(index).id);
            } else {
                index += 1;
            }
        }
        ids
    }
}
