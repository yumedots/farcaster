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
            pump(cx, app);
            pump(cx, app);

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

fn dirty_repository(project: &std::path::Path) {
    git(project, &["init", "-q"]);
    git(project, &["config", "user.name", "Panel Test"]);
    git(project, &["config", "user.email", "panel@example.invalid"]);
    std::fs::write(project.join("tracked.txt"), "one\n").expect("write tracked file");
    git(project, &["add", "."]);
    git(
        project,
        &["-c", "commit.gpgsign=false", "commit", "-qm", "base"],
    );
    std::fs::write(project.join("tracked.txt"), "two\n").expect("change tracked file");
    std::fs::write(project.join("fresh.txt"), "new\n").expect("write untracked file");
}

fn two_block_repository(project: &std::path::Path) {
    git(project, &["init", "-q"]);
    git(project, &["config", "user.name", "Panel Test"]);
    git(project, &["config", "user.email", "panel@example.invalid"]);
    let lines = (1..=24)
        .map(|line| format!("line {line}\n"))
        .collect::<String>();
    std::fs::write(project.join("wide.txt"), &lines).expect("write wide file");
    git(project, &["add", "."]);
    git(
        project,
        &["-c", "commit.gpgsign=false", "commit", "-qm", "base"],
    );
    let edited = lines
        .replace("line 2\n", "line two\n")
        .replace("line 19\n", "line nineteen\nline nineteen again\n");
    std::fs::write(project.join("wide.txt"), edited).expect("change wide file");
}

fn git(project: &std::path::Path, arguments: &[&str]) {
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
}

#[gpui::test]
fn the_app_diff_tab_stages_a_hunk(cx: &mut gpui::TestAppContext) {
    let test_name = concat!(module_path!(), "::the_app_diff_tab_stages_a_hunk");
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
                    .active_diff()
                    .is_none_or(|diff| diff.preparing())
            }) {
                assert!(std::time::Instant::now() < deadline, "the diff should load");
                pump(cx, app);
            }
            let hunks = cx.update(|_, cx| {
                app.read(cx)
                    .active_diff()
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
                    let diff = app.active_diff().expect("diff");
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
            let error = cx.update(|_, cx| {
                app.read(cx)
                    .active_diff()
                    .expect("the diff stays open")
                    .error
                    .clone()
            });
            assert_eq!(error, None);
            cx.update(|_, cx| {
                app.update(cx, |app, cx| {
                    let diff = app.active_diff().expect("diff");
                    assert!(diff.split, "a diff opens side by side");
                    app.toggle_repository_diff_split(cx);
                    assert!(!app.active_diff().expect("diff").split);
                });
            });
            pump(cx, app);
            cx.update(|window, cx| {
                app.update(cx, |app, cx| app.close_active_diff(window, cx));
            });
            cx.update(|_, cx| {
                let app = app.read(cx);
                assert!(app.active_diff().is_none());
                assert!(app.open_diffs().is_empty());
                assert_eq!(app.workspace.surface, crate::app::AppSurface::Chat);
            });
        },
    );
}

