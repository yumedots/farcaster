use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::*;
use crate::agents::{
    Backend, HarnessAccessMode, WorkerFamilyLink, WorkerLaunch, WorkerRouting, WorkerSession,
    WorkerSessionFactory,
};
use crate::modules::agents::contract::WorkerStatus;

struct UnusedFactory;

impl WorkerSessionFactory for UnusedFactory {
    fn create(&self, _launch: WorkerLaunch) -> Result<Box<dyn WorkerSession>, String> {
        Err("no worker can be started".into())
    }
}

fn assignment() -> WorkerAssignment {
    WorkerAssignment {
        profile: "test-profile".into(),
        execution: WorkerExecution {
            harness: Backend::Pi,
            provider: "test-provider".into(),
            model: "test-model".into(),
            effort: None,
        },
    }
}

fn family(project: &Path, child: &str, parent: &str) -> WorkerFamilyLink {
    WorkerFamilyLink {
        project: project.to_owned(),
        child_backend: Backend::Pi,
        child_session: child.into(),
        parent_backend: Backend::Pi,
        parent_session: parent.into(),
        execution: Some(WorkerExecution {
            harness: Backend::Pi,
            provider: "test-provider".into(),
            model: "test-model".into(),
            effort: None,
        }),
        routing: Some(WorkerRouting {
            name: child.to_owned(),
            assignment: assignment(),
            access_mode: HarnessAccessMode::Auto,
        }),
    }
}

fn pool() -> Result<(WorkerPool, async_channel::Receiver<()>), String> {
    let factory: Arc<dyn WorkerSessionFactory> = Arc::new(UnusedFactory);
    let pool = WorkerPool::new(BTreeMap::from([(Backend::Pi, factory)]))?;
    let updates = pool.updates();
    Ok((pool, updates))
}

#[test]
fn saved_routes_are_restored_as_idle_workers() -> Result<(), String> {
    let temp = tempfile::tempdir().map_err(|error| error.to_string())?;
    let project = temp
        .path()
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let (pool, _updates) = pool()?;
    pool.restore_families([family(&project, "implementation", "parent")])?;
    let snapshots = pool.snapshots()?;
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].status, WorkerStatus::Idle);
    assert_eq!(snapshots[0].backend, Backend::Pi);
    assert_eq!(snapshots[0].project, project);
    assert_eq!(
        snapshots[0].session_locator.as_deref(),
        Some("implementation")
    );
    Ok(())
}

#[test]
fn a_route_without_a_saved_name_is_ignored() -> Result<(), String> {
    let temp = tempfile::tempdir().map_err(|error| error.to_string())?;
    let project = temp
        .path()
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let (pool, _updates) = pool()?;
    let mut saved = family(&project, "implementation", "parent");
    saved.routing = None;
    pool.restore_families([saved])?;
    assert!(pool.snapshots()?.is_empty());
    Ok(())
}

#[test]
fn a_route_for_a_backend_without_a_factory_is_ignored() -> Result<(), String> {
    let temp = tempfile::tempdir().map_err(|error| error.to_string())?;
    let project = temp
        .path()
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let (pool, _updates) = pool()?;
    let mut saved = family(&project, "implementation", "parent");
    saved.child_backend = Backend::Codex;
    pool.restore_families([saved])?;
    assert!(pool.snapshots()?.is_empty());
    Ok(())
}

#[test]
fn an_ambiguous_saved_route_is_refused() -> Result<(), String> {
    let temp = tempfile::tempdir().map_err(|error| error.to_string())?;
    let project = temp
        .path()
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let (pool, _updates) = pool()?;
    pool.restore_families([family(&project, "implementation", "parent")])?;
    let error = pool
        .restore_families([family(&project, "implementation", "parent")])
        .expect_err("a duplicate saved route must be refused");
    assert!(error.contains("ambiguous"), "{error}");
    assert_eq!(pool.snapshots()?.len(), 1);
    Ok(())
}

#[test]
fn stopping_a_session_family_marks_its_children_stopped() -> Result<(), String> {
    let temp = tempfile::tempdir().map_err(|error| error.to_string())?;
    let project = temp
        .path()
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let (pool, updates) = pool()?;
    pool.restore_families([
        family(&project, "implementation", "parent"),
        family(&project, "review", "implementation"),
        family(&project, "untouched", "elsewhere"),
    ])?;
    let stopped = pool.stop_session_family(&project, &[(Backend::Pi, PathBuf::from("parent"))])?;
    assert_eq!(stopped, 2);
    let mut statuses = pool
        .snapshots()?
        .into_iter()
        .map(|snapshot| {
            (
                snapshot.session_locator.unwrap_or_default(),
                snapshot.status,
            )
        })
        .collect::<Vec<_>>();
    statuses.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(
        statuses,
        vec![
            ("implementation".to_owned(), WorkerStatus::Stopped),
            ("review".to_owned(), WorkerStatus::Stopped),
            ("untouched".to_owned(), WorkerStatus::Idle),
        ]
    );
    assert!(
        updates.try_recv().is_ok(),
        "the pool must announce the stop on its update channel"
    );
    Ok(())
}

#[test]
fn stopping_an_unknown_session_stops_nothing() -> Result<(), String> {
    let temp = tempfile::tempdir().map_err(|error| error.to_string())?;
    let project = temp
        .path()
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let (pool, updates) = pool()?;
    pool.restore_families([family(&project, "implementation", "parent")])?;
    let stopped = pool.stop_session_family(&project, &[(Backend::Pi, PathBuf::from("other"))])?;
    assert_eq!(stopped, 0);
    assert_eq!(pool.snapshots()?[0].status, WorkerStatus::Idle);
    assert!(updates.try_recv().is_err());
    Ok(())
}

#[test]
fn a_finished_family_stop_lifts_the_fence() -> Result<(), String> {
    let temp = tempfile::tempdir().map_err(|error| error.to_string())?;
    let project = temp
        .path()
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let (pool, _updates) = pool()?;
    let sessions = [(Backend::Pi, PathBuf::from("parent"))];
    pool.restore_families([family(&project, "implementation", "parent")])?;
    pool.fence_family(&super::caller::CallerContext {
        worker_id: "worker-1".into(),
        worker_name: "parent".into(),
        project: project.clone(),
        session: "parent".into(),
        backend: Backend::Pi,
        provider: None,
        model: None,
        effort: None,
        access_mode: HarnessAccessMode::Auto,
        parent_worker_id: None,
    })?;
    pool.finish_session_family_stop(&project, &sessions)?;
    assert_eq!(pool.snapshots()?.len(), 1);
    Ok(())
}
