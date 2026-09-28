use std::{
    fmt,
    path::PathBuf,
    time::{Duration, SystemTime},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RepositorySyncAction {
    PullOrFetch,
    Push,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RepositoryLocation {
    pub(crate) workspace_root: PathBuf,
    pub(crate) project_root: PathBuf,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct GitIdentity {
    pub(crate) head_oid: Option<String>,
    pub(crate) branch: Option<String>,
    pub(crate) upstream: Option<String>,
    pub(crate) nearest_branch: Option<String>,
    pub(crate) ahead: u64,
    pub(crate) behind: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ChangeLayer {
    Index,
    WorkingTree,
    Conflict,
    Untracked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Untracked,
    Conflict,
    Unknown(String),
}

impl ChangeKind {
    pub(crate) fn status_label(&self) -> &str {
        match self {
            Self::Added => "A",
            Self::Modified => "M",
            Self::Deleted => "D",
            Self::Renamed => "R",
            Self::Copied => "C",
            Self::TypeChanged => "T",
            Self::Untracked => "?",
            Self::Conflict => "U",
            Self::Unknown(status) => status,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct DiffTargetKey {
    pub(crate) workspace_root: PathBuf,
    pub(crate) relative_path: PathBuf,
    pub(crate) layer: ChangeLayer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DiffTarget {
    pub(crate) key: DiffTargetKey,
    pub(crate) workspace_root: PathBuf,
    pub(crate) relative_path: PathBuf,
    pub(crate) original_relative_path: Option<PathBuf>,
    pub(crate) layer: ChangeLayer,
    pub(crate) kind: ChangeKind,
    pub(crate) exists: bool,
    pub(super) token: std::sync::Arc<[u8]>,
}

impl DiffTarget {
    pub(crate) fn absolute_path(&self) -> PathBuf {
        self.workspace_root.join(&self.relative_path)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkingCopyChange {
    pub(crate) counts: Option<(usize, usize)>,
    pub(crate) relative_path: PathBuf,
    pub(crate) original_relative_path: Option<PathBuf>,
    pub(crate) layer: ChangeLayer,
    pub(crate) kind: ChangeKind,
    pub(crate) target: DiffTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkingCopySnapshot {
    pub(crate) location: RepositoryLocation,
    pub(crate) identity: GitIdentity,
    pub(crate) changes: Vec<WorkingCopyChange>,
    pub(crate) captured_at: SystemTime,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DiffResult {
    pub(crate) target: DiffTarget,
    pub(crate) patch: String,
    pub(crate) additions: Option<u64>,
    pub(crate) deletions: Option<u64>,
    pub(crate) exists: bool,
}

#[derive(Debug)]
pub(crate) enum RepositoryError {
    Io {
        context: String,
        source: std::io::Error,
    },
    CommandTimedOut {
        program: String,
        timeout: Duration,
    },
    CommandFailed {
        program: String,
        status: Option<i32>,
        stderr: String,
        stderr_truncated: bool,
    },
    OutputTruncated {
        program: String,
    },
    ReaderStalled {
        program: String,
    },
    InvalidRepository(String),
    InvalidOutput {
        detail: String,
    },
    InvalidPath(PathBuf),
    TargetMismatch(String),
    StaleSnapshot,
    SyncUnavailable(String),
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { context, source } => write!(formatter, "{context}: {source}"),
            Self::CommandTimedOut { program, timeout } => {
                write!(formatter, "{program} timed out after {timeout:?}")
            }
            Self::CommandFailed {
                program,
                status,
                stderr,
                stderr_truncated,
            } => {
                let suffix = if *stderr_truncated {
                    " (truncated)"
                } else {
                    ""
                };
                write!(
                    formatter,
                    "{program} exited with {}: {stderr}{suffix}",
                    status.map_or_else(|| "a signal".to_owned(), |code| code.to_string())
                )
            }
            Self::OutputTruncated { program } => {
                write!(formatter, "{program} output exceeded the configured limit")
            }
            Self::ReaderStalled { program } => {
                write!(formatter, "{program} output pipes did not close after exit")
            }
            Self::InvalidRepository(detail) => write!(formatter, "invalid repository: {detail}"),
            Self::InvalidOutput { detail } => write!(formatter, "invalid Git output: {detail}"),
            Self::InvalidPath(path) => {
                write!(formatter, "invalid repository path: {}", path.display())
            }
            Self::TargetMismatch(detail) => write!(formatter, "diff target mismatch: {detail}"),
            Self::StaleSnapshot => write!(
                formatter,
                "working copy changed; refresh before loading diff"
            ),
            Self::SyncUnavailable(detail) => formatter.write_str(detail),
        }
    }
}

impl std::error::Error for RepositoryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
