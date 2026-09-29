use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::agents::{Backend, HarnessAccessMode};

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkerExecution {
    pub(crate) harness: Backend,
    pub(crate) provider: String,
    pub(crate) model: String,
    pub(crate) effort: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct WorkerAssignment {
    pub(crate) profile: String,
    pub(crate) execution: WorkerExecution,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub(crate) struct WorkerFamilyLink {
    pub(crate) project: PathBuf,
    pub(crate) child_backend: Backend,
    pub(crate) child_session: String,
    pub(crate) parent_backend: Backend,
    pub(crate) parent_session: String,
    #[serde(default)]
    pub(crate) execution: Option<WorkerExecution>,
    #[serde(default)]
    pub(crate) routing: Option<WorkerRouting>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub(crate) struct WorkerRouting {
    pub(crate) name: String,
    pub(crate) assignment: WorkerAssignment,
    pub(crate) access_mode: HarnessAccessMode,
}
