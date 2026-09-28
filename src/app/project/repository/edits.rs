use std::{collections::BTreeSet, path::PathBuf};

use gpui::{AppContext as _, Context, Entity, FocusHandle, Focusable as _, Subscription, Window};
use gpui_component::input::TextareaState;

use crate::{
    app::{FarcasterApp, ui::primitives::create_submit_textarea},
    repository::{RepositoryEdit, RepositoryEditReview, WorkingCopySnapshot},
};

#[derive(Default)]
pub(in crate::app) struct FileSelection {
    pub paths: BTreeSet<PathBuf>,
    anchor: Option<PathBuf>,
}

impl FileSelection {
    pub fn toggle(&mut self, path: PathBuf) {
        if !self.paths.remove(&path) {
            self.paths.insert(path);
        }
    }

    /// A modifier click: the row toggles itself and becomes the new anchor.
    pub fn toggle_from(&mut self, path: PathBuf) {
        self.anchor = Some(path.clone());
        self.toggle(path);
    }

    /// A shift click: every visible row between the anchor and the row is added.
    pub fn extend_to(&mut self, path: PathBuf, visible: &[PathBuf]) {
        let anchor = self
            .anchor
            .as_ref()
            .and_then(|anchor| visible.iter().position(|row| row == anchor));
        let (Some(anchor), Some(end)) = (anchor, visible.iter().position(|row| row == &path))
        else {
            self.toggle_from(path);
            return;
        };
        let (start, end) = if anchor <= end {
            (anchor, end)
        } else {
            (end, anchor)
        };
        self.paths.extend(visible[start..=end].iter().cloned());
    }

    pub fn retain(&mut self, snapshot: &WorkingCopySnapshot) {
        self.paths.retain(|path| {
            snapshot
                .changes
                .iter()
                .any(|change| &change.relative_path == path)
        });
        if self
            .anchor
            .as_ref()
            .is_some_and(|anchor| !self.paths.contains(anchor))
        {
            self.anchor = None;
        }
    }
}

#[derive(Default)]
pub(in crate::app) struct RepositoryEditState {
    pub selection: FileSelection,
    pub pending: Option<PendingRepositoryEdit>,
    pub staging: bool,
    generation: u64,
}

impl RepositoryEditState {
    pub(super) fn clear(&mut self) {
        self.selection = Default::default();
        self.pending = None;
        self.generation = self.generation.saturating_add(1);
    }
}

pub(in crate::app) struct PendingRepositoryEdit {
    pub focus: FocusHandle,
    pub input: Entity<TextareaState>,
    pub action: RepositoryEdit,
    pub paths: Vec<PathBuf>,
    pub review: Option<RepositoryEditReview>,
    pub error: Option<String>,
    pub applying: bool,
    return_focus: Option<FocusHandle>,
    _subscription: Subscription,
}

impl PendingRepositoryEdit {
    pub fn preparing(&self) -> bool {
        self.review.is_none() && self.error.is_none()
    }

    pub fn can_apply(&self, cx: &gpui::App) -> bool {
        self.review.is_some()
            && self.error.is_none()
            && !self.applying
            && (!self.action.requires_message() || !self.input.read(cx).value().trim().is_empty())
    }
}

impl FarcasterApp {
    pub(in crate::app) fn clear_repository_selection(&mut self, cx: &mut Context<Self>) {
        if self.project.repository.edits.pending.is_none() {
            self.project.repository.edits.selection.paths.clear();
            self.notify_run_panel(cx);
        }
    }

