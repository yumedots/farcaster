use super::{
    super::super::FarcasterApp,
    repository_presentation::{git_identity, repository_sync_metadata},
};
use crate::{
    app::{
        RunPanelView,
        ui::{
            assets::AppIcon,
            change_tree::{ChangeSort, ChangeView},
            primitives::{
                AppTooltip as _, ButtonTone, activates_button, dropdown_button, icon_button,
                icon_control, section_heading,
            },
            theme::{MONO_FONT_FAMILY, theme},
        },
    },
    repository::{RepositoryEdit, RepositorySyncAction, WorkingCopySnapshot},
};
use gpui::{
    AnyElement, App, Div, ElementId, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div,
    prelude::FluentBuilder as _,
};

use gpui_component::{
    Disableable as _, Sizable as _, Size,
    button::{Button, ButtonVariants as _},
    menu::{DropdownMenu as _, PopupMenuItem},
};

pub(super) fn file_action(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on_press: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let press = std::rc::Rc::new(on_press);
    let click = press.clone();
    icon_control(id, label)
        .size(theme().size(20.0))
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            click(window, cx);
        })
        .on_key_down(move |event, window, cx| {
            if activates_button(event) {
                cx.stop_propagation();
                press(window, cx);
            }
        })
}

pub(super) fn repository_header(
    app: &FarcasterApp,
    snapshot: Option<&WorkingCopySnapshot>,
    entity: WeakEntity<FarcasterApp>,
    panel: WeakEntity<RunPanelView>,
    filtering: bool,
) -> AnyElement {
    let refresh = entity.clone();
    let clear = entity.clone();
    let clear_selection = entity.clone();
    let commit = entity.clone();
    let staged_commit = entity.clone();
    let selected_count = app.project.repository.edits.selection.paths.len();
    let enabled = app.project.repository.execution_allowed;
    let syncing = app.project.repository.sync.action;
    let pending = app.project.repository.edits.pending.is_some();
    let commit_like_vscode = app.settings.stage_changes_like_vscode;
    let count = snapshot.map_or(0, |snapshot| {
        snapshot
            .changes
            .iter()
            .map(|change| &change.relative_path)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    });
    let staged = snapshot.map_or_else(std::collections::BTreeSet::new, |snapshot| {
        snapshot
            .changes
            .iter()
            .filter(|change| change.layer == crate::repository::ChangeLayer::Index)
            .map(|change| change.relative_path.clone())
            .collect::<std::collections::BTreeSet<_>>()
    });
    let menu = repository_actions(app, snapshot, entity, panel, filtering);

    div()
        .flex_none()
        .flex()
        .flex_col()
        .gap(theme().space.xs)
        .child(
            div()
                .flex()
                .items_center()
                .gap(theme().space.xs)
                .child(section_heading(if commit_like_vscode {
                    "Source control"
                } else {
                    "Changes"
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_ellipsis()
                        .text_size(theme().type_scale.caption)
                        .text_color(theme().colors.muted)
                        .when(snapshot.is_some(), |label| {
                            label.child(if selected_count > 0 {
                                format!("· {selected_count} selected")
                            } else {
                                format!("· {count}")
                            })
                        }),
                )
                .when(
                    snapshot.is_some() && !commit_like_vscode && selected_count > 0,
                    |row| {
                        let selection_button = |id, icon, label: &'static str| {
                            Button::new(id)
                                .icon(icon)
                                .with_size(Size::Small)
                                .disabled(syncing.is_some() || pending)
                                .accessibility_label(label)
                                .tooltip(label)
                        };
                        row.child(
                            selection_button(
                                "clear-selected-files",
                                AppIcon::X,
                                "Clear file selection",
                            )
                            .ghost()
                            .on_click(move |_, _, cx| {
                                let _ = clear.update(cx, |this, cx| {
                                    this.clear_repository_selection(cx);
                                });
                            }),
                        )
                        .child(
                            selection_button(
                                "commit-selected-files",
                                AppIcon::Check,
                                "Review and commit selected files",
                            )
                            .primary()
                            .on_click(move |_, window, cx| {
                                let _ = commit.update(cx, |this, cx| {
                                    this.review_repository_edit(
                                        RepositoryEdit::Commit,
                                        None,
                                        window,
                                        cx,
                                    );
                                });
                            }),
                        )
                    },
                )
                .when(commit_like_vscode && selected_count > 0, |row| {
                    row.child(
                        Button::new("clear-selected-files")
                            .icon(AppIcon::X)
                            .with_size(Size::Small)
                            .disabled(syncing.is_some() || pending)
                            .accessibility_label("Clear file selection")
                            .tooltip("Clear file selection")
                            .ghost()
                            .on_click(move |_, _, cx| {
                                let _ = clear_selection.update(cx, |this, cx| {
                                    this.clear_repository_selection(cx);
                                });
                            }),
                    )
                })
                .when(commit_like_vscode && !staged.is_empty(), |row| {
                    let staged_commit_paths = staged.clone();
                    let label = format!("Commit {} staged changes", staged.len());
                    row.child(
                        Button::new("commit-staged-changes")
                            .icon(AppIcon::Check)
                            .with_size(Size::Small)
                            .disabled(syncing.is_some() || pending)
                            .accessibility_label(label.clone())
                            .tooltip(label)
                            .primary()
                            .on_click(move |_, window, cx| {
                                let _ = staged_commit.update(cx, |this, cx| {
                                    this.review_repository_paths(
                                        RepositoryEdit::CommitIndex,
                                        staged_commit_paths.clone(),
                                        window,
                                        cx,
                                    );
                                });
                            }),
                    )
                })
                .when(enabled, |row| {
                    row.child(icon_button(
                        "refresh-working-copy",
                        AppIcon::ArrowsClockwise,
                        "Refresh working copy",
                        ButtonTone::Quiet,
                        move |_, cx| {
                            let _ =
                                refresh.update(cx, |this, cx| this.request_repository_refresh(cx));
                        },
                    ))
                })
                .child(menu),
        )
        .when_some(snapshot, |section, snapshot| {
            let metadata = repository_sync_metadata(&snapshot.identity);
            let detail = format!("{} · {metadata}", repository_identity_label(snapshot));
            section.child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme().space.sm)
                    .text_size(theme().type_scale.caption)
                    .child(working_copy_totals(
                        app.project.repository.additions,
                        app.project.repository.deletions,
                    ))
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("repository-identity")
                            .min_w_0()
                            .text_ellipsis()
                            .font_family(MONO_FONT_FAMILY)
                            .text_color(theme().colors.subtle)
                            .app_tooltip(detail.clone())
                            .child(repository_identity_label(snapshot)),
                    ),
            )
        })
        .when_some(syncing, |section, action| {
            section.child(
                div()
                    .text_size(theme().type_scale.caption)
                    .text_color(theme().colors.accent)
                    .child(match action {
                        RepositorySyncAction::PullOrFetch => "Syncing repository…",
                        RepositorySyncAction::Push => "Pushing repository…",
                    }),
            )
        })
        .into_any_element()
}

