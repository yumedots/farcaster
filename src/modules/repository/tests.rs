use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use super::*;

#[path = "edit_tests.rs"]
mod edit_tests;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("pi-repository-{label}-{}-{id}", std::process::id()));
        fs::create_dir_all(&path).expect("create test directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _remove_result = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn discovery_uses_the_deepest_marker() {
    let temp = TestDirectory::new("discovery");
    fs::create_dir(temp.path().join(".git")).expect("create Git marker");
    let nested = temp.path().join("nested/project");
    fs::create_dir_all(&nested).expect("create nested project");
    fs::create_dir(nested.parent().expect("nested parent").join(".git"))
        .expect("create nested Git marker");

    let root = discover_available(temp.path())
        .expect("discover root")
        .expect("root repository");
    assert_eq!(
        root.location.workspace_root,
        temp.path().canonicalize().expect("canonical root")
    );

    let nested_backend = discover_available(&nested)
        .expect("discover nested")
        .expect("nested repository");
    assert_eq!(
        nested_backend.location.workspace_root,
        nested
            .parent()
            .expect("nested parent")
            .canonicalize()
            .expect("canonical nested root")
    );
}

#[test]
fn no_repository_is_distinct_from_failure() {
    let temp = TestDirectory::new("none");
    let parent =
        RepositoryBackend::discover(temp.path().parent().expect("test operation should succeed"))
            .expect("discover enclosing repository");
    let result = RepositoryBackend::discover(temp.path()).expect("marker scan should succeed");
    assert_eq!(
        result.map(|backend| backend.location.workspace_root),
        parent.map(|backend| backend.location.workspace_root)
    );
    assert!(RepositoryBackend::discover(&temp.path().join("missing")).is_err());
}

#[cfg(unix)]
#[test]
fn stable_keys_do_not_use_lossy_path_display() {
    use std::os::unix::ffi::OsStringExt as _;

    let temp = TestDirectory::new("keys");
    let location = RepositoryLocation {
        workspace_root: temp.path().to_path_buf(),
        project_root: temp.path().to_path_buf(),
    };
    let first = change(
        &location,
        Arc::from([]),
        PathBuf::from(OsString::from_vec(vec![0xff])),
        None,
        ChangeLayer::Untracked,
        ChangeKind::Untracked,
    )
    .expect("first target");
    let second = change(
        &location,
        Arc::from([]),
        PathBuf::from(OsString::from_vec(vec![0xfe])),
        None,
        ChangeLayer::Untracked,
        ChangeKind::Untracked,
    )
    .expect("second target");
    assert_eq!(
        first.target.relative_path.to_string_lossy(),
        second.target.relative_path.to_string_lossy()
    );
    assert_eq!(first.target.layer, ChangeLayer::Untracked);
    assert_eq!(first.target.kind.status_label(), "U");
    assert_ne!(first.target.key, second.target.key);
}

#[test]
fn diff_results_count_text_and_mark_binary_counts_unknown() {
    assert_eq!(
        patch_counts("--- a/x\n+++ b/x\n-old\n+new\n+more\n"),
        (Some(2), Some(1))
    );
    assert_eq!(patch_counts("GIT binary patch\nliteral 1\n"), (None, None));
}

