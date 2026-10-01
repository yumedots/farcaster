use super::*;
use std::collections::BTreeSet;

struct EditRepo {
    temp: TestDirectory,
    backend: RepositoryBackend,
}

impl EditRepo {
    fn new(kind: RepositoryKind) -> Self {
        let temp = TestDirectory::new("edit");
        let root = temp.path().join("repo");
        let home = temp.path().join("home");
        let config = temp.path().join("config");
        fs::create_dir_all(&root).expect("test operation should succeed");
        fs::create_dir_all(&home).expect("test operation should succeed");
        fs::create_dir_all(&config).expect("test operation should succeed");
        let preference = match kind {
            RepositoryKind::Git => {
                run_git(&root, &home, &config, &["init"]);
                run_git(
                    &root,
                    &home,
                    &config,
                    &["config", "user.name", "Review Test"],
                );
                run_git(
                    &root,
                    &home,
                    &config,
                    &["config", "user.email", "review@example.invalid"],
                );
                BackendPreference::Git
            }
            RepositoryKind::Jujutsu => {
                run_jj(&root, &home, &config, &["git", "init"]);
                BackendPreference::Jujutsu
            }
        };
        let options = RepositoryOptions {
            environment: isolated_environment(&home, &config),
            ..RepositoryOptions::default()
        };
        let backend = RepositoryBackend::discover_with_options(&root, preference, options)
            .expect("test operation should succeed")
            .expect("test operation should succeed");
        Self { temp, backend }
    }

    fn root(&self) -> &Path {
        &self.backend.location.workspace_root
    }
    fn write(&self, path: &str, text: impl AsRef<[u8]>) {
        fs::write(self.root().join(path), text).expect("test operation should succeed");
    }
    fn read(&self, path: &str) -> String {
        fs::read_to_string(self.root().join(path)).expect("test operation should succeed")
    }

    fn command(&self, args: &[&str]) -> String {
        let output = Command::new(self.backend.executable())
            .args(args)
            .current_dir(self.root())
            .envs(isolated_environment(
                &self.temp.path().join("home"),
                &self.temp.path().join("config"),
            ))
            .output()
            .expect("test operation should succeed");
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("test operation should succeed")
    }

    fn review(&self, paths: &[&str]) -> RepositoryEditReview {
        let snapshot = self
            .backend
            .snapshot()
            .expect("test operation should succeed");
        self.backend
            .prepare_edit(&snapshot, &paths.iter().map(PathBuf::from).collect())
            .expect("test operation should succeed")
    }

    fn base(&self) {
        self.write("selected", "base\n");
        self.write("other", "base\n");
        if self.backend.location.kind == RepositoryKind::Git {
            self.command(&["add", "."]);
        }
        self.command(&["commit", "-m", "base"]);
    }

    fn target(&self, path: &str, layer: ChangeLayer) -> DiffTarget {
        self.backend
            .snapshot()
            .expect("test operation should succeed")
            .changes
            .into_iter()
            .find(|change| change.relative_path == Path::new(path) && change.layer == layer)
            .map(|change| change.target)
            .expect("change in the snapshot")
    }

    /// A committed file with two changes far enough apart to land in separate
    /// hunks, one on line 2 and one on line 19.
    fn two_hunk_file(&self, path: &str) {
        let lines = (1..=24)
            .map(|line| format!("line {line}\n"))
            .collect::<String>();
        self.write(path, &lines);
        self.command(&["add", path]);
        self.command(&["commit", "-m", "wide"]);
        let edited = lines
            .replace("line 2\n", "line two\n")
            .replace("line 19\n", "line nineteen\n");
        self.write(path, &edited);
    }
}

#[test]
fn git_commit_selected_includes_working_contents_and_preserves_other_staging() {
    let repo = EditRepo::new(RepositoryKind::Git);
    repo.base();
    repo.write("selected", "staged\n");
    repo.write("other", "other staged\n");
    repo.command(&["add", "."]);
    repo.write("selected", "working\n");
    repo.write("other", "other working\n");
    repo.write(":(glob)* new", "new\n");
    let review = repo.review(&["selected", ":(glob)* new"]);
    repo.backend
        .apply_edit(&review, RepositoryEdit::Commit, "feat: chosen files")
        .expect("test operation should succeed");
    assert_eq!(repo.command(&["show", "HEAD:selected"]), "working\n");
    assert_eq!(repo.command(&["show", "HEAD::(glob)* new"]), "new\n");
    assert_eq!(repo.command(&["show", "HEAD:other"]), "base\n");
    assert_eq!(repo.command(&["show", ":other"]), "other staged\n");
    assert_eq!(repo.read("other"), "other working\n");
    assert_eq!(
        repo.command(&["diff", "--cached", "--name-only"]),
        "other\n"
    );
}

