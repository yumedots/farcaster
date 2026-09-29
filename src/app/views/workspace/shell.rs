use gpui::{
    InteractiveElement as _, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, WeakEntity, div,
};

use crate::app::{
    FarcasterApp, PickerScope,
    ui::{
        assets::AppIcon,
        layout::{LayoutMode, shows_left_inline, shows_right_inline},
        primitives::{AppTooltip as _, ButtonTone, icon_button},
        theme::theme,
    },
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
            .child(self.render_folder_counts(entity))
    }

    pub(in crate::app::views) fn render_folder_counts(
        &self,
        entity: WeakEntity<Self>,
    ) -> impl IntoElement {
        let live = self
            .sessions
            .visible
            .iter()
            .filter(|session| !session.archived)
            .map(|session| (session.app_session_id, session.project.as_path()))
            .collect::<Vec<_>>();
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(theme().space.xs)
            .children(self.sessions.folders.folders.iter().map(|folder| {
                let count = live
                    .iter()
                    .filter(|(id, project)| {
                        self.sessions.folders.folder_for_session(*id, project) == Some(folder.id)
                    })
                    .count();
                let tooltip = if count == 1 {
                    format!("{} · 1 session", folder.name)
                } else {
                    format!("{} · {count} sessions", folder.name)
                };
                let folder_id = folder.id;
                let click = entity.clone();
                div()
                    .id(format!("folder-count-{folder_id}"))
                    .flex_none()
                    .px(theme().space.xs)
                    .rounded(theme().radius)
                    .text_size(theme().type_scale.caption)
                    .text_color(theme().colors.muted)
                    .cursor_pointer()
                    .hover(|chip| chip.bg(theme().colors.highlight))
                    .app_tooltip(tooltip)
                    .on_click(move |_, window, cx| {
                        let _ = click.update(cx, |this, cx| {
                            this.open_folder_sessions(folder_id, window, cx)
                        });
                    })
                    .child(count.to_string())
            }))
    }
}