#[test]
fn git_snapshot_and_lazy_diff_use_separate_layers() {
    if Command::new("git").arg("--version").output().is_err() {
        return;
    }
    let temp = TestDirectory::new("git-command");
    let repository = temp.path().join("repo");
    let home = temp.path().join("home");
    let config = temp.path().join("config");
    fs::create_dir_all(&repository).expect("create repository directory");
    fs::create_dir_all(&home).expect("create home directory");
    fs::create_dir_all(&config).expect("create config directory");
    run_git(&repository, &home, &config, &["init"]);
    run_git(
        &repository,
        &home,
        &config,
        &["config", "user.name", "Pi Test"],
    );
    run_git(
        &repository,
        &home,
        &config,
        &["config", "user.email", "pi@example.invalid"],
    );
    fs::write(repository.join("file.txt"), "base\n").expect("write base");
    run_git(&repository, &home, &config, &["add", "file.txt"]);
    run_git(&repository, &home, &config, &["commit", "-m", "base"]);
    fs::write(repository.join("file.txt"), "staged\n").expect("write staged");
    run_git(&repository, &home, &config, &["add", "file.txt"]);
    fs::write(repository.join("file.txt"), "staged\nworking\n").expect("write working");

    let options = RepositoryOptions {
        environment: isolated_environment(&home, &config),
        ..RepositoryOptions::default()
    };
    let backend = RepositoryBackend::discover_with_options(&repository, options)
        .expect("discover Git")
        .expect("Git repository");
    let mut snapshot = backend.snapshot().expect("capture Git snapshot");
    assert_eq!(
        backend
            .working_copy_totals(&mut snapshot)
            .expect("count Git working copy diff"),
        (Some(2), Some(1))
    );
    for (layer, counts) in [
        (ChangeLayer::Index, (1, 1)),
        (ChangeLayer::WorkingTree, (1, 0)),
    ] {
        let change = snapshot
            .changes
            .iter()
            .find(|change| change.layer == layer)
            .expect("test operation should succeed");
        assert_eq!(change.counts, Some(counts));
    }
    assert_eq!(
        backend.list_project_files().expect("list Git files"),
        ["file.txt"]
    );
    assert!(
        snapshot
            .changes
            .iter()
            .any(|row| row.layer == ChangeLayer::Index)
    );
    assert!(
        snapshot
            .changes
            .iter()
            .any(|row| row.layer == ChangeLayer::WorkingTree)
    );
    let target = snapshot
        .changes
        .iter()
        .find(|row| row.layer == ChangeLayer::Index)
        .expect("staged row")
        .target
        .clone();
    let diff = backend.load_diff(target).expect("load staged diff");
    assert!(diff.patch.contains("+staged"));
    assert_eq!(diff.additions, Some(1));
    assert_eq!(diff.deletions, Some(1));
    assert!(diff.exists);

    run_git(&repository, &home, &config, &["reset", "--hard", "HEAD"]);
    fs::remove_file(repository.join("file.txt")).expect("delete tracked file");
    let deleted = backend.snapshot().expect("capture deleted file");
    assert_eq!(deleted.changes.len(), 1);
    assert_eq!(deleted.changes[0].kind, ChangeKind::Deleted);
    assert!(!deleted.changes[0].target.exists);

    fs::write(repository.join("file.txt"), "base\n").expect("restore tracked file");
    assert!(
        backend
            .snapshot()
            .expect("capture restored working copy")
            .changes
            .is_empty()
    );

    fs::write(repository.join("file.txt"), "committed\n").expect("modify tracked file");
    run_git(&repository, &home, &config, &["add", "file.txt"]);
    run_git(&repository, &home, &config, &["commit", "-m", "change"]);
    assert!(
        backend
            .snapshot()
            .expect("capture committed working copy")
            .changes
            .is_empty()
    );
}

#[test]
fn git_untracked_files_are_counted_from_their_contents() {
    if Command::new("git").arg("--version").output().is_err() {
        return;
    }
    let temp = TestDirectory::new("git-untracked-totals");
    let repository = temp.path().join("repo");
    let home = temp.path().join("home");
    let config = temp.path().join("config");
    fs::create_dir_all(&repository).expect("create repository directory");
    fs::create_dir_all(&home).expect("create home directory");
    fs::create_dir_all(&config).expect("create config directory");
    run_git(&repository, &home, &config, &["init"]);
    run_git(
        &repository,
        &home,
        &config,
        &["config", "user.name", "Pi Test"],
    );
    run_git(
        &repository,
        &home,
        &config,
        &["config", "user.email", "pi@example.invalid"],
    );
    fs::write(repository.join("file.txt"), "base\n").expect("write base");
    run_git(&repository, &home, &config, &["add", "file.txt"]);
    run_git(&repository, &home, &config, &["commit", "-m", "base"]);
    fs::write(repository.join("new.txt"), "one\ntwo\n").expect("write untracked text");

    let options = RepositoryOptions {
        environment: isolated_environment(&home, &config),
        ..RepositoryOptions::default()
    };
    let backend = RepositoryBackend::discover_with_options(&repository, options)
        .expect("discover Git")
        .expect("Git repository");
    let mut snapshot = backend.snapshot().expect("capture Git snapshot");
    assert_eq!(
        backend
            .working_copy_totals(&mut snapshot)
            .expect("count untracked text"),
        (Some(2), Some(0))
    );
    let change = snapshot
        .changes
        .iter()
        .find(|change| change.layer == ChangeLayer::Untracked)
        .expect("untracked row");
    assert_eq!(change.counts, Some((2, 0)));

    fs::write(repository.join("blob.bin"), [0_u8, 1]).expect("write untracked binary");
    let mut snapshot = backend.snapshot().expect("capture binary snapshot");
    assert_eq!(
        backend
            .working_copy_totals(&mut snapshot)
            .expect("count untracked binary"),
        (None, None)
    );
    let change = snapshot
        .changes
        .iter()
        .find(|change| change.relative_path.as_path() == Path::new("blob.bin"))
        .expect("binary row");
    assert_eq!(change.counts, None);
}

