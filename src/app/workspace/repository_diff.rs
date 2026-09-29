use std::path::PathBuf;

use gpui::{AppContext as _, Context, Window};

use super::FarcasterApp;
use crate::{
    app::{AppSurface, RepositoryDiff},
    repository::{ChangeLayer, DiffTarget, DiffTargetKey, HunkApply, RepositoryError},
};

fn target_for(
    key: &DiffTargetKey,
    layer: ChangeLayer,
    snapshot: &crate::repository::WorkingCopySnapshot,
) -> Result<DiffTarget, RepositoryError> {
    snapshot
        .changes
        .iter()
        .find(|change| &change.target.key == key)
        .or_else(|| {
            snapshot
                .changes
                .iter()
                .find(|change| change.relative_path == key.relative_path && change.layer == layer)
        })
        .or_else(|| {
            snapshot
                .changes
                .iter()
                .find(|change| change.relative_path == key.relative_path)
        })
        .map(|change| change.target.clone())
        .ok_or(RepositoryError::StaleSnapshot)
}

impl FarcasterApp {
    fn diff_index(&self, key: &DiffTargetKey) -> Option<usize> {
        self.workspace
            .diffs
            .iter()
            .position(|diff| &diff.key == key)
    }

    pub(in crate::app) fn active_diff(&self) -> Option<&RepositoryDiff> {
        let key = self.workspace.active_diff.as_ref()?;
        self.workspace.diffs.iter().find(|diff| &diff.key == key)
    }

    pub(in crate::app) fn open_diffs(&self) -> &[RepositoryDiff] {
        &self.workspace.diffs
    }

    fn active_diff_mut(&mut self) -> Option<&mut RepositoryDiff> {
        let key = self.workspace.active_diff.clone()?;
        self.workspace.diffs.iter_mut().find(|diff| diff.key == key)
    }

    pub(in crate::app) fn open_repository_diff(
        &mut self,
        key: DiffTargetKey,
        path: PathBuf,
        layer: ChangeLayer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(backend) = self.project.repository.backend.clone() else {
            return;
        };
        match self.diff_index(&key) {
            Some(index) => {
                self.workspace.diffs[index].layer = layer;
            }
            None => {
                self.workspace
                    .diffs
                    .push(RepositoryDiff::new(key.clone(), path, layer));
            }
        }
        let Some(diff) = self.workspace.diffs.iter_mut().find(|diff| diff.key == key) else {
            return;
        };
        diff.error = None;
        let generation = diff.next_generation();
        self.workspace.active_diff = Some(key.clone());
        self.show_diff_center(window, cx);
        self.load_repository_diff(backend, key, layer, generation, cx);
    }

    pub(in crate::app) fn activate_repository_diff(
        &mut self,
        key: DiffTargetKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.diff_index(&key).is_none() {
            return;
        }
        self.workspace.active_diff = Some(key);
        self.show_diff_center(window, cx);
    }

    fn show_diff_center(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hide_native_workspace_surfaces(cx);
        if self.workspace.surface != AppSurface::Diff {
            self.workspace.diff_return = Some(self.workspace.surface);
        }
        self.set_surface(AppSurface::Diff, cx);
        self.overlays.repository_diff_focus.focus(window, cx);
        self.notify_run_panel(cx);
        cx.notify();
    }

