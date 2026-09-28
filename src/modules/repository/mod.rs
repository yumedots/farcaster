mod adapter;
mod contract;
mod core;

pub(crate) use adapter::watcher::{RepositoryWatchEvent, RepositoryWatcher};
pub(crate) use contract::{
    ChangeKind, ChangeLayer, DiffTargetKey, GitIdentity, RepositoryError, RepositoryLocation,
    RepositorySyncAction, WorkingCopyChange, WorkingCopySnapshot,
};
#[cfg(test)]
pub(crate) use contract::{DiffResult, DiffTarget};

pub(crate) use core::{RepositoryBackend, RepositoryEdit, RepositoryEditReview};

use core::{change, command_failed, require_complete_stdout};

#[cfg(test)]
use adapter::RepositoryOptions;
#[cfg(test)]
use core::{diff_result, patch_counts};
#[cfg(test)]
mod tests;
