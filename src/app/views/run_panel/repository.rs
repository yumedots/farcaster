use super::{
    super::super::FarcasterApp,
    repository_controls::{file_action, repository_header},
    repository_presentation::{
        ChangeSection, accessible_change_path, bounded_message, change_color, change_kind_label,
        change_sort_key, change_status_label, display_change_path, file_path_labels, group_title,
        repository_row_id,
    },
};
use crate::{
    app::ui::theme::theme,
    app::ui::{
        assets::AppIcon,
        file_icons::file_icon,
        primitives::{
            AppIconSize, AppTooltip as _, ContextMenuTrigger, SearchField, activates_button,
            app_icon, section_heading,
        },
    },
    repository::{RepositoryEdit, WorkingCopyChange, WorkingCopySnapshot},
};
use gpui::{
    Anchor, AnyElement, ClipboardItem, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Role, SharedString, StatefulInteractiveElement as _, Styled as _,
    WeakEntity, div, prelude::FluentBuilder as _, px,
};
use gpui_component::menu::{DropdownMenu as _, PopupMenu, PopupMenuItem};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    rc::Rc,
};

use super::{
    RepositoryView,
    change_tree::{self, ChangeView, TreeRow},
};
use crate::app::RunPanelView;

impl FarcasterApp {
    pub(super) fn render_repository(
        &self,
        entity: WeakEntity<Self>,
        panel: WeakEntity<RunPanelView>,
        browser: &RepositoryView<'_>,
    ) -> AnyElement {
        let snapshot = self.project.repository.snapshot.as_ref();
        let header = repository_header(
            self,
            snapshot,
            entity.clone(),
            panel.clone(),
            !browser.query.trim().is_empty(),
        );

        div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(theme().space.xs)
            .child(header)
            .when(
                self.project.repository.loading && !self.project.repository.initialized,
                |section| {
                    section.child(
                        div()
                            .id("repository-loading")
                            .role(Role::Status)
                            .text_size(theme().type_scale.caption)
                            .text_color(theme().colors.accent)
                            .child("Reading working copy…"),
                    )
                },
            )
            .when_some(
                self.project.repository.watcher_error.as_deref(),
                |section, error| {
                    section.child(repository_error_notice(
                        "Auto-refresh unavailable. Use Refresh to try again.",
                        error,
                        theme().colors.warning,
                    ))
                },
            )
            .when_some(
                self.project.repository.sync.error.as_deref(),
                |section, error| {
                    section.child(repository_notice(
                        &format!("Repository sync failed: {}", bounded_message(error)),
                        theme().colors.error,
                    ))
                },
            )
            .when_some(
                self.project.repository.error.as_deref(),
                |section, error| {
                    let message = if self.project.repository.snapshot.is_some() {
                        "Could not refresh changes. Showing the previous result."
                    } else {
                        "Could not read this repository. Check the project folder and refresh."
                    };
                    section.child(repository_error_notice(
                        message,
                        error,
                        theme().colors.error,
                    ))
                },
            )
            .when_some(snapshot, |section, snapshot| {
                section
                    .child(
                        SearchField::new("repository-search", browser.search)
                            .accessible_label("Filter changed files"),
                    )
                    .child(self.repository_changes(
                        snapshot,
                        entity.clone(),
                        panel.clone(),
                        browser,
                    ))
            })
            .when(!self.project.repository.execution_allowed, |section| {
                section.child(
                    div()
                        .id("repository-disabled")
                        .role(Role::Status)
                        .text_size(theme().type_scale.caption)
                        .text_color(theme().colors.warning)
                        .child("Repository integration is disabled for this untrusted project"),
                )
            })
            .when(
                self.project.repository.execution_allowed
                    && self.project.repository.snapshot.is_none()
                    && self.project.repository.error.is_none()
                    && self.project.repository.initialized,
                |section| {
                    section.child(
                        div()
                            .id("repository-not-found")
                            .role(Role::Status)
                            .text_size(theme().type_scale.caption)
                            .text_color(theme().colors.subtle)
                            .child("No repository found for this project"),
                    )
                },
            )
            .into_any_element()
    }