fn repository_actions(
    app: &FarcasterApp,
    snapshot: Option<&WorkingCopySnapshot>,
    entity: WeakEntity<FarcasterApp>,
    panel: WeakEntity<RunPanelView>,
    filtering: bool,
) -> impl IntoElement {
    let project = app.project.repository.project.clone();
    let enabled = app.project.repository.execution_allowed;
    let syncing = app.project.repository.sync.action;
    let identity = snapshot.map(|snapshot| snapshot.identity.clone());
    let current_view = app.settings.source_control_view;
    let current_sort = app.settings.source_control_sort;
    dropdown_button("repository-actions", "⋯", ButtonTone::Quiet, true)
        .dropdown_caret(false)
        .size(theme().controls.icon_button)
        .px(gpui::px(0.0))
        .accessibility_label("Repository actions")
        .tooltip("Repository actions")
        .dropdown_menu(move |mut menu, _, _| {
            for (label, open) in [
                ("Expand all folders", true),
                ("Collapse all folders", false),
            ] {
                let panel = panel.clone();
                let project = project.clone();
                menu = menu.item(PopupMenuItem::new(label).disabled(filtering).on_click(
                    move |_, _, cx| {
                        let _ = panel.update(cx, |view, cx| {
                            view.changes.set_all(&project, open);
                            cx.notify();
                        });
                    },
                ));
            }
            menu = menu.separator();
            for (view, label) in [
                (ChangeView::Tree, "View as tree"),
                (ChangeView::List, "View as list"),
            ] {
                let entity = entity.clone();
                menu = menu.item(
                    PopupMenuItem::new(label)
                        .checked(view == current_view)
                        .on_click(move |_, _, cx| {
                            let _ = entity
                                .update(cx, |this, cx| this.set_source_control_view(view, cx));
                        }),
                );
            }
            for sort in ChangeSort::ALL {
                let entity = entity.clone();
                menu = menu.item(
                    PopupMenuItem::new(sort.label())
                        .checked(sort == current_sort)
                        .on_click(move |_, _, cx| {
                            let _ = entity
                                .update(cx, |this, cx| this.set_source_control_sort(sort, cx));
                        }),
                );
            }
            menu = menu.separator();
            for action in [
                RepositorySyncAction::PullOrFetch,
                RepositorySyncAction::Push,
            ] {
                let entity = entity.clone();
                let label = match action {
                    RepositorySyncAction::PullOrFetch => "Pull repository",
                    RepositorySyncAction::Push => "Push repository",
                };
                let available = enabled
                    && syncing.is_none()
                    && identity
                        .as_ref()
                        .is_some_and(|identity| action.is_available_for(identity));
                menu = menu.item(PopupMenuItem::new(label).disabled(!available).on_click(
                    move |_, _, cx| {
                        let _ =
                            entity.update(cx, |this, cx| this.request_repository_sync(action, cx));
                    },
                ));
            }
            menu
        })
}

fn working_copy_totals(additions: Option<u64>, deletions: Option<u64>) -> AnyElement {
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(theme().space.xs)
        .child(
            div()
                .text_color(theme().colors.success)
                .child(additions.map_or_else(|| "+—".to_owned(), |count| format!("+{count}"))),
        )
        .child(
            div()
                .text_color(theme().colors.error)
                .child(deletions.map_or_else(|| "-—".to_owned(), |count| format!("-{count}"))),
        )
        .into_any_element()
}

fn repository_identity_label(snapshot: &WorkingCopySnapshot) -> String {
    git_identity(&snapshot.identity)
}
