use notify::{
    Event,
    event::{AccessKind, CreateKind},
};

use super::*;

#[test]
fn ignores_access_and_reports_changes() {
    assert_eq!(
        repository_event(Ok(Event::new(EventKind::Access(AccessKind::Any)))),
        None
    );
    assert_eq!(
        repository_event(Ok(Event::new(EventKind::Create(CreateKind::File)))),
        Some(RepositoryWatchEvent::Changed)
    );
}

#[test]
fn discovery_watches_project_and_ancestors_but_only_accepts_repository_markers() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project = temp.path().join("parent/project");
    fs::create_dir_all(&project).expect("project");
    let project = project.canonicalize().expect("project");
    let parent = project.parent().expect("parent").to_path_buf();

    let targets = discovery_targets(&project).expect("targets");
    assert!(
        targets
            .iter()
            .all(|target| target.mode == RecursiveMode::NonRecursive)
    );
    assert!(targets.contains(&WatchTarget {
        path: project.clone(),
        mode: RecursiveMode::NonRecursive,
    }));
    assert!(targets.contains(&WatchTarget {
        path: parent,
        mode: RecursiveMode::NonRecursive,
    }));
    assert_eq!(
        discovery_event(Ok(
            Event::new(EventKind::Create(CreateKind::Folder)).add_path(project.join(".git"))
        )),
        Some(RepositoryWatchEvent::Changed)
    );
    assert_eq!(
        discovery_event(Ok(
            Event::new(EventKind::Create(CreateKind::File)).add_path(project.join("file.rs"))
        )),
        None
    );
}

#[test]
fn nested_git_project_watches_project_and_repository_metadata() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().join("workspace");
    let project = workspace.join("app");
    fs::create_dir_all(workspace.join(".git")).expect("git metadata");
    fs::create_dir_all(&project).expect("project");
    let workspace = workspace.canonicalize().expect("workspace");
    let project = project.canonicalize().expect("project");

    let targets = watch_targets(&RepositoryLocation {
        workspace_root: workspace.clone(),
        project_root: project.clone(),
    })
    .expect("targets");

    assert!(targets.contains(&WatchTarget {
        path: project,
        mode: RecursiveMode::Recursive,
    }));
    assert!(targets.contains(&WatchTarget {
        path: workspace.join(".git"),
        mode: RecursiveMode::Recursive,
    }));
}

#[test]
fn linked_worktree_watches_common_metadata_that_contains_private_state() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().join("worktree");
    let git_dir = temp.path().join("main/.git/worktrees/worktree");
    let common_dir = temp.path().join("main/.git");
    fs::create_dir_all(&workspace).expect("workspace");
    fs::create_dir_all(&git_dir).expect("worktree metadata");
    fs::write(
        workspace.join(".git"),
        format!("gitdir: {}\n", git_dir.display()),
    )
    .expect("git pointer");
    fs::write(git_dir.join("commondir"), "../..\n").expect("common pointer");
    let workspace = workspace.canonicalize().expect("workspace");
    let git_dir = git_dir.canonicalize().expect("worktree metadata");
    let common_dir = common_dir.canonicalize().expect("common metadata");

    let targets = watch_targets(&RepositoryLocation {
        workspace_root: workspace.clone(),
        project_root: workspace,
    })
    .expect("targets");

    assert!(
        targets
            .iter()
            .any(|target| { target.path == common_dir && target.mode == RecursiveMode::Recursive })
    );
    assert!(git_dir.starts_with(&common_dir));
}