#[test]
fn linked_git_worktree_is_an_independent_working_copy() {
    if Command::new("git").arg("--version").output().is_err() {
        return;
    }
    let temp = TestDirectory::new("git-worktree");
    let repository = temp.path().join("repo");
    let worktree = temp.path().join("worktree");
    let home = temp.path().join("home");
    let config = temp.path().join("config");
    fs::create_dir_all(&repository).expect("create repository directory");
    fs::create_dir_all(&home).expect("create home directory");
    fs::create_dir_all(&config).expect("create config directory");
    run_git(&repository, &home, &config, &["init"]);
    run_git(
        &repository,
        &home,
        &config,
        &["config", "user.name", "Pi Test"],
    );
    run_git(
        &repository,
        &home,
        &config,
        &["config", "user.email", "pi@example.invalid"],
    );
    fs::write(repository.join("file.txt"), "base\n").expect("write base");
    run_git(&repository, &home, &config, &["add", "file.txt"]);
    run_git(&repository, &home, &config, &["commit", "-m", "base"]);
    run_git(
        &repository,
        &home,
        &config,
        &[
            "worktree",
            "add",
            "-b",
            "linked",
            worktree.to_str().expect("UTF-8 worktree path"),
        ],
    );
    fs::write(worktree.join("file.txt"), "linked\n").expect("modify worktree");

    let options = RepositoryOptions {
        environment: isolated_environment(&home, &config),
        ..RepositoryOptions::default()
    };
    let backend = RepositoryBackend::discover_with_options(&worktree, options)
        .expect("discover linked worktree")
        .expect("Git worktree");
    assert_eq!(
        backend.location.workspace_root,
        worktree.canonicalize().expect("canonical worktree")
    );
    let snapshot = backend.snapshot().expect("worktree status");
    assert_eq!(snapshot.changes.len(), 1);
    assert_eq!(snapshot.changes[0].layer, ChangeLayer::WorkingTree);
}

#[test]
fn watcher_detects_metadata_only_commits_and_settles_after_refresh() {
    use std::time::{Duration, Instant};

    if Command::new("git").arg("--version").output().is_err() {
        return;
    }
    let temp = TestDirectory::new("watch-commits");
    let repository = temp.path().join("repo");
    let home = temp.path().join("home");
    let config = temp.path().join("config");
    fs::create_dir_all(&home).expect("test operation should succeed");
    fs::create_dir_all(&config).expect("test operation should succeed");
    fs::create_dir_all(&repository).expect("test operation should succeed");
    run_git(&repository, &home, &config, &["init"]);
    run_git(
        &repository,
        &home,
        &config,
        &["config", "user.name", "Test"],
    );
    run_git(
        &repository,
        &home,
        &config,
        &["config", "user.email", "test@example.com"],
    );
    fs::write(repository.join("file.txt"), "working\n").expect("test operation should succeed");
    let backend = RepositoryBackend::discover_with_options(
        &repository,
        RepositoryOptions {
            environment: isolated_environment(&home, &config),
            ..RepositoryOptions::default()
        },
    )
    .expect("test operation should succeed")
    .expect("test operation should succeed");
    let initial = backend.snapshot().expect("test operation should succeed");
    assert_eq!(initial.changes.len(), 1);
    let (_watcher, events) =
        RepositoryWatcher::start(backend.location()).expect("test operation should succeed");

    let assert_quiet = || {
        std::thread::sleep(Duration::from_millis(200));
        assert!(
            events.try_recv().is_err(),
            "snapshot caused another refresh"
        );
    };
    backend.snapshot().expect("test operation should succeed");
    assert_quiet();
    run_git(&repository, &home, &config, &["add", "file.txt"]);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Ok(event) = events.try_recv() {
            assert_eq!(event, RepositoryWatchEvent::Changed);
            break;
        }
        assert!(Instant::now() < deadline, "missed staging event");
        std::thread::sleep(Duration::from_millis(10));
    }
    let refreshed = backend.snapshot().expect("test operation should succeed");
    assert_eq!(refreshed.changes.len(), 1);

    run_git(
        &repository,
        &home,
        &config,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-m",
            "external commit",
        ],
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Ok(event) = events.try_recv() {
            assert_eq!(event, RepositoryWatchEvent::Changed);
            break;
        }
        assert!(Instant::now() < deadline, "missed Git commit");
        std::thread::sleep(Duration::from_millis(10));
    }
    let refreshed = backend.snapshot().expect("test operation should succeed");
    assert!(refreshed.changes.is_empty());
    assert_ne!(refreshed.identity, initial.identity);
    std::thread::sleep(Duration::from_millis(200));
    while events.try_recv().is_ok() {}
    backend.snapshot().expect("test operation should succeed");
    assert_quiet();
}

fn run_git(repository: &Path, home: &Path, config: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(repository)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", config)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("run Git command");
    assert!(
        output.status.success(),
        "Git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn discover_available(project: &Path) -> Result<Option<RepositoryBackend>, RepositoryError> {
    RepositoryBackend::discover_with_options(project, available_options())
}

fn available_options() -> RepositoryOptions {
    let executable = std::env::current_exe()
        .expect("current executable")
        .into_os_string();
    RepositoryOptions {
        git_executable: executable,
        ..RepositoryOptions::default()
    }
}

fn isolated_environment(home: &Path, config: &Path) -> Vec<(OsString, OsString)> {
    vec![
        (OsString::from("HOME"), home.as_os_str().to_os_string()),
        (
            OsString::from("XDG_CONFIG_HOME"),
            config.as_os_str().to_os_string(),
        ),
        (OsString::from("GIT_CONFIG_NOSYSTEM"), OsString::from("1")),
        (OsString::from("GIT_TERMINAL_PROMPT"), OsString::from("0")),
    ]
}