#[gpui::test]
fn the_app_diff_shows_the_lines_between_its_blocks(cx: &mut gpui::TestAppContext) {
    let test_name = concat!(
        module_path!(),
        "::the_app_diff_shows_the_lines_between_its_blocks"
    );
    crate::app::test_support::with_prepared_offline_app(
        test_name,
        cx,
        two_block_repository,
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
                    .first()
                    .cloned()
                    .expect("the changed file");
                (change.target.key, change.relative_path, change.layer)
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
            wait_for_diffs(cx, app, 1);
            let between = cx.update(|_, cx| {
                let app = app.read(cx);
                let diff = app.active_diff().expect("the diff");
                let file = diff.diff.as_ref().expect("a diff");
                assert_eq!(file.hunks.len(), 2);
                assert!(!diff.hide_unchanged, "a diff opens on the whole file");
                diff.rows
                    .iter()
                    .filter(|row| match row {
                        crate::repository::DiffRow::Line { source, .. }
                        | crate::repository::DiffRow::Split { source, .. } => {
                            matches!(source, crate::repository::DiffSource::Unchanged(_))
                        }
                        crate::repository::DiffRow::Block { .. }
                        | crate::repository::DiffRow::Band { .. } => false,
                    })
                    .count()
            });
            assert_eq!(between, 22);
            pump(cx, app);
            pump(cx, app);

            cx.update(|_, cx| {
                app.update(cx, |app, cx| app.toggle_settings_hide_unchanged_lines(cx));
            });
            let folded = cx.update(|_, cx| {
                let app = app.read(cx);
                assert!(app.settings.hide_unchanged_lines);
                let diff = app.active_diff().expect("the diff");
                assert!(!diff.preparing(), "folding the lines reads nothing again");
                diff.rows
                    .iter()
                    .filter_map(|row| match row {
                        crate::repository::DiffRow::Band { lines, .. } => Some(*lines),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            });
            assert_eq!(folded, vec![1, 16, 5]);
            pump(cx, app);

            cx.update(|_, cx| {
                app.update(cx, |app, cx| {
                    app.toggle_settings_hide_unchanged_lines(cx);
                    app.toggle_repository_diff_split(cx);
                });
            });
            pump(cx, app);
            pump(cx, app);
        },
    );
}

#[gpui::test]
fn the_app_diff_reveals_every_line_of_an_opened_region(cx: &mut gpui::TestAppContext) {
    let test_name = concat!(
        module_path!(),
        "::the_app_diff_reveals_every_line_of_an_opened_region"
    );
    crate::app::test_support::with_prepared_offline_app(
        test_name,
        cx,
        two_block_repository,
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
            open_diff(cx, app, std::path::Path::new("wide.txt"));
            pump(cx, app);

            cx.update(|_, cx| {
                app.update(cx, |app, cx| app.toggle_settings_hide_unchanged_lines(cx));
            });
            let folded = cx.update(|_, cx| {
                let app = app.read(cx);
                app.active_diff()
                    .expect("the diff")
                    .rows
                    .iter()
                    .filter_map(|row| match row {
                        crate::repository::DiffRow::Band { span, lines, .. } => {
                            Some((*span, *lines))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            });
            assert_eq!(folded, vec![(0, 1), (1, 16), (2, 5)]);

            cx.update(|_, cx| {
                app.update(cx, |app, cx| app.toggle_repository_diff_span(1, cx));
            });
            pump(cx, app);
            pump(cx, app);
            let opened = cx.update(|_, cx| {
                let diff = app.read(cx).active_diff().expect("the diff");
                let revealed = diff
                    .rows
                    .iter()
                    .filter(|row| {
                        matches!(
                            row,
                            crate::repository::DiffRow::Line {
                                source: crate::repository::DiffSource::Unchanged(1),
                                ..
                            } | crate::repository::DiffRow::Split {
                                source: crate::repository::DiffSource::Unchanged(1),
                                ..
                            }
                        )
                    })
                    .count();
                assert!(
                    diff.rows.iter().any(|row| matches!(
                        row,
                        crate::repository::DiffRow::Band {
                            span: 1,
                            folded: false,
                            ..
                        }
                    )),
                    "an opened run keeps the band that folds it again"
                );
                let rows = diff
                    .rows
                    .iter()
                    .filter(|row| !matches!(row, crate::repository::DiffRow::Block { .. }))
                    .count();
                let height = rows as f32 * 18.0;
                let reachable = f32::from(diff.scroll.max_offset().y)
                    + f32::from(diff.scroll.bounds().size.height);
                (revealed, height, reachable)
            });
            assert_eq!(opened.0, 16, "every line of the run is drawn");
            assert!(
                opened.2 >= opened.1 - 1.0,
                "the reading should be as tall as its rows: {} of {:?}",
                opened.1,
                opened.2
            );
        },
    );
}

fn wait_for_diffs(
    cx: &mut gpui::VisualTestContext,
    app: &gpui::Entity<FarcasterApp>,
    count: usize,
) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while cx.update(|_, cx| {
        let diffs = app.read(cx).open_diffs();
        diffs.len() != count || diffs.iter().any(|diff| diff.preparing())
    }) {
        assert!(
            std::time::Instant::now() < deadline,
            "{count} diffs should load"
        );
        pump(cx, app);
    }
}

#[gpui::test]
fn the_app_diff_tabs_hold_more_than_one_file(cx: &mut gpui::TestAppContext) {
    let test_name = concat!(
        module_path!(),
        "::the_app_diff_tabs_hold_more_than_one_file"
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
                    .iter()
                    .map(|change| {
                        (
                            change.target.key.clone(),
                            change.relative_path.clone(),
                            change.layer,
                        )
                    })
                    .collect::<Vec<_>>()
            });
            assert_eq!(changes.len(), 2, "the fixture leaves two files changed");
            for (key, path, layer) in changes.clone() {
                cx.update(|window, cx| {
                    app.update(cx, |app, cx| {
                        app.open_repository_diff(key, path, layer, window, cx);
                    });
                });
            }
            wait_for_diffs(cx, app, 2);
            cx.update(|_, cx| {
                let app = app.read(cx);
                assert_eq!(app.workspace.surface, crate::app::AppSurface::Diff);
                assert_eq!(app.workspace.active_diff.as_ref(), Some(&changes[1].0));
            });
            pump(cx, app);

            cx.update(|window, cx| {
                app.update(cx, |app, cx| {
                    app.activate_repository_diff(changes[0].0.clone(), window, cx)
                });
            });
            pump(cx, app);
            cx.update(|_, cx| {
                let app = app.read(cx);
                assert_eq!(app.workspace.active_diff.as_ref(), Some(&changes[0].0));
                assert_eq!(app.open_diffs().len(), 2, "both tabs stay open");
            });

            cx.update(|window, cx| {
                app.update(cx, |app, cx| app.close_active_diff(window, cx));
            });
            pump(cx, app);
            cx.update(|_, cx| {
                let app = app.read(cx);
                assert_eq!(app.workspace.active_diff.as_ref(), Some(&changes[1].0));
                assert_eq!(app.workspace.surface, crate::app::AppSurface::Diff);
            });

            cx.update(|window, cx| {
                app.update(cx, |app, cx| app.close_active_diff(window, cx));
            });
            cx.update(|_, cx| {
                let app = app.read(cx);
                assert!(app.open_diffs().is_empty());
                assert_eq!(app.workspace.surface, crate::app::AppSurface::Chat);
            });
        },
    );
}