#[test]
fn git_review_rejects_same_status_binary_edits_and_empty_message() {
    let repo = EditRepo::new(RepositoryKind::Git);
    repo.base();
    repo.write("selected", b"\0one");
    let review = repo.review(&["selected"]);
    for action in [RepositoryEdit::Commit, RepositoryEdit::CommitIndex] {
        assert!(
            repo.backend.apply_edit(&review, action, " \n").is_err(),
            "{action:?}"
        );
    }
    repo.write("selected", b"\0two");
    for action in [
        RepositoryEdit::Commit,
        RepositoryEdit::CommitIndex,
        RepositoryEdit::Discard,
        RepositoryEdit::Stage,
        RepositoryEdit::Unstage,
    ] {
        assert!(
            matches!(
                repo.backend.apply_edit(&review, action, "message"),
                Err(RepositoryError::StaleSnapshot)
            ),
            "{action:?}"
        );
    }
    assert_eq!(
        fs::read(repo.root().join("selected")).expect("test operation should succeed"),
        b"\0two"
    );
}

#[test]
fn git_discard_restores_both_layers_and_does_not_touch_other_files() {
    let repo = EditRepo::new(RepositoryKind::Git);
    repo.base();
    repo.write("selected", "staged\n");
    repo.command(&["add", "selected"]);
    repo.write("selected", "working\n");
    repo.write("other", "keep\n");
    let review = repo.review(&["selected"]);
    repo.backend
        .apply_edit(&review, RepositoryEdit::Discard, "")
        .expect("test operation should succeed");
    assert_eq!(repo.read("selected"), "base\n");
    assert_eq!(repo.command(&["show", ":selected"]), "base\n");
    assert_eq!(repo.read("other"), "keep\n");
    fs::remove_file(repo.root().join("selected")).expect("test operation should succeed");
    repo.backend
        .apply_edit(&repo.review(&["selected"]), RepositoryEdit::Discard, "")
        .expect("test operation should succeed");
    assert_eq!(repo.read("selected"), "base\n");
}

#[test]
fn git_discard_handles_renames_and_new_files() {
    let repo = EditRepo::new(RepositoryKind::Git);
    repo.base();
    repo.command(&["mv", "selected", "renamed"]);
    let review = repo.review(&["renamed"]);
    assert!(review.paths().contains(&PathBuf::from("selected")));
    repo.backend
        .apply_edit(&review, RepositoryEdit::Discard, "")
        .expect("test operation should succeed");
    assert_eq!(repo.read("selected"), "base\n");
    assert!(!repo.root().join("renamed").exists());
    for staged in [false, true] {
        repo.write("new", "new\n");
        if staged {
            repo.command(&["add", "new"]);
        }
        repo.backend
            .apply_edit(&repo.review(&["new"]), RepositoryEdit::Discard, "")
            .expect("test operation should succeed");
        assert!(!repo.root().join("new").exists());
    }
}

