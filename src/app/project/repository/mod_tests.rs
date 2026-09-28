use super::*;
use crate::repository::GitIdentity;

#[test]
fn refresh_gate_publishes_during_continuous_changes_and_coalesces_requests() {
    let mut gate = RefreshGate::default();
    let first = gate.request().expect("first refresh should start");
    assert!(gate.request().is_none());

    let completion = gate.finish(first).expect("active refresh should finish");
    assert!(completion.publish);
    let second = completion.next.expect("pending refresh should start");
    assert_ne!(first, second);

    assert!(gate.request().is_none());
    assert!(gate.request().is_none());
    let completion = gate.finish(second).expect("latest refresh should finish");
    assert!(completion.publish);
    let third = completion.next.expect("changes need one more refresh");
    let completion = gate.finish(third).expect("last refresh should finish");
    assert!(completion.publish);
    assert!(completion.next.is_none());
    assert!(gate.finish(first).is_none());
}

#[test]
fn display_equality_ignores_snapshot_capture_time() {
    let snapshot = snapshot(None);
    let mut later = snapshot.clone();
    later.captured_at = std::time::SystemTime::now();

    assert!(displayed_snapshot_eq(&snapshot, &later));
}

#[test]
fn invalidation_rejects_in_flight_work_without_starting_another_command() {
    let mut gate = RefreshGate::default();
    let generation = gate.request().expect("refresh should start");
    gate.invalidate();
    let completion = gate.finish(generation).expect("refresh should finish");
    assert!(!completion.publish);
    assert!(completion.next.is_none());
}

#[test]
fn project_change_rejects_old_scan_and_starts_pending_scan() {
    let mut gate = RefreshGate::default();
    let old = gate.request().expect("test operation should succeed");
    gate.invalidate();
    assert!(gate.request().is_none());
    let completion = gate.finish(old).expect("test operation should succeed");
    assert!(!completion.publish);
    let current = completion.next.expect("test operation should succeed");
    assert!(
        gate.finish(current)
            .expect("test operation should succeed")
            .publish
    );
}

fn snapshot(branch: Option<&str>) -> WorkingCopySnapshot {
    WorkingCopySnapshot {
        location: RepositoryLocation {
            workspace_root: PathBuf::from("/workspace"),
            project_root: PathBuf::from("/workspace/project"),
        },
        identity: GitIdentity {
            branch: branch.map(str::to_owned),
            ..GitIdentity::default()
        },
        changes: Vec::new(),
        captured_at: std::time::SystemTime::UNIX_EPOCH,
    }
}

fn cached_observation() -> RepositoryObservation {
    RepositoryObservation {
        backend: None,
        snapshot: Some(snapshot(None)),
        additions: Some(2),
        deletions: Some(1),
    }
}

#[test]
fn switching_projects_reuses_the_working_copy_that_was_observed_for_them() {
    let mut cache = ObservationCache::default();
    let first = PathBuf::from("/first");
    let second = PathBuf::from("/second");
    cache.remember(first.clone(), cached_observation());

    let reused = cache
        .reuse(&first)
        .expect("the first project keeps its working copy");
    assert!(reused.snapshot.is_some());
    assert_eq!(reused.additions, Some(2));
    assert_eq!(reused.deletions, Some(1));

    assert!(cache.reuse(&first).is_none());
    assert!(cache.reuse(&second).is_none());
}

#[test]
fn a_scan_with_nothing_to_show_is_never_remembered() {
    assert!(
        RepositoryObservation::from_scan(Ok(None)).is_none(),
        "a project without a working copy has nothing to show ahead of a switch"
    );
    assert!(
        RepositoryObservation::from_scan(Err(RepositoryError::CommandTimedOut {
            program: "git".into(),
            timeout: std::time::Duration::from_secs(8),
        }))
        .is_none()
    );
}
