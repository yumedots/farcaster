use std::path::PathBuf;

use gpui::{AppContext as _, Context, Window};

use super::FarcasterApp;
use crate::{
    app::RepositoryDiff,
    repository::{ChangeLayer, DiffTarget, DiffTargetKey, HunkApply, RepositoryError},
};

/// The change a diff overlay belongs to, looked up in the snapshot the caller
/// is about to read, so a stale overlay reports a stale working copy instead of
/// applying against the wrong version of the file.
fn target_for(
    key: &DiffTargetKey,
    layer: ChangeLayer,
    snapshot: &crate::repository::WorkingCopySnapshot,
) -> Result<DiffTarget, RepositoryError> {
    snapshot
        .changes
        .iter()
        .find(|change| &change.target.key == key)
        // Staging a file's last hunk moves it out of the layer it was opened
        // from, so the overlay follows the file to its remaining section.
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
        let return_focus = window.focused(cx);
        let mut diff = RepositoryDiff::new(key.clone(), path.clone(), layer);
        let generation = diff.next_generation();
        if self.overlays.repository_diff.is_none() {
            self.overlays.repository_diff_return_focus = return_focus;
        }
        self.overlays.repository_diff = Some(diff);
        self.overlays.repository_diff_focus.focus(window, cx);
        self.notify_run_panel(cx);
        cx.notify();
        self.load_repository_diff(backend, key, layer, generation, cx);
    }

    fn load_repository_diff(
        &self,
        backend: crate::repository::RepositoryBackend,
        key: DiffTargetKey,
        layer: ChangeLayer,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        let task = cx.background_spawn(async move {
            let snapshot = backend.snapshot()?;
            let target = target_for(&key, layer, &snapshot)?;
            backend.file_diff(&target)
        });
        cx.spawn(async move |weak, cx| {
            let result = task.await;
            let _ = weak.update(cx, |this, cx| {
                let Some(diff) = this.overlays.repository_diff.as_mut() else {
                    return;
                };
                if diff.generation() != generation {
                    return;
                }
                match result {
                    Ok(file_diff) => {
                        diff.additions = file_diff
                            .hunks
                            .iter()
                            .map(|hunk| hunk.additions as u64)
                            .sum();
                        diff.deletions = file_diff
                            .hunks
                            .iter()
                            .map(|hunk| hunk.deletions as u64)
                            .sum();
                        diff.diff = Some(file_diff);
                    }
                    Err(error) => diff.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Re-reads the file's diff without closing the overlay.
    pub(in crate::app) fn reload_repository_diff(&mut self, cx: &mut Context<Self>) {
        let Some(backend) = self.project.repository.backend.clone() else {
            return;
        };
        let Some(diff) = self.overlays.repository_diff.as_mut() else {
            return;
        };
        let key = diff.key.clone();
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
            || self
                .overlays
                .repository_diff
                .as_ref()
                .is_none_or(|diff| diff.applying.is_some() || diff.diff.is_none())
        {
            return;
        }
        let Some(backend) = self.project.repository.backend.clone() else {
            return;
        };
        let Some(diff) = self.overlays.repository_diff.as_mut() else {
            return;
        };
        let key = diff.key.clone();
        let layer = diff.layer;
        diff.applying = Some(index);
        self.notify_run_panel(cx);
        cx.notify();
        let task = cx.background_spawn(async move {
            let snapshot = backend.snapshot()?;
            let target = target_for(&key, layer, &snapshot)?;
            let current = backend.file_diff(&target)?;
            let patch = current.patch_for(index).ok_or_else(|| {
                RepositoryError::InvalidRepository("That hunk is no longer there".into())
            })?;
            backend.apply_hunk_patch(&patch, mode)?;
            // The index moved, so the next read needs a fresh target.
            let snapshot = backend.snapshot()?;
            let target = target_for(&key, layer, &snapshot)?;
            let moved = target.key.layer != layer;
            Ok::<_, RepositoryError>((
                backend.file_diff(&target)?,
                moved.then_some(target.key.layer),
            ))
        });
        cx.spawn(async move |weak, cx| {
            let result = task.await;
            let _ = weak.update(cx, |this, cx| {
                if let Some(diff) = this.overlays.repository_diff.as_mut() {
                    diff.applying = None;
                    match result {
                        Ok((file_diff, layer)) => {
                            diff.error = None;
                            if let Some(layer) = layer {
                                diff.layer = layer;
                            }
                            diff.additions = file_diff
                                .hunks
                                .iter()
                                .map(|hunk| hunk.additions as u64)
                                .sum();
                            diff.deletions = file_diff
                                .hunks
                                .iter()
                                .map(|hunk| hunk.deletions as u64)
                                .sum();
                            diff.diff = Some(file_diff);
                        }
                        Err(error) => diff.error = Some(error.to_string()),
                    }
                }
                // A hunk can move the index, the working tree, or both.
                this.request_repository_refresh(cx);
                this.notify_run_panel(cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub(in crate::app) fn close_repository_diff(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.overlays.repository_diff.take().is_none() {
            return;
        }
        let target = self.overlays.repository_diff_return_focus.take();
        let focus = self.overlays.repository_diff_focus.clone();
        self.restore_overlay_focus(target, &focus, window, cx);
        self.notify_run_panel(cx);
        cx.notify();
    }
}