#[test]
fn git_stage_moves_every_kind_of_working_change_into_the_index() {
    let repo = EditRepo::new(RepositoryKind::Git);
    repo.base();
    repo.write("selected", "staged\n");
    repo.write("fresh", "new\n");
    fs::remove_file(repo.root().join("other")).expect("test operation should succeed");

    let review = repo.review(&["selected", "fresh", "other"]);
    repo.backend
        .apply_edit(&review, RepositoryEdit::Stage, "")
        .expect("test operation should succeed");

    let snapshot = repo
        .backend
        .snapshot()
        .expect("test operation should succeed");
    for (path, kind) in [
        ("selected", ChangeKind::Modified),
        ("fresh", ChangeKind::Added),
        ("other", ChangeKind::Deleted),
    ] {
        let change = snapshot
            .changes
            .iter()
            .find(|change| change.relative_path == Path::new(path))
            .expect("staged row");
        assert_eq!(change.layer, ChangeLayer::Index, "{path}");
        assert_eq!(change.kind, kind, "{path}");
    }
    assert_eq!(snapshot.changes.len(), 3);
    assert_eq!(
        repo.command(&["diff", "--cached", "--name-only"]),
        "fresh\nother\nselected\n"
    );
    assert_eq!(repo.command(&["diff", "--name-only"]), "");

    let review = repo.review(&["selected", "fresh", "other"]);
    repo.backend
        .apply_edit(&review, RepositoryEdit::Unstage, "")
        .expect("test operation should succeed");

    let snapshot = repo
        .backend
        .snapshot()
        .expect("test operation should succeed");
    for (path, layer, kind) in [
        ("selected", ChangeLayer::WorkingTree, ChangeKind::Modified),
        ("fresh", ChangeLayer::Untracked, ChangeKind::Untracked),
        ("other", ChangeLayer::WorkingTree, ChangeKind::Deleted),
    ] {
        let change = snapshot
            .changes
            .iter()
            .find(|change| change.relative_path == Path::new(path))
            .expect("unstaged row");
        assert_eq!(change.layer, layer, "{path}");
        assert_eq!(change.kind, kind, "{path}");
    }
    assert_eq!(repo.command(&["diff", "--cached", "--name-only"]), "");
    assert_eq!(repo.read("selected"), "staged\n");
    assert!(!repo.root().join("other").exists());
}

#[test]
fn git_stage_collapses_a_file_that_is_staged_and_modified_again() {
    let repo = EditRepo::new(RepositoryKind::Git);
    repo.base();
    repo.write("selected", "first\n");
    repo.command(&["add", "selected"]);
    repo.write("selected", "second\n");

    let snapshot = repo
        .backend
        .snapshot()
        .expect("test operation should succeed");
    assert_eq!(
        snapshot.changes.len(),
        2,
        "one staged row and one working row"
    );

    repo.backend
        .apply_edit(&repo.review(&["selected"]), RepositoryEdit::Stage, "")
        .expect("test operation should succeed");

    let snapshot = repo
        .backend
        .snapshot()
        .expect("test operation should succeed");
    assert_eq!(snapshot.changes.len(), 1);
    assert_eq!(snapshot.changes[0].layer, ChangeLayer::Index);
    assert_eq!(repo.command(&["show", ":selected"]), "second\n");
}

#[test]
fn git_unstage_drops_index_entries_before_the_first_commit() {
    let repo = EditRepo::new(RepositoryKind::Git);
    repo.write("selected", "new\n");
    repo.command(&["add", "selected"]);

    let snapshot = repo
        .backend
        .snapshot()
        .expect("test operation should succeed");
    assert_eq!(snapshot.changes.len(), 1);
    assert_eq!(snapshot.changes[0].layer, ChangeLayer::Index);

    repo.backend
        .apply_edit(&repo.review(&["selected"]), RepositoryEdit::Unstage, "")
        .expect("test operation should succeed");

    let snapshot = repo
        .backend
        .snapshot()
        .expect("test operation should succeed");
    assert_eq!(snapshot.changes.len(), 1);
    assert_eq!(snapshot.changes[0].layer, ChangeLayer::Untracked);
    assert_eq!(repo.read("selected"), "new\n");
}

#[test]
fn git_initial_commit_selects_only_chosen_new_file() {
    let repo = EditRepo::new(RepositoryKind::Git);
    repo.write("selected", "new\n");
    repo.write("other", "keep\n");
    repo.command(&["add", "other"]);
    repo.backend
        .apply_edit(
            &repo.review(&["selected"]),
            RepositoryEdit::Commit,
            "initial",
        )
        .expect("test operation should succeed");
    assert_eq!(
        repo.command(&["ls-tree", "--name-only", "HEAD"]),
        "selected\n"
    );
    assert_eq!(
        repo.command(&["diff", "--cached", "--name-only"]),
        "other\n"
    );
}

