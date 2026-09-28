//! The process-wide handle to the worker pool.
//!
//! Every chat, its child sessions and each session family are served by one
//! pool, so the app keeps a handle to it here rather than threading it through
//! the view tree. The runtime asks this module to stop a family when its root
//! session goes away.

use std::{path::PathBuf, sync::Mutex};

use crate::agents::{Backend, WorkerPool, WorkerSnapshot};

static POOL: Mutex<Option<WorkerPool>> = Mutex::new(None);

#[cfg(test)]
static TEST_POOL: Mutex<Option<WorkerPool>> = Mutex::new(None);

#[cfg(test)]
static TEST_POOL_LOCK: Mutex<()> = Mutex::new(());

/// Installs the pool the app was started with.
pub(crate) fn install(workers: WorkerPool) -> Result<(), String> {
    let mut current = POOL.lock().map_err(|_| "worker pool is unavailable")?;
    *current = Some(workers);
    Ok(())
}

pub(crate) fn set_worker_app_proxy(proxy: Option<String>) -> Result<(), String> {
    #[cfg(test)]
    if let Some(workers) = test_pool()? {
        return workers.set_app_proxy(proxy);
    }
    let current = POOL.lock().map_err(|_| "worker pool is unavailable")?;
    match current.as_ref() {
        Some(workers) => workers.set_app_proxy(proxy),
        None => Ok(()),
    }
}

pub(crate) fn stop_session_family_workers(
    project: &std::path::Path,
    sessions: &[(Backend, PathBuf)],
) -> Result<usize, String> {
    #[cfg(test)]
    if let Some(workers) = test_pool()? {
        return workers.stop_session_family(project, sessions);
    }
    let current = POOL.lock().map_err(|_| "worker pool is unavailable")?;
    match current.as_ref() {
        Some(workers) => workers.stop_session_family(project, sessions),
        None => Ok(0),
    }
}

pub(crate) fn finish_session_family_worker_stop(
    project: &std::path::Path,
    sessions: &[(Backend, PathBuf)],
) -> Result<(), String> {
    #[cfg(test)]
    if let Some(workers) = test_pool()? {
        return workers.finish_session_family_stop(project, sessions);
    }
    let current = POOL.lock().map_err(|_| "worker pool is unavailable")?;
    match current.as_ref() {
        Some(workers) => workers.finish_session_family_stop(project, sessions),
        None => Ok(()),
    }
}

pub(crate) fn worker_snapshots() -> Result<Vec<WorkerSnapshot>, String> {
    let current = POOL.lock().map_err(|_| "worker pool is unavailable")?;
    match current.as_ref() {
        Some(workers) => workers.snapshots(),
        None => Ok(Vec::new()),
    }
}

#[cfg(test)]
fn test_pool() -> Result<Option<WorkerPool>, String> {
    Ok(TEST_POOL
        .lock()
        .map_err(|_| "test worker pool is unavailable")?
        .clone())
}

/// Runs `test` with a pool installed for the whole process, for the runtime
/// tests that exercise family commands without an app.
#[cfg(test)]
pub(crate) fn with_test_worker_pool<T>(workers: WorkerPool, test: impl FnOnce() -> T) -> T {
    let _serial = TEST_POOL_LOCK.lock().expect("test worker pool lock");
    *TEST_POOL.lock().expect("test worker pool") = Some(workers);
    struct Clear;
    impl Drop for Clear {
        fn drop(&mut self) {
            *TEST_POOL.lock().expect("test worker pool") = None;
        }
    }
    let _clear = Clear;
    test()
}