fn long_line_repository(project: &std::path::Path) {
    git(project, &["init", "-q"]);
    git(project, &["config", "user.name", "Panel Test"]);
    git(project, &["config", "user.email", "panel@example.invalid"]);
    let lines = (1..=120)
        .map(|line| format!("line {line}\n"))
        .collect::<String>();
    std::fs::write(project.join("long.txt"), &lines).expect("write long file");
    git(project, &["add", "."]);
    git(
        project,
        &["-c", "commit.gpgsign=false", "commit", "-qm", "base"],
    );
    let wide = format!("{}\n", "wide ".repeat(400));
    std::fs::write(project.join("long.txt"), lines.replace("line 2\n", &wide))
        .expect("write the wide line");
}

fn one_long_line_repository(project: &std::path::Path) {
    git(project, &["init", "-q"]);
    git(project, &["config", "user.name", "Panel Test"]);
    git(project, &["config", "user.email", "panel@example.invalid"]);
    let before = format!("{}\n", "old ".repeat(60));
    std::fs::write(project.join("one.txt"), &before).expect("write the file");
    git(project, &["add", "."]);
    git(
        project,
        &["-c", "commit.gpgsign=false", "commit", "-qm", "base"],
    );
    let after = format!("{}\n", "new ".repeat(60));
    std::fs::write(project.join("one.txt"), after).expect("change the file");
}

fn long_deleted_line_repository(project: &std::path::Path) {
    git(project, &["init", "-q"]);
    git(project, &["config", "user.name", "Panel Test"]);
    git(project, &["config", "user.email", "panel@example.invalid"]);
    let before = format!("{}\n", "old ".repeat(60));
    std::fs::write(project.join("one.txt"), &before).expect("write the file");
    git(project, &["add", "."]);
    git(
        project,
        &["-c", "commit.gpgsign=false", "commit", "-qm", "base"],
    );
    std::fs::write(project.join("one.txt"), "new\n").expect("change the file");
}

#[gpui::test]
fn the_app_diff_sizes_a_row_to_the_lines_it_holds(cx: &mut gpui::TestAppContext) {
    let test_name = concat!(
        module_path!(),
        "::the_app_diff_sizes_a_row_to_the_lines_it_holds"
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
            open_diff(cx, app, std::path::Path::new("tracked.txt"));
            pump(cx, app);
            pump(cx, app);
            let row = cx
                .debug_bounds("repository-diff-row")
                .expect("the row of the change");
            assert!(
                f32::from(row.size.width) < 400.0,
                "a one-line file was read {} wide in a 1400px window",
                f32::from(row.size.width),
            );
        },
    );
}

#[gpui::test]
fn the_app_diff_sizes_each_column_to_its_own_side(cx: &mut gpui::TestAppContext) {
    let test_name = concat!(
        module_path!(),
        "::the_app_diff_sizes_each_column_to_its_own_side"
    );
    crate::app::test_support::with_prepared_offline_app(
        test_name,
        cx,
        long_deleted_line_repository,
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
            open_diff(cx, app, std::path::Path::new("one.txt"));
            pump(cx, app);
            pump(cx, app);
            let deleted = cx.update(|_, cx| {
                f32::from(app.read(cx).active_diff().expect("the diff").widest_left)
            });
            let row = cx
                .debug_bounds("repository-diff-row")
                .expect("the row of the change");
            assert!(
                f32::from(row.size.width) < deleted * 1.2,
                "the row is {} wide against {deleted} for the side that holds the long line",
                f32::from(row.size.width),
            );
        },
    );
}

