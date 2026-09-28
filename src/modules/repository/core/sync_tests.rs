use super::*;

#[test]
fn git_sync_preserves_branch_and_upstream_mapping() {
    let identity = GitIdentity {
        branch: Some("feature/sidebar".into()),
        upstream: Some("origin/review/sidebar".into()),
        ..GitIdentity::default()
    };

    assert_eq!(
        strings(&identity, RepositorySyncAction::PullOrFetch),
        ["pull", "--ff-only", "--", "origin", "review/sidebar"]
    );
    assert_eq!(
        strings(&identity, RepositorySyncAction::Push),
        ["push", "--", "origin", "feature/sidebar:review/sidebar"]
    );
}

#[test]
fn sync_is_unavailable_without_a_branch_or_upstream() {
    let detached = GitIdentity {
        branch: None,
        ..GitIdentity::default()
    };
    assert!(!RepositorySyncAction::Push.is_available_for(&detached));

    let unpublished = GitIdentity {
        branch: Some("main".into()),
        ..GitIdentity::default()
    };
    assert!(!RepositorySyncAction::Push.is_available_for(&unpublished));
}

fn strings(identity: &GitIdentity, action: RepositorySyncAction) -> Vec<String> {
    arguments(identity, action)
        .expect("sync arguments")
        .into_iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect()
}