#[test]
fn git_index_commit_leaves_unstaged_edits_and_unrelated_files_alone() {
    let repo = EditRepo::new(RepositoryKind::Git);
    repo.base();
    repo.write("selected", "staged\n");
    repo.write("fresh", "new\n");
    repo.command(&["add", "selected", "fresh"]);
    repo.write("selected", "working\n");
    repo.write("other", "unstaged\n");

    repo.backend
        .apply_edit(
            &repo.review(&["selected", "fresh"]),
            RepositoryEdit::CommitIndex,
            "staged contents",
        )
        .expect("test operation should succeed");

    assert_eq!(repo.command(&["show", "HEAD:selected"]), "staged\n");
    assert_eq!(repo.command(&["show", "HEAD:fresh"]), "new\n");
    assert_eq!(repo.command(&["show", "HEAD:other"]), "base\n");
    assert_eq!(repo.read("selected"), "working\n");
    assert_eq!(repo.read("other"), "unstaged\n");
    assert_eq!(repo.command(&["diff", "--name-only"]), "other\nselected\n");
    assert_eq!(repo.command(&["diff", "--cached", "--name-only"]), "");
}

#[test]
fn git_index_commit_fails_when_nothing_is_staged() {
    let repo = EditRepo::new(RepositoryKind::Git);
    repo.base();
    repo.write("selected", "working\n");
    let head = repo.command(&["rev-parse", "HEAD"]);

    assert!(
        repo.backend
            .apply_edit(
                &repo.review(&["selected"]),
                RepositoryEdit::CommitIndex,
                "nothing staged"
            )
            .is_err()
    );

    assert_eq!(repo.command(&["rev-parse", "HEAD"]), head);
    assert_eq!(repo.read("selected"), "working\n");
}

#[test]
fn git_hunk_staging_moves_one_hunk_into_the_index() {
    let repo = EditRepo::new(RepositoryKind::Git);
    repo.base();
    repo.two_hunk_file("wide");

    let diff = repo
        .backend
        .file_diff(&repo.target("wide", ChangeLayer::WorkingTree), false)
        .expect("test operation should succeed");
    assert_eq!(diff.hunks.len(), 2);

    let patch = diff.patch_for(0).expect("first hunk");
    repo.backend
        .apply_hunk_patch(&patch, HunkApply::Stage)
        .expect("test operation should succeed");

    let staged = repo.command(&["show", ":wide"]);
    assert!(staged.contains("line two\n"));
    assert!(staged.contains("line 19\n"));
    assert!(!staged.contains("line nineteen\n"));
    assert!(repo.read("wide").contains("line nineteen\n"));

    // The staged hunk can be taken back out of the index on its own.
    let diff = repo
        .backend
        .file_diff(&repo.target("wide", ChangeLayer::Index), false)
        .expect("test operation should succeed");
    assert_eq!(diff.hunks.len(), 1);
    let patch = diff.patch_for(0).expect("only hunk");
    repo.backend
        .apply_hunk_patch(&patch, HunkApply::Unstage)
        .expect("test operation should succeed");

    // Unstaging leaves the index on the commit again, working tree untouched.
    let index = repo.command(&["show", ":wide"]);
    assert!(index.contains("line 2\n"));
    assert!(!index.contains("line two\n"));
    assert!(repo.read("wide").contains("line two\n"));
    assert!(repo.read("wide").contains("line nineteen\n"));
}

#[test]
fn git_file_diff_carries_the_unchanged_lines_between_its_hunks() {
    let repo = EditRepo::new(RepositoryKind::Git);
    repo.base();
    repo.two_hunk_file("wide");

    let hunks_only = repo
        .backend
        .file_diff(&repo.target("wide", ChangeLayer::WorkingTree), false)
        .expect("test operation should succeed");
    assert_eq!(hunks_only.hunks.len(), 2);
    assert_eq!(
        hunks_only.spans().iter().map(Vec::len).collect::<Vec<_>>(),
        vec![1, 6, 3]
    );

    let whole = repo
        .backend
        .file_diff(&repo.target("wide", ChangeLayer::WorkingTree), true)
        .expect("test operation should succeed");
    assert_eq!(whole.hunks.len(), 2);
    assert_eq!(
        whole.spans().iter().map(Vec::len).collect::<Vec<_>>(),
        vec![1, 16, 5]
    );
    assert_eq!(
        whole.spans()[2]
            .iter()
            .map(|line| line.text.clone())
            .collect::<Vec<_>>(),
        ["line 20", "line 21", "line 22", "line 23", "line 24"]
            .map(str::to_owned)
            .to_vec()
    );
    assert_eq!(
        whole.spans()[1]
            .iter()
            .map(|line| line.text.clone())
            .collect::<Vec<_>>(),
        (3..=18)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
    );
    assert_eq!(whole.spans()[1][0].old_line, Some(3));
    assert_eq!(whole.spans()[1][0].new_line, Some(3));

    let patch = whole.patch_for(0).expect("first hunk");
    repo.backend
        .apply_hunk_patch(&patch, HunkApply::Stage)
        .expect("test operation should succeed");
    assert!(repo.command(&["show", ":wide"]).contains("line two\n"));
    assert!(repo.read("wide").contains("line nineteen\n"));
}

