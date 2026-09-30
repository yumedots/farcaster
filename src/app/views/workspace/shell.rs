use gpui::{
    InteractiveElement as _, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, WeakEntity, div, prelude::FluentBuilder as _,
};

use crate::app::{
    FarcasterApp, PickerScope,
    ui::{
        assets::AppIcon,
        keybindings::platform_key,
        layout::{LayoutMode, shows_left_inline, shows_right_inline},
        primitives::{AppTooltip as _, ButtonTone, icon_button},
        theme::theme,
    },
    views::session_rail::{VisibleSessionTarget, numbered_session_items},
};

impl FarcasterApp {
    pub(in crate::app::views) fn render_workspace_panels(
        &self,
        mode: LayoutMode,
        entity: WeakEntity<Self>,
    ) -> impl IntoElement {
        let sessions_visible = if shows_left_inline(mode) {
            !self.workspace.session_rail_hidden
        } else {
            self.overlays.view.sessions
        };
        let source_control_visible = if shows_right_inline(mode) {
            !self.workspace.run_panel_hidden
        } else {
            self.overlays.view.run
        };
        let rail_toggle = entity.clone();
        let panel_toggle = entity.clone();
        let actions = entity.clone();
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(theme().space.xs)
            .pr(theme().space.sm)
            .child(self.render_session_numbers(entity))
            .child(icon_button(
                "toggle-session-rail",
                AppIcon::SidebarLeft,
                if sessions_visible {
                    "Hide sessions"
                } else {
                    "Show sessions"
                },
                ButtonTone::Quiet,
                move |window, cx| {
                    let _ = rail_toggle
                        .update(cx, |this, cx| this.toggle_sessions_from_top_bar(window, cx));
                },
            ))
            .child(icon_button(
                "toggle-run-panel",
                AppIcon::GitFork,
                if source_control_visible {
                    "Hide source control"
                } else {
                    "Show source control"
                },
                ButtonTone::Quiet,
                move |window, cx| {
                    let _ = panel_toggle.update(cx, |this, cx| {
                        this.toggle_source_control_from_top_bar(window, cx)
                    });
                },
            ))
            .child(icon_button(
                "session-actions",
                AppIcon::List,
                "Actions",
                ButtonTone::Quiet,
                move |window, cx| {
                    let _ = actions.update(cx, |this, cx| {
                        this.open_picker(PickerScope::Actions, window, cx)
                    });
                },
            ))
    }

    fn render_session_numbers(&self, entity: WeakEntity<Self>) -> impl IntoElement {
        let selected = self.selected_app_session_id();
        let items = self.visible_active_items();
        let folder = self.current_folder_id();
        let numbered = numbered_session_items(&items, &self.sessions.folders, folder);
        let hint = format!(
            "{} + number to switch between {}",
            platform_key("⌘", "Alt"),
            if folder.is_some() {
                "chats in this folder"
            } else {
                "open chats"
            }
        );
        let numbers = numbered
            .iter()
            .enumerate()
            .filter_map(|(index, &(_, item))| {
                let target = VisibleSessionTarget::from_item(item)?;
                let current = selected == Some(target.app_session_id());
                let key = match &target {
                    VisibleSessionTarget::Draft(draft) => format!("draft:{}", draft.id),
                    VisibleSessionTarget::Persisted(session) => format!("session:{}", session.id),
                };
                let number = index + 1;
                let click = entity.clone();
                Some(
                    div()
                        .id(format!("session-number-{key}"))
                        .flex_none()
                        .h(theme().size(20.0))
                        .w(theme().size(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .when(current, |slot| slot.bg(theme().colors.highlight))
                        .text_size(theme().type_scale.caption)
                        .text_center()
                        .text_color(if current {
                            theme().colors.text
                        } else {
                            theme().colors.muted
                        })
                        .cursor_pointer()
                        .hover(|slot| slot.bg(theme().colors.highlight))
                        .app_tooltip(hint.clone())
                        .on_click(move |_, window, cx| {
                            let _ = click.update(cx, |this, cx| {
                                this.select_visible_session(target.clone(), window, cx)
                            });
                        })
                        .child(number.to_string()),
                )
            })
            .collect::<Vec<_>>();
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(theme().space.sm)
            .children(if numbers.len() > 1 {
                numbers
            } else {
                Vec::new()
            })
    }
}
