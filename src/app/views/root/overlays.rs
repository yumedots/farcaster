use gpui::{
    AnyElement, Context, FocusHandle, IntoElement as _, ParentElement as _, Styled as _,
    WeakEntity, prelude::FluentBuilder as _,
};

use super::{
    super::{FarcasterApp, OVERLAY_KEY_CONTEXT, dialogs},
    keybindings,
};
use crate::app::ui::{primitives::modal, theme::theme};

impl FarcasterApp {
    pub(super) fn render_root_overlays(
        &self,
        root: gpui::Div,
        entity: WeakEntity<Self>,
        picker: Option<AnyElement>,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let sessions_sheet = self.overlays.view.sessions.then(|| {
            panel_sheet(
                "sessions",
                "Sessions",
                &self.overlays.sheet_focus,
                entity.clone(),
                self.views
                    .session_rail
                    .clone()
                    .cached(gpui::StyleRefinement::default().size_full())
                    .into_any_element(),
            )
        });
        let run_sheet = self.overlays.view.run.then(|| {
            panel_sheet(
                "run",
                if self.visible_review().is_some() {
                    "Review"
                } else {
                    "Session details"
                },
                &self.overlays.sheet_focus,
                entity.clone(),
                self.views
                    .run_panel
                    .clone()
                    .cached(gpui::StyleRefinement::default().size_full())
                    .into_any_element(),
            )
        });

        root.when_some(picker, |root, picker| root.child(picker))
            .when(self.overlays.view.project_trust, |root| {
                root.child(dialogs::project_trust::render(self, entity.clone()))
            })
            .when(self.overlays.view.settings, |root| {
                root.child(dialogs::settings::render(self, entity.clone(), cx))
            })
            .when(self.overlays.view.keybindings, |root| {
                let close = entity.clone();
                root.child(modal(
                    "keybindings-help",
                    "Keyboard shortcuts",
                    &self.overlays.sheet_focus,
                    OVERLAY_KEY_CONTEXT,
                    move |window, cx| {
                        let _ = close.update(cx, |this, cx| this.close_sheet(window, cx));
                    },
                    |surface| {
                        surface
                            .w(theme().size(520.0))
                            .max_w_full()
                            .child(keybindings::render_help())
                    },
                ))
            })
            .when_some(sessions_sheet, |root, sheet| root.child(sheet))
            .when_some(run_sheet, |root, sheet| root.child(sheet))
            .when(self.sessions.pending_archive.is_some(), |root| {
                root.child(dialogs::archive_confirmation::render(self, entity.clone()))
            })
            .when(
                self.workspace.send_to_chat.is_some() && !self.overlays.view.project_trust,
                |root| root.child(dialogs::send_to_chat::render(self, entity.clone(), cx)),
            )
            .when(self.sessions.pending_delete.is_some(), |root| {
                root.child(dialogs::delete_confirmation::render(self, entity.clone()))
            })
            .when(self.sessions.import.is_some(), |root| {
                root.child(dialogs::session_import::render(self, entity.clone()))
            })
            .when(self.project.repository.pending_jj_init.is_some(), |root| {
                root.child(dialogs::jj_init_confirmation::render(self, entity.clone()))
            })
            .when(self.project.repository.edits.pending.is_some(), |root| {
                root.child(dialogs::repository_edit::render(self, entity.clone(), cx))
            })
            .when_some(
                dialogs::image_preview::render(self, entity.clone()),
                |root, preview| root.child(preview),
            )
            .when(self.lifecycle.pending_quit.is_some(), |root| {
                root.child(dialogs::quit_confirmation::render(self, entity.clone()))
            })
    }
}

fn panel_sheet(
    id: &'static str,
    title: &'static str,
    focus: &FocusHandle,
    entity: WeakEntity<FarcasterApp>,
    content: AnyElement,
) -> AnyElement {
    modal(
        id,
        title,
        focus,
        OVERLAY_KEY_CONTEXT,
        move |window, cx| {
            let _ = entity.update(cx, |this, cx| this.close_sheet(window, cx));
        },
        |surface| surface.h_full().max_w_full().child(content),
    )
    .into_any_element()
}
