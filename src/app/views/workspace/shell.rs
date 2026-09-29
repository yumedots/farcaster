use gpui::{IntoElement, ParentElement as _, Styled as _, WeakEntity, div};

use crate::app::{
    FarcasterApp, PickerScope,
    ui::{
        assets::AppIcon,
        layout::{LayoutMode, shows_left_inline, shows_right_inline},
        primitives::{ButtonTone, icon_button},
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
        let actions = entity;
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
    }
}