#[test]
fn git_hunk_revert_restores_only_that_hunk_in_the_working_tree() {
    let repo = EditRepo::new(RepositoryKind::Git);
    repo.base();
    repo.two_hunk_file("wide");

    let diff = repo
        .backend
        .file_diff(&repo.target("wide", ChangeLayer::WorkingTree), false)
        .expect("test operation should succeed");
    let patch = diff.patch_for(1).expect("second hunk");
    repo.backend
        .apply_hunk_patch(&patch, HunkApply::Revert)
        .expect("test operation should succeed");

    let contents = repo.read("wide");
    assert!(contents.contains("line two\n"));
    assert!(contents.contains("line 19\n"));
    assert!(!contents.contains("line nineteen\n"));
    assert!(
        repo.command(&["diff", "--cached", "--name-only"])
            .is_empty()
    );
}

#[test]
fn git_hunk_patch_reports_an_index_that_moved_on() {
    let repo = EditRepo::new(RepositoryKind::Git);
    repo.base();
    repo.two_hunk_file("wide");

    let diff = repo
        .backend
        .file_diff(&repo.target("wide", ChangeLayer::WorkingTree), false)
        .expect("test operation should succeed");
    let patch = diff.patch_for(1).expect("second hunk");
    // Stage the whole file, so the hunk's context is no longer in the index.
    repo.command(&["add", "wide"]);

    assert!(
        repo.backend
            .apply_hunk_patch(&patch, HunkApply::Stage)
            .is_err()
    );
    // A failed hunk leaves the staged file exactly as it was.
    assert_eq!(repo.command(&["diff", "--cached", "--name-only"]), "wide\n");
    assert_eq!(repo.command(&["show", ":wide"]), repo.read("wide"));
    assert!(repo.backend.apply_hunk_patch("", HunkApply::Stage).is_err());
}

#[test]
fn git_hunk_staging_covers_a_new_file() {
    let repo = EditRepo::new(RepositoryKind::Git);
    repo.base();
    repo.write("fresh", "one\ntwo\nthree\n");

    let diff = repo
        .backend
        .file_diff(&repo.target("fresh", ChangeLayer::Untracked), false)
        .expect("test operation should succeed");
    assert_eq!(diff.hunks.len(), 1);
    let patch = diff.patch_for(0).expect("only hunk");
    repo.backend
        .apply_hunk_patch(&patch, HunkApply::Stage)
        .expect("test operation should succeed");

    assert_eq!(repo.command(&["show", ":fresh"]), "one\ntwo\nthree\n");
}

#[test]
fn review_rejects_empty_selection_and_paths_outside_project() {
    let repo = EditRepo::new(RepositoryKind::Git);
    repo.base();
    let snapshot = repo
        .backend
        .snapshot()
        .expect("test operation should succeed");
    for selected in [BTreeSet::new(), BTreeSet::from([PathBuf::from("../other")])] {
        assert!(repo.backend.prepare_edit(&snapshot, &selected).is_err());
    }
}