    fn repository_changes(
        &self,
        snapshot: &WorkingCopySnapshot,
        entity: WeakEntity<Self>,
        panel: WeakEntity<RunPanelView>,
        browser: &RepositoryView<'_>,
    ) -> AnyElement {
        let mut list = div()
            .id("repository-files")
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .track_scroll(browser.scroll);
        let mut shown = 0;
        if self.settings.stage_changes_like_vscode {
            for section in ChangeSection::ALL {
                // One file can appear under two sections, so they render apart.
                let rows = self.repository_rows(
                    snapshot,
                    Some(section),
                    entity.clone(),
                    panel.clone(),
                    browser,
                );
                if rows.is_empty() {
                    continue;
                }
                shown += rows.len();
                list = list.child(
                    div()
                        .flex()
                        .flex_col()
                        .child(self.repository_section_header(snapshot, section, entity.clone()))
                        .children(rows),
                );
            }
        } else {
            let rows = self.repository_rows(snapshot, None, entity.clone(), panel.clone(), browser);
            shown = rows.len();
            list = list.children(rows);
        }
        list.when(shown == 0, |list| {
            list.child(repository_notice(
                if !browser.query.trim().is_empty() {
                    "No matching files"
                } else {
                    "Working tree and index are clean"
                },
                theme().colors.subtle,
            ))
        })
        .into_any_element()
    }

