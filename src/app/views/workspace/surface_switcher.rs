use crate::{
    app::ui::assets::AppIcon,
    app::ui::file_icons::file_icon,
    app::ui::layout::TRAFFIC_LIGHT_INSET,
    app::ui::primitives::{
        AppIconSize, AppTooltip as _, IndicatorEdge, app_icon, icon_control, line_indicator,
    },
    app::ui::theme::theme,
    app::{AppSurface, FarcasterApp, RepositoryDiff},
};
use gpui::{
    AnyElement, Context, ElementId, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, WeakEntity,
    Window, div, prelude::FluentBuilder as _,
};

impl FarcasterApp {
    fn workspace_harness_icon(&self) -> AppIcon {
        self.snapshot
            .selected_session
            .as_deref()
            .and_then(|path| {
                self.sessions
                    .all
                    .iter()
                    .find(|session| session.path == path)
            })
            .map(|session| AppIcon::for_harness(session.harness))
            .or_else(|| {
                let selected = self.sessions.selected_draft.as_deref()?;
                self.sessions
                    .drafts
                    .iter()
                    .find(|draft| draft.id == selected)
                    .map(|draft| AppIcon::for_harness(draft.harness))
            })
            .unwrap_or(AppIcon::Pi)
    }

    fn workspace_label(&self) -> SharedString {
        self.active_harness()
            .map(crate::agents::backend_display_name)
            .unwrap_or_else(|| "Chat".into())
            .into()
    }

    pub(in crate::app::views) fn render_workspace_tabs(
        &self,
        entity: WeakEntity<Self>,
        mode: crate::app::ui::layout::LayoutMode,
    ) -> impl IntoElement {
        let (modifier, chat_hint) = if cfg!(target_os = "macos") {
            (
                "Cmd",
                "Chat composer (Cmd+G in app views; Ctrl+G Ctrl+G anywhere)",
            )
        } else {
            ("Ctrl", "Chat composer (Ctrl+G Ctrl+G anywhere)")
        };
        let surface = self.workspace.surface;
        let active_diff = self.workspace.active_diff.as_ref();
        div()
            .id("workspace-tabs")
            .flex_none()
            .h(theme().size(38.0))
            .flex()
            .items_center()
            .bg(theme().colors.canvas)
            .border_b(theme().border)
            .border_color(theme().colors.border)
            .when(cfg!(target_os = "macos"), |row| {
                row.child(
                    div()
                        .w(theme().size(TRAFFIC_LIGHT_INSET))
                        .h_full()
                        .on_mouse_down(MouseButton::Left, |_, window, _| {
                            window.start_window_move();
                        }),
                )
            })
            .child(
                div()
                    .id("workspace-bar-controls")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .id("workspace-tab-strip")
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .items_center()
                            .overflow_x_scroll()
                            .child(workspace_tab(
                                "workspace-tab-chat",
                                self.workspace_label(),
                                app_icon(self.workspace_harness_icon(), AppIconSize::Inline)
                                    .into_any_element(),
                                chat_hint.into(),
                                surface == AppSurface::Chat,
                                entity.clone(),
                                |app, window, cx| app.show_chat_surface(window, cx),
                                None,
                            ))
                            .child(workspace_tab(
                                "workspace-tab-editor",
                                self.text_editor_name().into(),
                                app_icon(self.text_editor_icon(), AppIconSize::Inline)
                                    .into_any_element(),
                                format!(
                                    "{} ({modifier}+E in app views; {} anywhere)",
                                    self.text_editor_name(),
                                    crate::app::ui::navigation::command_key(
                                        crate::app::ui::navigation::Command::Editor
                                    )
                                )
                                .into(),
                                surface == AppSurface::Editor,
                                entity.clone(),
                                |app, window, cx| app.show_editor_surface(window, cx),
                                None,
                            ))
                            .child(workspace_tab(
                                "workspace-tab-terminal",
                                "Terminal".into(),
                                app_icon(AppIcon::Ghostty, AppIconSize::Inline).into_any_element(),
                                format!(
                                    "Terminal ({modifier}+T in app views; {} anywhere)",
                                    crate::app::ui::navigation::command_key(
                                        crate::app::ui::navigation::Command::Terminal
                                    )
                                )
                                .into(),
                                surface == AppSurface::Terminal,
                                entity.clone(),
                                |app, window, cx| app.show_terminal_surface(window, cx),
                                None,
                            ))
                            .children(self.open_diffs().iter().enumerate().map(|(index, diff)| {
                                let active =
                                    surface == AppSurface::Diff && active_diff == Some(&diff.key);
                                diff_tab(index, diff, active, entity.clone())
                            }))
                            .child(div().flex_1().h_full().on_mouse_down(
                                MouseButton::Left,
                                |_, window, _| {
                                    window.start_window_move();
                                },
                            )),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(theme().size(24.0))
                            .h_full()
                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                window.start_window_move();
                            }),
                    )
                    .child(self.render_workspace_panels(mode, entity.clone())),
            )
    }
}

#[allow(clippy::too_many_arguments)]
fn workspace_tab(
    id: impl Into<ElementId>,
    label: SharedString,
    icon: AnyElement,
    hint: SharedString,
    active: bool,
    entity: WeakEntity<FarcasterApp>,
    action: impl Fn(&mut FarcasterApp, &mut Window, &mut Context<FarcasterApp>) + 'static,
    trailing: Option<AnyElement>,
) -> AnyElement {
    div()
        .id(id)
        .relative()
        .flex_none()
        .h_full()
        .max_w(theme().size(220.0))
        .flex()
        .items_center()
        .gap(theme().space.xs)
        .px(theme().space.sm)
        .whitespace_nowrap()
        .text_size(theme().type_scale.caption)
        .text_color(if active {
            theme().colors.text
        } else {
            theme().colors.muted
        })
        .when(active, |tab| tab.bg(theme().colors.surface))
        .hover(|tab| tab.bg(theme().colors.highlight))
        .cursor_pointer()
        .when(active, |tab| {
            tab.child(line_indicator(IndicatorEdge::Top, theme().colors.indicator))
        })
        .app_tooltip(hint)
        .child(icon)
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .child(label),
        )
        .children(trailing)
        .on_click(move |_, window, cx| {
            let _ = entity.update(cx, |app, cx| action(app, window, cx));
        })
        .into_any_element()
}

fn diff_tab(
    index: usize,
    diff: &RepositoryDiff,
    active: bool,
    entity: WeakEntity<FarcasterApp>,
) -> AnyElement {
    let path = diff.path.clone();
    let label: SharedString = path
        .file_name()
        .map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
        .into();
    let hint: SharedString = path.display().to_string().into();
    let open_key = diff.key.clone();
    let close = entity.clone();
    let close_key = diff.key.clone();
    let close_hint: SharedString = format!("Close {hint}").into();
    workspace_tab(
        ("workspace-tab-diff", index),
        label,
        file_icon(&path),
        hint,
        active,
        entity,
        move |app, window, cx| app.activate_repository_diff(open_key.clone(), window, cx),
        Some(
            icon_control(("close-workspace-tab", index), close_hint)
                .size(theme().size(16.0))
                .child(app_icon(AppIcon::X, AppIconSize::Inline))
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    let _ = close.update(cx, |this, cx| {
                        this.close_repository_diff(&close_key, window, cx);
                    });
                })
                .into_any_element(),
        ),
    )
}
