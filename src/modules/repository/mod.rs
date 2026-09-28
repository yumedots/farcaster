mod adapter;
mod contract;
mod core;

pub(crate) use adapter::watcher::{RepositoryWatchEvent, RepositoryWatcher};
pub(crate) use contract::{
    ChangeKind, ChangeLayer, DiffResult, DiffTarget, DiffTargetKey, GitIdentity, RepositoryError,
    RepositoryLocation, RepositorySyncAction, WorkingCopyChange, WorkingCopySnapshot,
};

pub(crate) use core::{
    DiffHunk, DiffLine, DiffLineKind, FileDiff, HunkApply, RepositoryBackend, RepositoryEdit,
    RepositoryEditReview, SplitRow,
};

use core::{change, command_failed, require_complete_stdout};

use core::diff_result;

#[cfg(test)]
use adapter::RepositoryOptions;
#[cfg(test)]
use core::patch_counts;
#[cfg(test)]
mod tests;