    fn repository_rows(
        &self,
        snapshot: &WorkingCopySnapshot,
        section: Option<ChangeSection>,
        entity: WeakEntity<Self>,
        panel: WeakEntity<RunPanelView>,
        browser: &RepositoryView<'_>,
    ) -> Vec<AnyElement> {
        let prefix = section.map_or("", ChangeSection::key);
        let sort = self.settings.source_control_sort;
        if self.settings.source_control_view == ChangeView::List {
            let mut indices = snapshot
                .changes
                .iter()
                .enumerate()
                .filter(|(_, change)| section.is_none_or(|section| section.matches(change.layer)))
                .filter(|(_, change)| {
                    change_tree::matches(
                        &change.relative_path,
                        change.original_relative_path.as_deref(),
                        browser.query,
                    )
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            indices.sort_by(|left, right| {
                let left = &snapshot.changes[*left];
                let right = &snapshot.changes[*right];
                change_sort_key(sort, &left.relative_path, &left.kind).cmp(&change_sort_key(
                    sort,
                    &right.relative_path,
                    &right.kind,
                ))
            });
            let visible = indices
                .iter()
                .map(|index| snapshot.changes[*index].relative_path.clone())
                .collect::<Vec<PathBuf>>();
            return indices
                .iter()
                .filter_map(|index| {
                    self.repository_change_row(
                        &snapshot.changes[*index],
                        entity.clone(),
                        ChangeRowSpec {
                            section: prefix,
                            visible: &visible,
                            parent_path: true,
                        },
                    )
                })
                .collect();
        }
        let rows = change_tree::rows(
            snapshot
                .changes
                .iter()
                .enumerate()
                .filter(|(_, change)| section.is_none_or(|section| section.matches(change.layer)))
                .map(|(index, change)| {
                    (
                        index,
                        change.relative_path.as_path(),
                        change.original_relative_path.as_deref(),
                        change.counts,
                    )
                }),
            browser.query,
            &self.project.repository.project,
            browser.state,
        );
        let visible = rows
            .iter()
            .filter_map(|row| match row {
                TreeRow::File { index, .. } => Some(snapshot.changes[*index].relative_path.clone()),
                TreeRow::Folder { .. } => None,
            })
            .collect::<Vec<PathBuf>>();
        rows.iter()
            .filter_map(|row| match row {
                TreeRow::Folder {
                    path,
                    label,
                    count,
                    counts,
                    depth,
                    open,
                } => {
                    let project = self.project.repository.project.clone();
                    let path = path.clone();
                    let panel = panel.clone();
                    let accessible = format!(
                        "{} {}",
                        if *open { "Collapse" } else { "Expand" },
                        path.display()
                    );
                    Some(
                        crate::app::ui::primitives::tree_folder_row(
                            repository_folder_id(prefix, &path),
                            label.clone(),
                            *depth,
                            *open,
                            browser.query.trim().is_empty(),
                            move |_, cx| {
                                let _ = panel.update(cx, |view, cx| {
                                    view.changes.toggle(&project, &path);
                                    cx.notify();
                                });
                            },
                        )
                        .aria_label(accessible)
                        .when(!*open, |row| {
                            row.child(crate::app::ui::primitives::folder_change_summary(
                                *count, *counts,
                            ))
                        })
                        .into_any_element(),
                    )
                }
                TreeRow::File { index, depth } => self
                    .repository_change_row(
                        &snapshot.changes[*index],
                        entity.clone(),
                        ChangeRowSpec {
                            section: prefix,
                            visible: &visible,
                            parent_path: false,
                        },
                    )
                    .map(|row| {
                        div()
                            .pl(px(*depth as f32 * 12.0))
                            .child(row)
                            .into_any_element()
                    }),
            })
            .collect()
    }

    fn repository_section_header(
        &self,
        snapshot: &WorkingCopySnapshot,
        section: ChangeSection,
        entity: WeakEntity<Self>,
    ) -> AnyElement {
        let group: SharedString = format!("repository-section-{}", section.key()).into();
        let staged = section == ChangeSection::Staged;
        let paths = section_paths(snapshot, section);
        let editable = self.project.repository.execution_allowed
            && self.project.repository.sync.action.is_none()
            && self.project.repository.edits.pending.is_none()
            && !self.project.repository.edits.staging;
        let mut header = div()
            .id(format!("repository-section-{}", section.key()))
            .flex_none()
            .flex()
            .items_center()
            .gap(theme().space.xs)
            .h(theme().size(24.0))
            .px(theme().space.xs)
            .rounded(theme().radius)
            .group(group.clone())
            .child(section_heading(section.title()))
            .child(
                div()
                    .text_size(theme().type_scale.caption)
                    .text_color(theme().colors.muted)
                    .child(format!("· {}", paths.len())),
            )
            .child(div().flex_1());
        if section != ChangeSection::Merge {
            let stage_entity = entity.clone();
            let stage_paths = paths.clone();
            let (icon, label) = if staged {
                (AppIcon::Minus, "Unstage all changes")
            } else {
                (AppIcon::Plus, "Stage all changes")
            };
            let action = if staged {
                RepositoryEdit::Unstage
            } else {
                RepositoryEdit::Stage
            };
            header = header.child(
                file_action(
                    format!("stage-repository-section-{}", section.key()),
                    label,
                    move |_, cx| {
                        if editable {
                            let _ = stage_entity.update(cx, |this, cx| {
                                this.stage_repository_paths(action, stage_paths.clone(), cx)
                            });
                        }
                    },
                )
                .opacity(0.0)
                .group_hover(group.clone(), |control| control.opacity(1.0))
                .focus_visible(|control| control.opacity(1.0))
                .child(app_icon(icon, AppIconSize::Inline)),
            );
        }
        if section == ChangeSection::Changes {
            let discard_entity = entity.clone();
            let discard_paths = paths.clone();
            header = header.child(
                file_action(
                    format!("discard-repository-section-{}", section.key()),
                    "Discard all changes…",
                    move |window, cx| {
                        if editable {
                            let _ = discard_entity.update(cx, |this, cx| {
                                this.review_repository_paths(
                                    RepositoryEdit::Discard,
                                    discard_paths.clone(),
                                    window,
                                    cx,
                                )
                            });
                        }
                    },
                )
                .opacity(0.0)
                .group_hover(group, |control| control.opacity(1.0))
                .focus_visible(|control| control.opacity(1.0))
                .child(app_icon(
                    AppIcon::ArrowCounterClockwise,
                    AppIconSize::Inline,
                )),
            );
        }
        header.into_any_element()
    }

    fn repository_change_row(
        &self,
        change: &WorkingCopyChange,
        entity: WeakEntity<Self>,
        spec: ChangeRowSpec<'_>,
    ) -> Option<AnyElement> {
        let ChangeRowSpec {
            section,
            visible,
            parent_path,
        } = spec;
        let focus = self
            .project
            .repository
            .row_focus
            .get(&change.target.key)?
            .clone();
        let click_entity = entity.clone();
        let click_path = change.target.absolute_path();
        let key_entity = entity.clone();
        let key_path = change.target.absolute_path();
        let row_id = repository_row_id(&change.target.key, section);
        let action_group: gpui::SharedString = format!("repository-actions-{row_id}").into();
        let staging_like_vscode = self.settings.stage_changes_like_vscode;
        let staged = change.layer == crate::repository::ChangeLayer::Index;
        let selecting = !self.project.repository.edits.selection.paths.is_empty();
        let selected = self
            .project
            .repository
            .edits
            .selection
            .paths
            .contains(&change.relative_path);
        let select_entity = entity.clone();
        let select_path = change.relative_path.clone();
        let stage_entity = entity.clone();
        let stage_paths = BTreeSet::from([change.relative_path.clone()]);
        let relative_path = change.relative_path.clone();
        let open_layer = change.layer;
        let key_open_key = staging_like_vscode.then(|| change.target.key.clone());
        let key_open_relative = change.relative_path.clone();
        let menu_key = change.target.key.clone();
        let menu_entity = entity.clone();
        let menu_paths = self.repository_menu_paths(&change.relative_path);
        let menu_absolute = change.target.absolute_path();
        let menu_relative = change.relative_path.clone();
        let change_key = change.target.key.clone();
        let extend_visible = Rc::new(visible.to_vec());
        let click_visible = extend_visible.clone();
        let discard_path = change.relative_path.clone();
        let editable = self.project.repository.execution_allowed
            && self.project.repository.sync.action.is_none()
            && self.project.repository.edits.pending.is_none()
            && !self.project.repository.edits.staging
            && change.kind != crate::repository::ChangeKind::Conflict;
        let discardable = editable
            && (!staging_like_vscode || change.layer != crate::repository::ChangeLayer::Index)
            && (staging_like_vscode || !selecting);
        let full_path = format!("{} · ⌥ Open diff", display_change_path(change));
        let (filename, parent) = file_path_labels(&change.relative_path);
        let status = change_status_label(change).to_owned();
        let layer = group_title(change.layer);
        let state = change_kind_label(&change.kind);
        let accessible_path = accessible_change_path(change);
        let accessible = format!("Edit {layer} {state} file {accessible_path} in the editor");
        let target = div()
            .id(("repository-change", row_id))
            .group(action_group.clone())
            .track_focus(&focus)
            .role(Role::Button)
            .aria_label(accessible)
            .app_tooltip(full_path.clone())
            .tab_index(0)
            .on_mouse_down(
                gpui::MouseButton::Left,
                crate::app::ui::primitives::preserve_pointer_focus,
            )
            .min_w_0()
            .w_full()
            .h(theme().size(24.0))
            .px(theme().space.xs)
            .rounded(theme().radius)
            .flex()
            .items_center()
            .gap(theme().space.xs)
            .hover(|row| row.bg(theme().colors.highlight))
            .when(selected, |row| row.bg(theme().colors.highlight))
            .focus(|row| row.bg(theme().colors.highlight))
            .cursor_pointer()
            .on_click(move |event, window, cx| {
                let modifiers = event.modifiers();
                if staging_like_vscode && (modifiers.platform || modifiers.control) {
                    let _ = click_entity.update(cx, |this, cx| {
                        this.toggle_repository_selection(relative_path.clone(), cx)
                    });
                    return;
                }
                if staging_like_vscode && modifiers.shift {
                    let visible = (*click_visible).clone();
                    let _ = click_entity.update(cx, |this, cx| {
                        this.extend_repository_selection(relative_path.clone(), visible, cx)
                    });
                    return;
                }
                // Without the option key a click opens the diff in the app, the
                // way the source control view does; the key still reaches the
                // editor the way it always has.
                if staging_like_vscode && !modifiers.alt {
                    let key = change_key.clone();
                    let _ = click_entity.update(cx, |this, cx| {
                        this.open_repository_diff(
                            key.clone(),
                            relative_path.clone(),
                            open_layer,
                            window,
                            cx,
                        )
                    });
                    return;
                }
                let _ = click_entity.update(cx, |this, cx| {
                    this.open_file_editor_with_diff(
                        click_path.clone(),
                        None,
                        modifiers.alt,
                        window,
                        cx,
                    );
                });
            })
            .on_key_down(move |event, window, cx| {
                if activates_button(event) {
                    cx.stop_propagation();
                    let Some(key) = key_open_key.clone() else {
                        let _ = key_entity.update(cx, |this, cx| {
                            this.open_file_editor(key_path.clone(), window, cx);
                        });
                        return;
                    };
                    let _ = key_entity.update(cx, |this, cx| {
                        this.open_repository_diff(
                            key,
                            key_open_relative.clone(),
                            open_layer,
                            window,
                            cx,
                        )
                    });
                }
            })
            // The checkbox is the only leading control; every action in the VS
            // Code panel sits on the right, next to discard.
            .when(!staging_like_vscode, |row| {
                row.child(
                    file_action(
                        ("select-repository-file", row_id),
                        format!(
                            "{} {} for commit",
                            if selected { "Deselect" } else { "Select" },
                            change.relative_path.display()
                        ),
                        move |_, cx| {
                            if editable {
                                let _ = select_entity.update(cx, |this, cx| {
                                    this.toggle_repository_file(select_path.clone(), cx)
                                });
                            }
                        },
                    )
                    .role(Role::CheckBox)
                    .aria_toggled(if selected {
                        gpui::Toggled::True
                    } else {
                        gpui::Toggled::False
                    })
                    .when(!selecting, |control| {
                        control
                            .opacity(0.0)
                            .group_hover(action_group.clone(), |control| control.opacity(1.0))
                            .focus_visible(|control| control.opacity(1.0))
                    })
                    .child(
                        div()
                            .size(theme().size(14.0))
                            .border(theme().border)
                            .border_color(theme().colors.muted)
                            .when(!editable, |checkbox| checkbox.opacity(0.4))
                            .rounded(theme().radius)
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(selected, |checkbox| {
                                checkbox.bg(theme().colors.accent).child(
                                    app_icon(AppIcon::Check, AppIconSize::Inline)
                                        .text_color(theme().colors.surface),
                                )
                            }),
                    ),
                )
            })
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap(theme().space.xs)
                    .child(file_icon(&change.relative_path))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .flex()
                            .items_center()
                            .gap(theme().space.xs)
                            .overflow_hidden()
                            .text_size(theme().type_scale.caption)
                            .text_color(theme().colors.text)
                            // The name keeps its natural width and only shrinks
                            // when pressed, ending in an ellipsis instead of
                            // running under the row actions.
                            .child(
                                div()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(filename),
                            )
                            .when(parent_path && !parent.is_empty(), |label| {
                                // A folder path matters most at its tail, so it
                                // gives up its head first.
                                label.child(
                                    div()
                                        .min_w_0()
                                        .flex_1()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_color(theme().colors.subtle)
                                        .text_ellipsis_start()
                                        .child(parent.clone()),
                                )
                            }),
                    )
                    .when(
                        !staging_like_vscode
                            && change.layer == crate::repository::ChangeLayer::Index,
                        |label| {
                            label.child(
                                div()
                                    .text_size(theme().type_scale.caption)
                                    .text_color(theme().colors.subtle)
                                    .child("Staged"),
                            )
                        },
                    ),
            )
            .when(staging_like_vscode, |row| {
                let action = if staged {
                    RepositoryEdit::Unstage
                } else {
                    RepositoryEdit::Stage
                };
                row.child(
                    div().w(theme().size(20.0)).flex_none().child(
                        file_action(
                            ("stage-repository-file", row_id),
                            format!(
                                "{} {}",
                                if staged { "Unstage" } else { "Stage" },
                                change.relative_path.display()
                            ),
                            move |_, cx| {
                                if editable {
                                    let _ = stage_entity.update(cx, |this, cx| {
                                        this.stage_repository_paths(action, stage_paths.clone(), cx)
                                    });
                                }
                            },
                        )
                        .opacity(0.0)
                        .group_hover(action_group.clone(), |control| control.opacity(1.0))
                        .focus_visible(|control| control.opacity(1.0))
                        .child(app_icon(
                            if staged {
                                AppIcon::Minus
                            } else {
                                AppIcon::Plus
                            },
                            AppIconSize::Inline,
                        )),
                    ),
                )
            })
            .child(
                div()
                    .w(theme().size(20.0))
                    .flex_none()
                    .when(discardable, |slot| {
                        slot.child(
                            file_action(
                                ("discard-repository-file", row_id),
                                if matches!(
                                    change.kind,
                                    crate::repository::ChangeKind::Added
                                        | crate::repository::ChangeKind::Untracked
                                ) {
                                    "Delete file…"
                                } else {
                                    "Discard file changes…"
                                },
                                move |window, cx| {
                                    let _ = entity.update(cx, |this, cx| {
                                        this.review_repository_edit(
                                            RepositoryEdit::Discard,
                                            Some(discard_path.clone()),
                                            window,
                                            cx,
                                        )
                                    });
                                },
                            )
                            .opacity(0.0)
                            .group_hover(action_group, |control| control.opacity(1.0))
                            .focus_visible(|control| control.opacity(1.0))
                            .child(app_icon(
                                AppIcon::ArrowCounterClockwise,
                                AppIconSize::Inline,
                            )),
                        )
                    }),
            )
            // The status letter owns the row's right edge, so the action
            // squares always land in the same place beside it.
            .child(
                div()
                    .w(theme().size(16.0))
                    .flex_none()
                    .flex()
                    .justify_center()
                    .text_size(theme().type_scale.caption)
                    .text_color(change_color(&change.kind))
                    .child(status),
            );
        if !staging_like_vscode {
            return Some(target.into_any_element());
        }
        let delete_label = matches!(
            change.kind,
            crate::repository::ChangeKind::Added | crate::repository::ChangeKind::Untracked
        );
        let menu_action = if staged {
            RepositoryEdit::Unstage
        } else {
            RepositoryEdit::Stage
        };
        Some(
            ContextMenuTrigger::new(
                ("repository-change-menu", row_id),
                target.into_any_element(),
            )
            .size_full()
            .dropdown_menu_with_anchor(Anchor::TopLeft, move |menu: PopupMenu, _, _| {
                let mut menu = menu.min_w(theme().size(190.0));
                if editable {
                    let stage_paths = menu_paths.iter().cloned().collect::<BTreeSet<PathBuf>>();
                    let stage_entity = menu_entity.clone();
                    menu = menu.item(
                        PopupMenuItem::new(if staged {
                            "Unstage changes"
                        } else {
                            "Stage changes"
                        })
                        .icon(if staged {
                            AppIcon::Minus
                        } else {
                            AppIcon::Plus
                        })
                        .on_click(move |_, _, cx| {
                            let _ = stage_entity.update(cx, |this, cx| {
                                this.stage_repository_paths(menu_action, stage_paths.clone(), cx)
                            });
                        }),
                    );
                }
                if discardable {
                    let discard_paths = menu_paths.iter().cloned().collect::<BTreeSet<PathBuf>>();
                    let discard_entity = menu_entity.clone();
                    menu = menu.item(
                        PopupMenuItem::new(if delete_label {
                            "Delete file…"
                        } else {
                            "Discard changes…"
                        })
                        .icon(AppIcon::ArrowCounterClockwise)
                        .on_click(move |_, window, cx| {
                            let _ = discard_entity.update(cx, |this, cx| {
                                this.review_repository_paths(
                                    RepositoryEdit::Discard,
                                    discard_paths.clone(),
                                    window,
                                    cx,
                                )
                            });
                        }),
                    );
                }
                let open_entity = menu_entity.clone();
                let open_path = menu_absolute.clone();
                let diff_entity = menu_entity.clone();
                let diff_key = menu_key.clone();
                let diff_relative = menu_relative.clone();
                let copy_absolute = menu_absolute.clone();
                let copy_relative = menu_relative.clone();
                menu.separator()
                    .item(
                        PopupMenuItem::new("Open diff").on_click(move |_, window, cx| {
                            let _ = diff_entity.update(cx, |this, cx| {
                                this.open_repository_diff(
                                    diff_key.clone(),
                                    diff_relative.clone(),
                                    open_layer,
                                    window,
                                    cx,
                                )
                            });
                        }),
                    )
                    .item(PopupMenuItem::new("Open file in editor").on_click(
                        move |_, window, cx| {
                            let _ = open_entity.update(cx, |this, cx| {
                                this.open_file_editor_with_diff(
                                    open_path.clone(),
                                    None,
                                    false,
                                    window,
                                    cx,
                                )
                            });
                        },
                    ))
                    .item(PopupMenuItem::new("Copy path").on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(
                            copy_absolute.display().to_string(),
                        ));
                    }))
                    .item(
                        PopupMenuItem::new("Copy relative path").on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(
                                copy_relative.display().to_string(),
                            ));
                        }),
                    )
            })
            .mouse_button(MouseButton::Right)
            .anchor_to_cursor()
            .into_any_element(),
        )
    }
}

struct ChangeRowSpec<'a> {
    section: &'a str,
    visible: &'a [PathBuf],
    parent_path: bool,
}

fn section_paths(snapshot: &WorkingCopySnapshot, section: ChangeSection) -> BTreeSet<PathBuf> {
    snapshot
        .changes
        .iter()
        .filter(|change| section.matches(change.layer))
        .map(|change| change.relative_path.clone())
        .collect()
}

fn repository_folder_id(section: &str, path: &Path) -> String {
    if section.is_empty() {
        format!("repository-folder-{}", path.display())
    } else {
        format!("repository-folder-{section}-{}", path.display())
    }
}

fn repository_error_notice(message: &str, detail: &str, color: gpui::Rgba) -> AnyElement {
    let detail = bounded_message(detail);
    repository_notice(message, color)
        .min_w_0()
        .app_tooltip(detail.clone())
        .into_any_element()
}

fn repository_notice(message: &str, color: gpui::Rgba) -> gpui::Stateful<gpui::Div> {
    div()
        .id(message.to_owned())
        .role(Role::Status)
        .text_size(theme().type_scale.caption)
        .text_color(color)
        .child(message.to_owned())
}

#[cfg(test)]
#[path = "repository_tests.rs"]
mod tests;
