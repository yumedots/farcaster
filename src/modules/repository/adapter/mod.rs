pub(super) mod git;
mod git_edit;
pub(super) mod process;
pub(super) mod watcher;

use std::{ffi::OsString, path::Path, sync::Arc, time::Duration};

use self::process::ProcessExecutor;
use super::{
    RepositoryBackend, RepositoryError,
    core::{discover_location, executable_available, port::RepositoryOperations},
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(8);
const DEFAULT_SYNC_TIMEOUT: Duration = Duration::from_secs(120);
const DEFAULT_OUTPUT_LIMIT: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(super) struct RepositoryOptions {
    pub(in crate::modules::repository) git_executable: OsString,
    pub(in crate::modules::repository) timeout: Duration,
    pub(in crate::modules::repository) sync_timeout: Duration,
    pub(in crate::modules::repository) output_limit: usize,
    pub(in crate::modules::repository) environment: Vec<(OsString, OsString)>,
}

impl Default for RepositoryOptions {
    fn default() -> Self {
        Self {
            git_executable: std::env::var_os("FARCASTER_GIT")
                .unwrap_or_else(|| OsString::from("git")),
            timeout: DEFAULT_TIMEOUT,
            sync_timeout: DEFAULT_SYNC_TIMEOUT,
            output_limit: DEFAULT_OUTPUT_LIMIT,
            environment: Vec::new(),
        }
    }
}

impl RepositoryBackend {
    pub(crate) fn discover(project: &Path) -> Result<Option<Self>, RepositoryError> {
        Self::discover_with_options(project, RepositoryOptions::default())
    }

    pub(super) fn discover_with_options(
        project: &Path,
        options: RepositoryOptions,
    ) -> Result<Option<Self>, RepositoryError> {
        let git_available = executable_available(&options.git_executable);
        let Some(location) = discover_location(project, git_available)? else {
            return Ok(None);
        };
        let operations: Arc<dyn RepositoryOperations> = Arc::new(git::GitOperations);
        let executor = ProcessExecutor::new(
            options.git_executable.clone(),
            location.workspace_root.clone(),
            options.timeout,
            options.sync_timeout,
            options.output_limit,
            options.environment,
        );
        Ok(Some(Self::new(location, Arc::new(executor), operations)))
    }
}
