use super::*;

#[test]
fn change_rows_keep_staged_and_working_tree_layers_apart() {
    assert_ne!(
        crate::repository::ChangeLayer::Index,
        crate::repository::ChangeLayer::WorkingTree
    );
    assert_eq!(group_title(crate::repository::ChangeLayer::Index), "Staged");
    assert_eq!(
        group_title(crate::repository::ChangeLayer::WorkingTree),
        "Working tree"
    );
}

/// The source control section only renders in a wide window with the run panel
/// showing, so this drives the real app over a dirty repository and draws the
/// frame that contains it.
#[gpui::test]
fn the_source_control_panel_renders_a_dirty_repository(cx: &mut gpui::TestAppContext) {
    let test_name = concat!(
        module_path!(),
        "::the_source_control_panel_renders_a_dirty_repository"
    );
    crate::app::test_support::with_prepared_offline_app(
        test_name,
        cx,
        dirty_repository,
        |cx, app, _, _| {
            cx.simulate_resize(gpui::size(gpui::px(1400.0), gpui::px(900.0)));
            cx.update(|_, cx| {
                app.update(cx, |app, cx| {
                    app.project.repository.execution_allowed = true;
                    app.workspace.run_panel_hidden = false;
                    app.request_repository_refresh(cx);
                });
            });
            wait_for_repository(cx, app);
            let changes = cx.update(|_, cx| {
                app.read(cx)
                    .project
                    .repository
                    .snapshot
                    .as_ref()
                    .expect("snapshot")
                    .changes
                    .len()
            });
            assert!(
                changes >= 2,
                "the panel should have staged and working changes to render, found {changes}"
            );
            // Two frames: the section reflects state the first draw installs.
            pump(cx, app);
            pump(cx, app);

            // The VS Code sections and the flat list are separate render paths.
            cx.update(|_, cx| {
                app.update(cx, |app, _| {
                    app.settings.stage_changes_like_vscode = true;
                });
            });
            pump(cx, app);
            pump(cx, app);
            cx.update(|_, cx| {
                app.update(cx, |app, _| {
                    app.settings.source_control_view =
                        crate::app::ui::change_tree::ChangeView::List;
                    app.settings.source_control_sort =
                        crate::app::ui::change_tree::ChangeSort::Status;
                });
            });
            pump(cx, app);
            pump(cx, app);
        },
    );
}

/// A repository with one modified tracked file and one untracked file, ready
/// before the app boots so the first refresh already sees both.
fn dirty_repository(project: &std::path::Path) {
    let git = |arguments: &[&str]| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(project)
            .args(arguments)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "-q"]);
    git(&["config", "user.name", "Panel Test"]);
    git(&["config", "user.email", "panel@example.invalid"]);
    std::fs::write(project.join("tracked.txt"), "one\n").expect("write tracked file");
    git(&["add", "."]);
    git(&["-c", "commit.gpgsign=false", "commit", "-qm", "base"]);
    std::fs::write(project.join("tracked.txt"), "two\n").expect("change tracked file");
    std::fs::write(project.join("fresh.txt"), "new\n").expect("write untracked file");
}

/// Opens the in-app diff for the modified file, draws it, and stages its hunk,
/// which is the whole hunk path the gutter actions take.
#[gpui::test]
fn the_app_diff_overlay_stages_a_hunk(cx: &mut gpui::TestAppContext) {
    let test_name = concat!(module_path!(), "::the_app_diff_overlay_stages_a_hunk");
    crate::app::test_support::with_prepared_offline_app(
        test_name,
        cx,
        dirty_repository,
        |cx, app, _, _| {
            cx.simulate_resize(gpui::size(gpui::px(1400.0), gpui::px(900.0)));
            cx.update(|_, cx| {
                app.update(cx, |app, cx| {
                    app.project.repository.execution_allowed = true;
                    app.workspace.run_panel_hidden = false;
                    app.request_repository_refresh(cx);
                });
            });
            wait_for_repository(cx, app);

            let target = cx.update(|_, cx| {
                let app = app.read(cx);
                let change = app
                    .project
                    .repository
                    .snapshot
                    .as_ref()
                    .expect("snapshot")
                    .changes
                    .iter()
                    .find(|change| change.layer == crate::repository::ChangeLayer::WorkingTree)
                    .expect("a working tree change");
                (
                    change.target.key.clone(),
                    change.relative_path.clone(),
                    change.layer,
                )
            });
            cx.update(|window, cx| {
                app.update(cx, |app, cx| {
                    app.open_repository_diff(
                        target.0.clone(),
                        target.1.clone(),
                        target.2,
                        window,
                        cx,
                    );
                });
            });
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            while cx.update(|_, cx| {
                app.read(cx)
                    .overlays
                    .repository_diff
                    .as_ref()
                    .is_none_or(|diff| diff.preparing())
            }) {
                assert!(std::time::Instant::now() < deadline, "the diff should load");
                pump(cx, app);
            }
            let hunks = cx.update(|_, cx| {
                app.read(cx)
                    .overlays
                    .repository_diff
                    .as_ref()
                    .and_then(|diff| diff.diff.as_ref())
                    .expect("a loaded diff")
                    .hunks
                    .len()
            });
            assert_eq!(hunks, 1, "one hunk replaces the single line");
            pump(cx, app);

            cx.update(|_, cx| {
                app.update(cx, |app, cx| {
                    app.apply_repository_hunk(0, crate::repository::HunkApply::Stage, cx)
                });
            });
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            let staged = loop {
                pump(cx, app);
                let outcome = cx.update(|_, cx| {
                    let app = app.read(cx);
                    let diff = app.overlays.repository_diff.as_ref().expect("diff");
                    (diff.applying.is_none(), diff.error.clone())
                });
                assert_eq!(outcome.1, None, "staging the hunk should not fail");
                if !outcome.0 {
                    continue;
                }
                let staged = cx.update(|_, cx| {
                    app.read(cx)
                        .project
                        .repository
                        .snapshot
                        .as_ref()
                        .is_some_and(|snapshot| {
                            // The staged row is a different key: same path, new layer.
                            snapshot.changes.iter().any(|change| {
                                change.layer == crate::repository::ChangeLayer::Index
                                    && change.relative_path == target.1
                            })
                        })
                });
                if staged {
                    break staged;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "staging the hunk should land in the index"
                );
            };
            assert!(staged);
            // The overlay stays open on the refreshed diff.
            let error = cx.update(|_, cx| {
                app.read(cx)
                    .overlays
                    .repository_diff
                    .as_ref()
                    .expect("the diff stays open")
                    .error
                    .clone()
            });
            assert_eq!(error, None);
            pump(cx, app);
            cx.update(|window, cx| {
                app.update(cx, |app, cx| app.close_repository_diff(window, cx));
            });
            cx.update(|_, cx| assert!(app.read(cx).overlays.repository_diff.is_none()));
        },
    );
}

fn wait_for_repository(cx: &mut gpui::VisualTestContext, app: &gpui::Entity<FarcasterApp>) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while cx.update(|_, cx| {
        let app = app.read(cx);
        app.project.repository.backend.is_none() || app.project.repository.snapshot.is_none()
    }) {
        assert!(
            std::time::Instant::now() < deadline,
            "the repository refresh should land"
        );
        pump(cx, app);
    }
}

fn pump(cx: &mut gpui::VisualTestContext, app: &gpui::Entity<FarcasterApp>) {
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(600));
    cx.run_until_parked();
    cx.update(|window, cx| {
        app.update(cx, |app, cx| app.drain_runtime(cx));
        window.draw(cx).clear(cx);
    });
}