#[gpui::test]
fn the_app_diff_keeps_a_wide_line_inside_its_own_half(cx: &mut gpui::TestAppContext) {
    let test_name = concat!(
        module_path!(),
        "::the_app_diff_keeps_a_wide_line_inside_its_own_half"
    );
    crate::app::test_support::with_prepared_offline_app(
        test_name,
        cx,
        one_long_line_repository,
        |cx, app, _, _| {
            cx.simulate_resize(gpui::size(gpui::px(900.0), gpui::px(700.0)));
            cx.update(|_, cx| {
                app.update(cx, |app, cx| {
                    app.project.repository.execution_allowed = true;
                    app.workspace.run_panel_hidden = false;
                    app.request_repository_refresh(cx);
                });
            });
            wait_for_repository(cx, app);
            open_diff(cx, app, std::path::Path::new("one.txt"));
            pump(cx, app);
            pump(cx, app);
            let old = cx
                .debug_bounds("repository-diff-old-text")
                .expect("the text of the deleted side");
            let new = cx
                .debug_bounds("repository-diff-new-text")
                .expect("the text of the added side");
            let overlap = f32::from(old.right()) - f32::from(new.left());
            assert!(
                overlap <= 1.0,
                "the deleted side runs {overlap} px into the added side"
            );
            let border = cx.update(|_, _| f32::from(crate::app::ui::theme::theme().border));
            let room = f32::from(new.left()) - f32::from(old.left()) - border;
            assert!(
                f32::from(new.size.width) <= room + 1.0,
                "the added side is {} wide in a half of {room}",
                f32::from(new.size.width),
            );
        },
    );
}

fn open_diff(
    cx: &mut gpui::VisualTestContext,
    app: &gpui::Entity<FarcasterApp>,
    path: &std::path::Path,
) {
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
            .find(|change| change.relative_path == path)
            .expect("a change for the file");
        (
            change.target.key.clone(),
            change.relative_path.clone(),
            change.layer,
        )
    });
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.open_repository_diff(target.0, target.1, target.2, window, cx);
        });
    });
    wait_for_diffs(cx, app, 1);
}

#[gpui::test]
fn the_app_diff_scrolls_a_long_line_sideways(cx: &mut gpui::TestAppContext) {
    let test_name = concat!(
        module_path!(),
        "::the_app_diff_scrolls_a_long_line_sideways"
    );
    crate::app::test_support::with_prepared_offline_app(
        test_name,
        cx,
        long_line_repository,
        |cx, app, _, _| {
            cx.simulate_resize(gpui::size(gpui::px(900.0), gpui::px(700.0)));
            cx.update(|_, cx| {
                app.update(cx, |app, cx| {
                    app.project.repository.execution_allowed = true;
                    app.workspace.run_panel_hidden = false;
                    app.request_repository_refresh(cx);
                });
            });
            wait_for_repository(cx, app);
            open_diff(cx, app, std::path::Path::new("long.txt"));
            pump(cx, app);
            pump(cx, app);
            let reading = cx.update(|_, cx| {
                let diff = app.read(cx).active_diff().expect("the diff");
                (
                    diff.split,
                    diff.scroll.max_offset().x,
                    diff.scroll.max_offset().y,
                    diff.rows.len(),
                )
            });
            assert!(reading.0, "a diff opens side by side");
            assert!(
                f32::from(reading.1) > 0.0,
                "a line wider than the window should scroll sideways, but the reading ends at {:?}",
                reading.1
            );
            assert!(
                f32::from(reading.2) >= reading.3 as f32 * 18.0 - 700.0,
                "the whole reading should be scrollable, {} rows end at {:?}",
                reading.3,
                reading.2
            );
        },
    );
}

#[gpui::test]
fn the_app_diff_opens_a_new_file_as_one_side(cx: &mut gpui::TestAppContext) {
    let test_name = concat!(
        module_path!(),
        "::the_app_diff_opens_a_new_file_as_one_side"
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
            open_diff(cx, app, std::path::Path::new("fresh.txt"));
            let paired = cx.update(|_, cx| {
                let diff = app.read(cx).active_diff().expect("the diff");
                let file = diff.diff.as_ref().expect("a diff");
                assert!(file.is_new_file(), "an untracked file is a new file");
                assert_eq!(file.hunks.len(), 1, "the whole file is one block");
                assert!(
                    file.spans().iter().all(Vec::is_empty),
                    "a new file has nothing between its blocks to show"
                );
                assert!(diff.split);
                diff.rows
                    .iter()
                    .filter(|row| matches!(row, crate::repository::DiffRow::Split { .. }))
                    .count()
            });
            assert_eq!(paired, 0, "a new file is not read against an empty column");
            pump(cx, app);
            pump(cx, app);
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
