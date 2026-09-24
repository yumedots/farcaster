mod adapter;
mod contract;
mod core;
mod domain;

pub(crate) use adapter::watcher::{RepositoryWatchEvent, RepositoryWatcher};
pub(crate) use contract::{
    BackendPreference, ChangeKind, ChangeLayer, DiffTargetKey, GitIdentity, JujutsuIdentity,
    RepositoryError, RepositoryKind, RepositoryLocation, RepositorySyncAction, SnapshotIdentity,
    WorkingCopyChange, WorkingCopySnapshot,
};
#[cfg(test)]
pub(crate) use contract::{DiffResult, DiffTarget};

pub(crate) use core::{
    PreferenceStore, RepositoryBackend, RepositoryEdit, RepositoryEditReview, load_preferences,
    save_preferences,
};

use core::{change, command_failed, require_complete_stdout};
use domain::SnapshotToken;

#[cfg(test)]
use adapter::RepositoryOptions;
#[cfg(test)]
use core::{diff_result, patch_counts};
#[cfg(test)]
mod tests;
