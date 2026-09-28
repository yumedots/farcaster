use super::*;

#[test]
fn file_labels_keep_names_and_parent_paths_distinct() {
    assert_eq!(
        file_path_labels(Path::new("src/app/theme.rs")),
        ("theme.rs".into(), "src/app".into())
    );
    assert_eq!(
        file_path_labels(Path::new("Cargo.toml")),
        ("Cargo.toml".into(), String::new())
    );
    assert_eq!(
        file_path_labels(Path::new("src/نام\n.rs")),
        ("نام\\n.rs".into(), "src".into())
    );
}

#[test]
fn unborn_and_detached_git_heads_are_explicit() {
    assert_eq!(
        git_identity(&GitIdentity {
            branch: Some("main".into()),
            ..GitIdentity::default()
        }),
        "main · unborn"
    );
    assert_eq!(
        git_identity(&GitIdentity {
            head_oid: Some("0123456789abcdef".into()),
            ..GitIdentity::default()
        }),
        "detached 01234567"
    );
}

#[test]
fn sync_metadata_uses_the_nearest_branch_with_ahead_and_behind_counts() {
    assert_eq!(
        repository_sync_metadata(&GitIdentity {
            nearest_branch: Some("main".into()),
            ahead: 2,
            ..GitIdentity::default()
        }),
        "main · 2 ahead"
    );
    assert_eq!(
        repository_sync_metadata(&GitIdentity::default()),
        "detached"
    );
}

#[test]
fn unusual_paths_are_reduced_to_one_visible_line() {
    assert_eq!(
        visible_path(Path::new("old\nname\t.rs")),
        "old\\nname\\t.rs"
    );
}