    fn load_repository_diff(
        &self,
        backend: crate::repository::RepositoryBackend,
        key: DiffTargetKey,
        layer: ChangeLayer,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        let lookup = key.clone();
        let task = cx.background_spawn(async move {
            let snapshot = backend.snapshot()?;
            let target = target_for(&lookup, layer, &snapshot)?;
            backend.file_diff(&target, true)
        });
        cx.spawn(async move |weak, cx| {
            let result = task.await;
            let _ = weak.update(cx, |this, cx| {
                let Some(diff) = this.workspace.diffs.iter_mut().find(|diff| diff.key == key)
                else {
                    return;
                };
                if diff.generation() != generation {
                    return;
                }
                let hidden = this.settings.hide_unchanged_lines;
                match result {
                    Ok(file_diff) => diff.set_diff(file_diff, hidden, cx.text_system()),
                    Err(error) => diff.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(in crate::app) fn toggle_repository_diff_span(
        &mut self,
        span: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(diff) = self.active_diff_mut() else {
            return;
        };
        diff.toggle_span(span);
        cx.notify();
    }

    pub(in crate::app) fn toggle_repository_diff_split(&mut self, cx: &mut Context<Self>) {
        let Some(diff) = self.active_diff_mut() else {
            return;
        };
        diff.set_split(!diff.split);
        cx.notify();
    }

    pub(in crate::app) fn reload_repository_diff(&mut self, cx: &mut Context<Self>) {
        let Some(backend) = self.project.repository.backend.clone() else {
            return;
        };
        let Some(key) = self.workspace.active_diff.clone() else {
            return;
        };
        let Some(diff) = self.workspace.diffs.iter_mut().find(|diff| diff.key == key) else {
            return;
        };
        let layer = diff.layer;
        diff.error = None;
        let generation = diff.next_generation();
        self.load_repository_diff(backend, key, layer, generation, cx);
    }

    pub(in crate::app) fn apply_repository_hunk(
        &mut self,
        index: usize,
        mode: HunkApply,
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
        let Some(diff) = self.active_diff() else {
            return;
        };
        if diff.applying.is_some() || diff.diff.is_none() {
            return;
        }
        let key = diff.key.clone();
        let layer = diff.layer;
        let Some(diff) = self.workspace.diffs.iter_mut().find(|diff| diff.key == key) else {
            return;
        };
        diff.applying = Some(index);
        self.notify_run_panel(cx);
        cx.notify();
        let lookup = key.clone();
        let task = cx.background_spawn(async move {
            let snapshot = backend.snapshot()?;
            let target = target_for(&lookup, layer, &snapshot)?;
            let current = backend.file_diff(&target, false)?;
            let patch = current.patch_for(index).ok_or_else(|| {
                RepositoryError::InvalidRepository("That hunk is no longer there".into())
            })?;
            backend.apply_hunk_patch(&patch, mode)?;
            let snapshot = backend.snapshot()?;
            let target = target_for(&lookup, layer, &snapshot)?;
            let moved = target.key.layer != layer;
            Ok::<_, RepositoryError>((
                backend.file_diff(&target, true)?,
                moved.then_some(target.key.layer),
            ))
        });
        cx.spawn(async move |weak, cx| {
            let result = task.await;
            let _ = weak.update(cx, |this, cx| {
                let hidden = this.settings.hide_unchanged_lines;
                if let Some(diff) = this.workspace.diffs.iter_mut().find(|diff| diff.key == key) {
                    diff.applying = None;
                    match result {
                        Ok((file_diff, layer)) => {
                            diff.error = None;
                            if let Some(layer) = layer {
                                diff.layer = layer;
                            }
                            diff.set_diff(file_diff, hidden, cx.text_system());
                        }
                        Err(error) => diff.error = Some(error.to_string()),
                    }
                }
                this.request_repository_refresh(cx);
                this.notify_run_panel(cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub(in crate::app) fn close_active_diff(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(key) = self.workspace.active_diff.clone() else {
            return;
        };
        self.close_repository_diff(&key, window, cx);
    }

    pub(in crate::app) fn close_repository_diff(
        &mut self,
        key: &DiffTargetKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.diff_index(key) else {
            return;
        };
        self.workspace.diffs.remove(index);
        if self.workspace.active_diff.as_ref() != Some(key) {
            cx.notify();
            return;
        }
        let next = self.workspace.diffs.last().map(|diff| diff.key.clone());
        let showing = self.workspace.surface == AppSurface::Diff;
        match next {
            Some(next) => {
                self.workspace.active_diff = Some(next);
                if showing {
                    self.overlays.repository_diff_focus.focus(window, cx);
                }
                self.notify_run_panel(cx);
                cx.notify();
            }
            None => {
                self.workspace.active_diff = None;
                if !showing {
                    cx.notify();
                    return;
                }
                match self
                    .workspace
                    .diff_return
                    .take()
                    .unwrap_or(AppSurface::Chat)
                {
                    AppSurface::Editor => self.show_editor_surface(window, cx),
                    AppSurface::Terminal => self.show_terminal_surface(window, cx),
                    AppSurface::Chat | AppSurface::Diff => self.show_chat_surface(window, cx),
                }
            }
        }
    }
}
