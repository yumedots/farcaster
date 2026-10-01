use super::*;
use std::path::PathBuf;

use crate::repository::JujutsuIdentity;

#[test]
fn sections_partition_every_change_layer() {
    for layer in [
        ChangeLayer::Index,
        ChangeLayer::WorkingTree,
        ChangeLayer::Untracked,
        ChangeLayer::Conflict,
    ] {
        assert_eq!(
            ChangeSection::ALL
                .into_iter()
                .filter(|section| section.matches(layer))
                .count(),
            1,
            "{layer:?}"
        );
    }
    assert!(ChangeSection::Changes.matches(ChangeLayer::WorkingTree));
    assert!(ChangeSection::Changes.matches(ChangeLayer::Untracked));
    assert!(ChangeSection::Staged.matches(ChangeLayer::Index));
    assert!(ChangeSection::Merge.matches(ChangeLayer::Conflict));
}

#[test]
fn list_sort_keys_order_by_path_name_or_status() {
    let sort = |sort: ChangeSort, rows: &[(&str, ChangeKind)]| {
        let mut rows = rows
            .iter()
            .map(|(path, kind)| (Path::new(*path), kind.clone()))
            .collect::<Vec<_>>();
        rows.sort_by_key(|(path, kind)| change_sort_key(sort, path, kind));
        rows.into_iter()
            .map(|(path, _)| path.to_path_buf())
            .collect::<Vec<_>>()
    };
    let rows = [
        ("src/aaa.rs", ChangeKind::Modified),
        ("aaa/zzz.rs", ChangeKind::Added),
    ];

    // Paths sort by the whole relative path, so the shallow folder wins.
    assert_eq!(
        sort(ChangeSort::Path, &rows),
        vec![PathBuf::from("aaa/zzz.rs"), PathBuf::from("src/aaa.rs")]
    );
    // Names ignore the directory and sort by the file alone.
    assert_eq!(
        sort(ChangeSort::Name, &rows),
        vec![PathBuf::from("src/aaa.rs"), PathBuf::from("aaa/zzz.rs")]
    );
    // Statuses group by their letter, so added comes before modified.
    assert_eq!(
        sort(ChangeSort::Status, &rows),
        vec![PathBuf::from("aaa/zzz.rs"), PathBuf::from("src/aaa.rs")]
    );
}

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
fn sync_metadata_uses_nearest_git_branch_and_jj_ancestor_bookmark() {
    assert_eq!(
        repository_sync_metadata(&SnapshotIdentity::Git(GitIdentity {
            nearest_branch: Some("main".into()),
            ahead: 2,
            ..GitIdentity::default()
        })),
        "main · 2 ahead"
    );
    assert_eq!(
        repository_sync_metadata(&SnapshotIdentity::Jujutsu(JujutsuIdentity {
            operation_id: String::new(),
            commit_id: "commit".into(),
            change_id: "change".into(),
            description: String::new(),
            bookmarks: Vec::new(),
            closest_bookmarks: vec!["main".into()],
            ahead: 2,
            conflicted_paths: Vec::new(),
            conflicted: false,
            empty: true,
        })),
        "main · 2 ahead"
    );
}

#[test]
fn unusual_paths_are_reduced_to_one_visible_line() {
    assert_eq!(
        visible_path(Path::new("old\nname\t.rs")),
        "old\\nname\\t.rs"
    );
}
