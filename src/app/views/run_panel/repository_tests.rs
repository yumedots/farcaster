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
        |project| {
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
        },
        |cx, app, _, _| {
            cx.simulate_resize(gpui::size(gpui::px(1400.0), gpui::px(900.0)));
            cx.update(|_, cx| {
                app.update(cx, |app, cx| {
                    app.project.repository.execution_allowed = true;
                    app.workspace.run_panel_hidden = false;
                    app.request_repository_refresh(cx);
                });
            });
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            while cx.update(|_, cx| {
                let app = app.read(cx);
                app.project.repository.backend.is_none()
                    || app.project.repository.snapshot.is_none()
            }) {
                assert!(
                    std::time::Instant::now() < deadline,
                    "the repository refresh should land"
                );
                pump(cx, app);
            }
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
        },
    );
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
