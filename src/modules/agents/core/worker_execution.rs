use crate::agents::Backend;
use serde::{Deserialize, Serialize};

/// One concrete harness/provider/model a child worker can be started with.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkerExecution {
    pub(crate) harness: Backend,
    pub(crate) provider: String,
    pub(crate) model: String,
    pub(crate) effort: Option<String>,
}

/// The requested worker name plus the execution that was reserved for it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct WorkerAssignment {
    pub(crate) profile: String,
    pub(crate) execution: WorkerExecution,
}