    pub(in crate::app) fn toggle_repository_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.project.repository.edits.pending.is_none() {
            self.project.repository.edits.selection.toggle(path);
            self.notify_run_panel(cx);
        }
    }

    pub(in crate::app) fn toggle_repository_selection(
        &mut self,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) {
        if self.repository_edit_available() {
            self.project.repository.edits.selection.toggle_from(path);
            self.notify_run_panel(cx);
        }
    }

    pub(in crate::app) fn extend_repository_selection(
        &mut self,
        path: PathBuf,
        visible: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        if self.repository_edit_available() {
            self.project
                .repository
                .edits
                .selection
                .extend_to(path, &visible);
            self.notify_run_panel(cx);
        }
    }

    fn repository_edit_available(&self) -> bool {
        self.project.repository.edits.pending.is_none()
            && !self.project.repository.edits.staging
            && self.project.repository.execution_allowed
    }

    /// Files a row's context menu acts on: the whole selection when the row is
    /// part of it, otherwise just that row.
    pub(in crate::app) fn repository_menu_paths(&self, path: &PathBuf) -> Vec<PathBuf> {
        let selection = &self.project.repository.edits.selection.paths;
        if selection.len() > 1 && selection.contains(path) {
            selection.iter().cloned().collect()
        } else {
            vec![path.clone()]
        }
    }

    pub(in crate::app) fn stage_repository_paths(
        &mut self,
        action: RepositoryEdit,
        paths: BTreeSet<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        if !matches!(action, RepositoryEdit::Stage | RepositoryEdit::Unstage)
            || paths.is_empty()
            || !self.project.repository.execution_allowed
            || self.project.repository.sync.action.is_some()
            || self.project.repository.edits.pending.is_some()
            || self.project.repository.edits.staging
        {
            return;
        }
        let Some(backend) = self.project.repository.backend.clone() else {
            return;
        };
        self.project.repository.edits.staging = true;
        self.notify_run_panel(cx);
        let task = cx.background_spawn(async move {
            let snapshot = backend.snapshot()?;
            let review = backend.prepare_edit(&snapshot, &paths)?;
            backend.apply_edit(&review, action, "")
        });
        cx.spawn(async move |weak, cx| {
            let result = task.await;
            let _ = weak.update(cx, |this, cx| {
                this.project.repository.edits.staging = false;
                if let Err(error) = result {
                    this.project.repository.error = Some(error.to_string());
                }
                this.request_repository_refresh(cx);
                this.notify_run_panel(cx);
            });
        })
        .detach();
    }

    pub(in crate::app) fn review_repository_edit(
        &mut self,
        action: RepositoryEdit,
        path: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selected = path
            .map(|path| BTreeSet::from([path]))
            .unwrap_or_else(|| self.project.repository.edits.selection.paths.clone());
        self.review_repository_paths(action, selected, window, cx);
    }

    pub(in crate::app) fn review_repository_paths(
        &mut self,
        action: RepositoryEdit,
        selected: BTreeSet<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.project.repository.execution_allowed
            || self.project.repository.sync.action.is_some()
            || self.project.repository.edits.pending.is_some()
        {
            return;
        }
        let (Some(backend), Some(snapshot)) = (
            self.project.repository.backend.clone(),
            self.project.repository.snapshot.clone(),
        ) else {
            return;
        };
        if selected.is_empty() {
            return;
        }
        let (input, subscription) = create_submit_textarea(
            window,
            cx,
            |input| input.auto_grow(2, 6).placeholder("Commit message"),
            move |this, window, cx| {
                if action.requires_message() {
                    this.confirm_repository_edit(window, cx);
                }
            },
        );
        let focus = cx.focus_handle();
        let return_focus = window.focused(cx);
        self.cover_native_workspace_surface(cx);
        if action.requires_message() {
            input.read(cx).focus_handle(cx).focus(window, cx);
        } else {
            focus.focus(window, cx);
        }
        self.project.repository.edits.generation =
            self.project.repository.edits.generation.saturating_add(1);
        let generation = self.project.repository.edits.generation;
        self.project.repository.edits.pending = Some(PendingRepositoryEdit {
            focus,
            input,
            action,
            paths: selected.iter().cloned().collect(),
            review: None,
            applying: false,
            error: None,
            return_focus,
            _subscription: subscription,
        });
        self.notify_run_panel(cx);
        cx.notify();
        let task = cx.background_spawn(async move { backend.prepare_edit(&snapshot, &selected) });
        cx.spawn(async move |weak, cx| {
            let result = task.await;
            let _ = weak.update(cx, |this, cx| {
                if this.project.repository.edits.generation != generation {
                    return;
                }
                let Some(pending) = this.project.repository.edits.pending.as_mut() else {
                    return;
                };
                match result {
                    Ok(review) => {
                        pending.paths = review.paths().to_vec();
                        pending.review = Some(review);
                    }
                    Err(error) => {
                        pending.error = Some(format!(
                            "{error}\nClose this review and try again after the changes refresh."
                        ));
                        this.request_repository_refresh(cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(in crate::app) fn close_repository_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .project
            .repository
            .edits
            .pending
            .as_ref()
            .is_some_and(|pending| pending.applying)
        {
            return;
        }
        let Some(pending) = self.project.repository.edits.pending.take() else {
            return;
        };
        self.project.repository.edits.generation =
            self.project.repository.edits.generation.saturating_add(1);
        self.restore_overlay_focus(pending.return_focus, &pending.focus, window, cx);
        self.restore_active_native_workspace_surface(window, cx);
        self.notify_run_panel(cx);
        cx.notify();
    }

    pub(in crate::app) fn confirm_repository_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.project.repository.execution_allowed
            || self.project.repository.sync.action.is_some()
        {
            return;
        }
        let Some(backend) = self.project.repository.backend.clone() else {
            return;
        };
        let Some(pending) = self.project.repository.edits.pending.as_mut() else {
            return;
        };
        if !pending.can_apply(cx) {
            return;
        }
        let Some(review) = pending.review.clone() else {
            return;
        };
        let action = pending.action;
        let message = pending.input.read(cx).value().to_string();
        pending.applying = true;
        let generation = self.project.repository.edits.generation;
        cx.notify();
        let task =
            cx.background_spawn(async move { backend.apply_edit(&review, action, &message) });
        cx.spawn_in(window, async move |weak, cx| {
            let result = task.await;
            let _ = weak.update_in(cx, |this, window, cx| {
                if this.project.repository.edits.generation != generation { return; }
                let Some(pending) = this.project.repository.edits.pending.as_mut() else { return; };
                pending.applying = false;
                match result {
                    Ok(()) => {
                        this.project.repository.edits.selection = Default::default();
                        this.close_repository_edit(window, cx);
                    }
                    Err(error) => { pending.error = Some(format!("{error}\nClose this review and inspect the refreshed changes before trying again.")); }
                }
                // A failed hook or command can still have changed repository state.
                this.request_repository_refresh(cx);
                this.notify_run_panel(cx);
                cx.notify();
            });
        }).detach();
    }
}

#[cfg(test)]
#[path = "edits_tests.rs"]
mod tests;
