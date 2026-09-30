pub(crate) use crate::app::ui::change_tree;
mod performance;
mod repository;
pub(in crate::app) mod repository_controls;
mod repository_presentation;
mod resize;
pub(super) mod review;
#[cfg(test)]
mod tests;

use gpui::{
    IntoElement, ParentElement as _, ScrollHandle, Styled as _, WeakEntity, div,
    prelude::FluentBuilder as _,
};

pub(super) use resize::clamped_run_panel_width;

use self::performance::render_performance;
use super::super::{FarcasterApp, RunPanelView};
use crate::{
    app::ui::primitives::{ButtonTone, button, panel},
    app::ui::theme::theme,
    sessions::root_session_for_path,
};

pub(crate) struct RepositoryView<'a> {
    pub(crate) state: &'a change_tree::ChangeTreeState,
    pub(crate) search: &'a gpui::Entity<gpui_component::input::InputState>,
    pub(crate) query: &'a str,
    pub(crate) scroll: &'a ScrollHandle,
}

impl FarcasterApp {
    pub(super) fn render_run_panel(
        &self,
        entity: WeakEntity<Self>,
        run_panel: WeakEntity<RunPanelView>,
        browser: &RepositoryView<'_>,
    ) -> impl IntoElement {
        let root = root_session_for_path(
            &self.sessions.all,
            self.snapshot.selected_session.as_deref(),
        );
        let body = div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .pt(theme().size(17.0))
            .pr(theme().size(15.0))
            .pb(theme().size(14.0))
            .pl(theme().size(18.0))
            .gap(theme().space.md)
            .when_some(
                self.lifecycle
                    .performance_monitor
                    .as_ref()
                    .filter(|monitor| monitor.is_detailed()),
                |run, monitor| run.child(render_performance(&monitor.summary)),
            )
            .when_some(root, |run, root| {
                let selected =
                    self.snapshot.selected_session.as_deref() == Some(root.path.as_path());
                let path = root.path.clone();
                let project = root.project.clone();
                let entity = entity.clone();
                run.child(
                    button(
                        "run-panel-main-agent",
                        "Main agent",
                        if selected {
                            ButtonTone::Neutral
                        } else {
                            ButtonTone::Quiet
                        },
                        true,
                        move |window, cx| {
                            let _ = entity.update(cx, |this, cx| {
                                this.select_session_and_focus(
                                    path.clone(),
                                    project.clone(),
                                    window,
                                    cx,
                                );
                            });
                        },
                    )
                    .w_full()
                    .flex_none(),
                )
            })
            .when(self.project.repository.backend.is_some(), |run| {
                run.child(self.render_repository(entity.clone(), run_panel.clone(), browser))
            });
        panel()
            .size_full()
            .rounded_none()
            .border_0()
            .bg(theme().colors.inspector)
            .child(body)
    }
}