#[test]
fn jj_commit_selected_and_discard_keep_other_changes() {
    if !jj_installed() {
        return;
    }
    let repo = EditRepo::new(RepositoryKind::Jujutsu);
    repo.base();
    repo.write("selected", "chosen\n");
    repo.write("other", "keep\n");
    repo.write("a|b.txt", "literal\n");
    let review = repo.review(&["selected", "a|b.txt"]);
    repo.backend
        .apply_edit(&review, RepositoryEdit::Commit, "chosen files")
        .expect("test operation should succeed");
    assert_eq!(
        repo.command(&["file", "show", "-r", "@-", "selected"]),
        "chosen\n"
    );
    assert_eq!(
        repo.command(&["file", "show", "-r", "@-", "other"]),
        "base\n"
    );
    assert_eq!(repo.read("other"), "keep\n");
    repo.backend
        .apply_edit(&repo.review(&["other"]), RepositoryEdit::Discard, "")
        .expect("test operation should succeed");
    assert_eq!(repo.read("other"), "base\n");
    assert_eq!(repo.read("selected"), "chosen\n");
}

#[test]
fn jj_review_rejects_changes_after_review() {
    if !jj_installed() {
        return;
    }
    let repo = EditRepo::new(RepositoryKind::Jujutsu);
    repo.base();
    repo.write("selected", "reviewed\n");
    let review = repo.review(&["selected"]);
    repo.write("selected", "later\n");
    assert!(matches!(
        repo.backend
            .apply_edit(&review, RepositoryEdit::Discard, ""),
        Err(RepositoryError::StaleSnapshot)
    ));
    assert_eq!(repo.read("selected"), "later\n");
}

#[cfg(unix)]
#[test]
fn git_failed_commit_preserves_contents_and_unrelated_index_entries() {
    use std::os::unix::fs::PermissionsExt as _;
    let repo = EditRepo::new(RepositoryKind::Git);
    repo.base();
    repo.write("selected", "keep chosen\n");
    repo.write("new", "keep new\n");
    repo.write("other", "keep staged\n");
    repo.command(&["add", "other"]);
    let head = repo.command(&["rev-parse", "HEAD"]);
    let hook = repo.root().join(".git/hooks/pre-commit");
    fs::write(&hook, "#!/bin/sh\nexit 1\n").expect("test operation should succeed");
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))
        .expect("test operation should succeed");
    assert!(
        repo.backend
            .apply_edit(
                &repo.review(&["selected", "new"]),
                RepositoryEdit::Commit,
                "must fail"
            )
            .is_err()
    );
    assert_eq!(repo.command(&["rev-parse", "HEAD"]), head);
    assert_eq!(repo.read("selected"), "keep chosen\n");
    assert_eq!(repo.read("new"), "keep new\n");
    assert_eq!(repo.command(&["show", ":other"]), "keep staged\n");
}

#[test]
fn scoped_commit_leaves_the_outside_end_of_a_rename_staged() {
    let mut repo = EditRepo::new(RepositoryKind::Git);
    repo.base();
    fs::create_dir(repo.root().join("nested")).expect("test operation should succeed");
    repo.command(&["mv", "selected", "nested/selected"]);
    repo.backend.location.project_root = repo.root().join("nested");
    let snapshot = repo
        .backend
        .snapshot()
        .expect("test operation should succeed");
    let review = repo
        .backend
        .prepare_edit(
            &snapshot,
            &BTreeSet::from([PathBuf::from("nested/selected")]),
        )
        .expect("test operation should succeed");
    assert_eq!(review.paths(), &[PathBuf::from("nested/selected")]);
    repo.backend
        .apply_edit(&review, RepositoryEdit::Commit, "add nested file")
        .expect("test operation should succeed");
    assert_eq!(repo.command(&["show", "HEAD:selected"]), "base\n");
    assert_eq!(
        repo.command(&["diff", "--cached", "--name-only"]),
        "selected\n"
    );
    assert_eq!(repo.read("nested/selected"), "base\n");
}

#[cfg(unix)]
#[test]
fn symlink_discard_does_not_follow_the_target() {
    let repo = EditRepo::new(RepositoryKind::Git);
    repo.base();
    let outside = repo.temp.path().join("outside");
    fs::write(&outside, "keep outside\n").expect("test operation should succeed");
    std::os::unix::fs::symlink(&outside, repo.root().join("link"))
        .expect("test operation should succeed");
    repo.backend
        .apply_edit(&repo.review(&["link"]), RepositoryEdit::Discard, "")
        .expect("test operation should succeed");
    assert_eq!(
        fs::read_to_string(&outside).expect("test operation should succeed"),
        "keep outside\n"
    );
    assert!(fs::symlink_metadata(repo.root().join("link")).is_err());
}
