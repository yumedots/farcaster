use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use gpui::{Bounds, Context, Entity, EntityId, Pixels, Window};
use gpui_libghostty::Terminal;

use super::{AppSurface, FarcasterApp, LoginBanner, spawn_workspace_terminal};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum TerminalSplitDirection {
    Right,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum TerminalDropSide {
    Left,
    Right,
    Up,
    Down,
}

pub(in crate::app) const HANDLE_BAND: f32 = 16.0;
pub(in crate::app) const HANDLE_PILL_WIDTH: f32 = 32.0;
pub(in crate::app) const HANDLE_PILL_HEIGHT: f32 = 10.0;

impl TerminalDropSide {
    pub(in crate::app) fn label(self) -> &'static str {
        match self {
            TerminalDropSide::Left => "left",
            TerminalDropSide::Right => "right",
            TerminalDropSide::Up => "up",
            TerminalDropSide::Down => "down",
        }
    }

    pub(in crate::app) fn for_point(x: f32, y: f32, width: f32, height: f32) -> Self {
        if width <= 0.0 || height <= 0.0 {
            return TerminalDropSide::Right;
        }
        let left = x / width;
        let right = 1.0 - left;
        let top = y / height;
        let bottom = 1.0 - top;
        let nearest = left.min(right).min(top).min(bottom);
        if nearest == left {
            TerminalDropSide::Left
        } else if nearest == right {
            TerminalDropSide::Right
        } else if nearest == top {
            TerminalDropSide::Up
        } else {
            TerminalDropSide::Down
        }
    }

    pub(in crate::app) fn drop_bounds(self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);
        let mut drop = bounds;
        match self {
            TerminalDropSide::Left => drop.size.width = Pixels::from(width / 2.0),
            TerminalDropSide::Right => {
                drop.origin.x = bounds.origin.x + Pixels::from(width / 2.0);
                drop.size.width = Pixels::from(width / 2.0);
            }
            TerminalDropSide::Up => drop.size.height = Pixels::from(height / 2.0),
            TerminalDropSide::Down => {
                drop.origin.y = bounds.origin.y + Pixels::from(height / 2.0);
                drop.size.height = Pixels::from(height / 2.0);
            }
        }
        drop
    }
}

pub(in crate::app) fn handle_pill_bounds(bounds: Bounds<Pixels>) -> Bounds<Pixels> {
    let centered = (f32::from(bounds.size.width) - HANDLE_PILL_WIDTH) / 2.0;
    let mut pill = bounds;
    pill.origin.x = bounds.origin.x + Pixels::from(centered);
    pill.origin.y = bounds.origin.y + Pixels::from((HANDLE_BAND - HANDLE_PILL_HEIGHT) / 2.0);
    pill.size.width = Pixels::from(HANDLE_PILL_WIDTH);
    pill.size.height = Pixels::from(HANDLE_PILL_HEIGHT);
    pill
}

pub(in crate::app) fn terminal_snapshots_cover_leaves<T>(
    leaves: &[EntityId],
    captured: &HashMap<EntityId, T>,
) -> bool {
    leaves.len() == captured.len() && leaves.iter().all(|id| captured.contains_key(id))
}

#[derive(Debug)]
pub(in crate::app) enum TerminalPane {
    Leaf(EntityId),
    Split {
        direction: TerminalSplitDirection,
        ratio: f32,
        first: Box<TerminalPane>,
        second: Box<TerminalPane>,
    },
}

pub(in crate::app) struct TerminalLayout {
    root: TerminalPane,
    focused: EntityId,
    panes: HashMap<EntityId, Entity<Terminal>>,
}

impl TerminalLayout {
    fn new(primary: Entity<Terminal>) -> Self {
        let id = primary.entity_id();
        let mut panes = HashMap::new();
        panes.insert(id, primary);
        Self {
            root: TerminalPane::Leaf(id),
            focused: id,
            panes,
        }
    }

    pub(in crate::app) fn root(&self) -> &TerminalPane {
        &self.root
    }

    pub(in crate::app) fn focused_id(&self) -> EntityId {
        self.focused
    }

    pub(in crate::app) fn set_focused(&mut self, id: EntityId) {
        if self.contains(id) {
            self.focused = id;
        }
    }

    pub(in crate::app) fn contains(&self, id: EntityId) -> bool {
        Self::contains_pane(&self.root, id)
    }

    pub(in crate::app) fn leaf_count(&self) -> usize {
        Self::leaf_ids_of(&self.root).len()
    }

    pub(in crate::app) fn single_leaf_id(&self) -> Option<EntityId> {
        (self.leaf_count() == 1).then(|| Self::first_leaf(&self.root))
    }

    pub(in crate::app) fn terminal(&self, id: EntityId) -> Option<&Entity<Terminal>> {
        self.panes.get(&id)
    }

    pub(in crate::app) fn focused_terminal(&self) -> Option<&Entity<Terminal>> {
        self.panes.get(&self.focused)
    }

    pub(in crate::app) fn terminals(&self) -> Vec<Entity<Terminal>> {
        Self::leaf_ids_of(&self.root)
            .into_iter()
            .filter_map(|id| self.panes.get(&id).cloned())
            .collect()
    }

    pub(in crate::app) fn leaf_ids(&self) -> Vec<EntityId> {
        Self::leaf_ids_of(&self.root)
    }

    pub(in crate::app) fn move_pane(
        &mut self,
        from: EntityId,
        to: EntityId,
        side: TerminalDropSide,
    ) -> bool {
        if from == to || !self.contains(from) || !self.contains(to) {
            return false;
        }
        let (direction, new_first) = match side {
            TerminalDropSide::Left => (TerminalSplitDirection::Right, true),
            TerminalDropSide::Right => (TerminalSplitDirection::Right, false),
            TerminalDropSide::Up => (TerminalSplitDirection::Down, true),
            TerminalDropSide::Down => (TerminalSplitDirection::Down, false),
        };
        let root = std::mem::replace(&mut self.root, TerminalPane::Leaf(from));
        let Some(remaining) = Self::remove_at(root, from) else {
            return false;
        };
        self.root = Self::insert_at(remaining, to, direction, from, new_first);
        true
    }

    #[cfg(test)]
    pub(in crate::app) fn ratio(&self, path: &[bool]) -> Option<f32> {
        let mut pane = &self.root;
        for &side in path {
            let TerminalPane::Split { first, second, .. } = pane else {
                return None;
            };
            pane = if side { second } else { first };
        }
        let TerminalPane::Split { ratio, .. } = pane else {
            return None;
        };
        Some(*ratio)
    }

    pub(in crate::app) fn set_ratio(&mut self, path: &[bool], ratio: f32) -> bool {
        let mut pane = &mut self.root;
        for &side in path {
            let TerminalPane::Split { first, second, .. } = pane else {
                return false;
            };
            pane = if side { second } else { first };
        }
        let TerminalPane::Split { ratio: slot, .. } = pane else {
            return false;
        };
        *slot = ratio;
        true
    }

    pub(in crate::app) fn insert(
        &mut self,
        direction: TerminalSplitDirection,
        pane: Entity<Terminal>,
    ) {
        let id = pane.entity_id();
        self.insert_id(direction, id);
        self.panes.insert(id, pane);
    }

    fn insert_id(&mut self, direction: TerminalSplitDirection, id: EntityId) {
        if !self.contains(self.focused) {
            return;
        }
        let root = std::mem::replace(&mut self.root, TerminalPane::Leaf(self.focused));
        self.root = Self::insert_at(root, self.focused, direction, id, false);
        self.focused = id;
    }

    pub(in crate::app) fn remove(&mut self, id: EntityId) -> bool {
        if self.leaf_count() <= 1 || !self.contains(id) {
            return false;
        }
        let neighbor = Self::neighbor(&self.root, id);
        let root = std::mem::replace(&mut self.root, TerminalPane::Leaf(id));
        let Some(root) = Self::remove_at(root, id) else {
            return false;
        };
        self.root = root;
        self.panes.remove(&id);
        if self.focused == id {
            self.focused = neighbor.unwrap_or_else(|| Self::first_leaf(&self.root));
        }
        true
    }

    fn insert_at(
        pane: TerminalPane,
        target: EntityId,
        direction: TerminalSplitDirection,
        new: EntityId,
        new_first: bool,
    ) -> TerminalPane {
        match pane {
            TerminalPane::Leaf(id) if id == target => {
                let existing = TerminalPane::Leaf(id);
                let added = TerminalPane::Leaf(new);
                let (first, second) = if new_first {
                    (added, existing)
                } else {
                    (existing, added)
                };
                TerminalPane::Split {
                    direction,
                    ratio: 0.5,
                    first: Box::new(first),
                    second: Box::new(second),
                }
            }
            TerminalPane::Leaf(id) => TerminalPane::Leaf(id),
            TerminalPane::Split {
                direction: existing,
                ratio,
                first,
                second,
            } => TerminalPane::Split {
                direction: existing,
                ratio,
                first: Box::new(Self::insert_at(*first, target, direction, new, new_first)),
                second: Box::new(Self::insert_at(*second, target, direction, new, new_first)),
            },
        }
    }

    fn remove_at(pane: TerminalPane, id: EntityId) -> Option<TerminalPane> {
        match pane {
            TerminalPane::Leaf(leaf) => (leaf != id).then_some(TerminalPane::Leaf(leaf)),
            TerminalPane::Split {
                direction,
                ratio,
                first,
                second,
            } => match (Self::remove_at(*first, id), Self::remove_at(*second, id)) {
                (Some(first), Some(second)) => Some(TerminalPane::Split {
                    direction,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (Some(pane), None) | (None, Some(pane)) => Some(pane),
                (None, None) => None,
            },
        }
    }

    fn neighbor(pane: &TerminalPane, id: EntityId) -> Option<EntityId> {
        match pane {
            TerminalPane::Leaf(_) => None,
            TerminalPane::Split { first, second, .. } => {
                if Self::contains_pane(first, id) {
                    Self::neighbor(first, id).or_else(|| Some(Self::first_leaf(second)))
                } else if Self::contains_pane(second, id) {
                    Self::neighbor(second, id).or_else(|| Some(Self::first_leaf(first)))
                } else {
                    None
                }
            }
        }
    }

    fn contains_pane(pane: &TerminalPane, id: EntityId) -> bool {
        match pane {
            TerminalPane::Leaf(leaf) => *leaf == id,
            TerminalPane::Split { first, second, .. } => {
                Self::contains_pane(first, id) || Self::contains_pane(second, id)
            }
        }
    }

    fn first_leaf(pane: &TerminalPane) -> EntityId {
        match pane {
            TerminalPane::Leaf(id) => *id,
            TerminalPane::Split { first, .. } => Self::first_leaf(first),
        }
    }

    fn leaf_ids_of(pane: &TerminalPane) -> Vec<EntityId> {
        match pane {
            TerminalPane::Leaf(id) => vec![*id],
            TerminalPane::Split { first, second, .. } => {
                let mut ids = Self::leaf_ids_of(first);
                ids.extend(Self::leaf_ids_of(second));
                ids
            }
        }
    }
}

impl FarcasterApp {
    pub(in crate::app) fn show_terminal_surface(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.center_surface_switch_blocked() {
            return;
        }
        let target = self.composer.sessions.current_target().to_owned();
        self.activate_terminal_for_target(target, self.workspace_project(), window, cx);
    }

    pub(in crate::app) fn activate_terminal_for_target(
        &mut self,
        target: String,
        project: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.project.repository.execution_allowed {
            self.notify_workspace_error(
                "Terminal",
                "Trust this project before opening its terminal.".to_owned(),
                cx,
            );
            return;
        }

        let project = project.canonicalize().unwrap_or(project);
        let cached = self
            .workspace
            .terminal
            .terminals
            .get(&target)
            .filter(|terminal| terminal.read(cx).is_alive())
            .cloned();
        let terminal = if let Some(terminal) = cached {
            terminal
        } else {
            let terminal = match spawn_workspace_terminal(
                crate::app::infrastructure::shell_environment::terminal_login_shell_command(),
                project,
                LoginBanner::Visible,
                window,
                cx,
            ) {
                Ok(terminal) => terminal,
                Err(error) => {
                    self.notify_workspace_error("Terminal", error, cx);
                    return;
                }
            };
            self.workspace
                .terminal
                .terminals
                .insert(target.clone(), terminal.clone());
            let monitored = terminal.downgrade();
            let monitored_target = target.clone();
            self.monitor_native_process(window, cx, move |this, window, cx| {
                let Some(monitored) = monitored.upgrade() else {
                    return false;
                };
                if this.workspace.terminal.terminals.get(&monitored_target) != Some(&monitored) {
                    return false;
                }
                if monitored.read(cx).is_alive() {
                    return true;
                }
                if this.workspace.terminal.view.as_ref() != Some(&monitored) {
                    this.workspace.terminal.terminals.remove(&monitored_target);
                } else if this.workspace.surface == AppSurface::Terminal {
                    this.close_terminal(window, cx);
                } else {
                    this.clear_terminal_process();
                }
                false
            });
            terminal
        };

        self.hide_terminal(cx);
        self.workspace.terminal.view = Some(terminal);
        self.workspace.terminal.active_target = Some(target);
        self.hide_editor(cx);
        self.reveal_native_center_surface(AppSurface::Terminal, window, cx);
    }

    fn clear_terminal_process(&mut self) {
        if let Some(target) = self.workspace.terminal.active_target.take() {
            self.workspace.terminal.terminals.remove(&target);
            self.workspace.terminal.layouts.remove(&target);
        }
        self.workspace.terminal.view = None;
    }

    pub(in crate::app) fn forget_terminal_for_target(&mut self, target: &str) {
        self.workspace.terminal.terminals.remove(target);
        self.workspace.terminal.layouts.remove(target);
        if self.workspace.terminal.active_target.as_deref() == Some(target) {
            self.workspace.terminal.view = None;
            self.workspace.terminal.active_target = None;
        }
    }

    /// Repaints every live terminal with the active theme without restarting it.
    pub(in crate::app) fn apply_terminal_theme(&mut self, cx: &mut Context<Self>) {
        let theme = crate::app::ui::theme::terminal_theme();
        let mut themed = HashSet::new();
        for terminal in self.workspace.terminal.terminals.values().cloned().chain(
            self.workspace
                .terminal
                .layouts
                .values()
                .flat_map(|layout| layout.terminals()),
        ) {
            if !themed.insert(terminal.entity_id()) {
                continue;
            }
            terminal.update(cx, |terminal, _| {
                if terminal.is_alive() {
                    let _ = terminal.update_theme(theme);
                }
            });
        }
        let editors = self
            .workspace
            .editor
            .project_editors
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for editor in editors {
            editor.update(cx, |editor, cx| editor.update_theme(cx));
        }
        if self.workspace.native_surface_covered && self.workspace.surface == AppSurface::Terminal {
            self.set_terminal_hidden_rendering(true, cx);
        }
        self.refresh_covered_workspace_snapshot(cx);
    }

    /// Lets a covered terminal keep rendering while an overlay presents it, so a
    /// theme change is visible in the overlay instead of only after it closes.
    pub(in crate::app) fn set_terminal_hidden_rendering(
        &self,
        rendered: bool,
        cx: &mut Context<Self>,
    ) {
        for terminal in self.active_terminal_panes() {
            terminal.update(cx, |terminal, _| terminal.set_hidden_rendering(rendered));
        }
    }

    pub(in crate::app) fn hide_terminal(&self, cx: &mut Context<Self>) {
        for terminal in self.active_terminal_panes() {
            terminal.update(cx, |terminal, _| terminal.set_visible(false));
        }
    }

    pub(in crate::app) fn restore_terminal_visibility(&self, cx: &mut Context<Self>) {
        if self.workspace.surface == AppSurface::Terminal {
            for terminal in self.active_terminal_panes() {
                terminal.update(cx, |terminal, _| terminal.set_visible(true));
            }
        }
    }

    pub(in crate::app) fn close_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let closing = self.active_owned_terminals();
        self.clear_terminal_process();
        self.retire_terminals(closing, window, cx);
        self.show_chat_surface(window, cx);
    }

    fn active_owned_terminals(&self) -> Vec<Entity<Terminal>> {
        let mut ids = HashSet::new();
        let mut candidates = Vec::new();
        candidates.extend(self.workspace.terminal.view.clone());
        if let Some(target) = self.workspace.terminal.active_target.as_ref() {
            candidates.extend(self.workspace.terminal.terminals.get(target).cloned());
            if let Some(layout) = self.workspace.terminal.layouts.get(target) {
                candidates.extend(layout.terminals());
            }
        }
        let mut terminals = Vec::new();
        for terminal in candidates {
            if ids.insert(terminal.entity_id()) {
                terminals.push(terminal);
            }
        }
        terminals
    }

    fn retire_terminals(
        &mut self,
        terminals: Vec<Entity<Terminal>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if terminals.is_empty() {
            return;
        }
        self.workspace.terminal.closing.extend(terminals);
        cx.spawn_in(window, async move |weak, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(80))
                .await;
            let _ = weak.update_in(cx, |this, _, _| {
                this.workspace.terminal.closing.clear();
            });
        })
        .detach();
    }

    pub(in crate::app) fn active_terminal_layout(&self) -> Option<&TerminalLayout> {
        let target = self.workspace.terminal.active_target.as_ref()?;
        self.workspace.terminal.layouts.get(target)
    }

    pub(in crate::app) fn active_terminal(&self) -> Option<Entity<Terminal>> {
        self.active_terminal_layout()
            .and_then(|layout| layout.focused_terminal())
            .cloned()
            .or_else(|| self.workspace.terminal.view.clone())
    }

    fn active_terminal_panes(&self) -> Vec<Entity<Terminal>> {
        if let Some(layout) = self.active_terminal_layout() {
            return layout.terminals();
        }
        self.workspace.terminal.view.iter().cloned().collect()
    }

    pub(in crate::app) fn split_terminal(
        &mut self,
        direction: TerminalSplitDirection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.workspace.surface != AppSurface::Terminal
            || self.native_workspace_covered_by_overlay()
        {
            return;
        }
        if !self.project.repository.execution_allowed {
            self.notify_workspace_error(
                "Terminal",
                "Trust this project before opening its terminal.".to_owned(),
                cx,
            );
            return;
        }
        let Some(target) = self.workspace.terminal.active_target.clone() else {
            return;
        };
        let Some(primary) = self.workspace.terminal.terminals.get(&target).cloned() else {
            return;
        };
        let project = self.workspace_project();
        let project = project.canonicalize().unwrap_or(project);
        let pane = match spawn_workspace_terminal(
            crate::app::infrastructure::shell_environment::terminal_login_shell_command(),
            project,
            LoginBanner::Visible,
            window,
            cx,
        ) {
            Ok(pane) => pane,
            Err(error) => {
                self.notify_workspace_error("Terminal", error, cx);
                return;
            }
        };
        let layout = self
            .workspace
            .terminal
            .layouts
            .entry(target.clone())
            .or_insert_with(|| TerminalLayout::new(primary));
        layout.insert(direction, pane.clone());
        self.monitor_split_terminal(target, pane, window, cx);
        cx.notify();
    }

    fn monitor_split_terminal(
        &mut self,
        target: String,
        pane: Entity<Terminal>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let monitored = pane.downgrade();
        self.monitor_native_process(window, cx, move |this, window, cx| {
            let Some(pane) = monitored.upgrade() else {
                return false;
            };
            let pane_id = pane.entity_id();
            let primary = this
                .workspace
                .terminal
                .terminals
                .get(&target)
                .map(|primary| primary.entity_id());
            if primary == Some(pane_id) {
                return false;
            }
            if pane.read(cx).is_alive() {
                return true;
            }
            this.remove_terminal_pane(&target, pane_id, window, cx);
            false
        });
    }

    pub(in crate::app) fn remove_terminal_pane(
        &mut self,
        target: &str,
        pane: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let removed = self
            .workspace
            .terminal
            .layouts
            .get(target)
            .and_then(|layout| layout.terminal(pane))
            .cloned();
        let Some(layout) = self.workspace.terminal.layouts.get_mut(target) else {
            return;
        };
        if !layout.remove(pane) {
            return;
        }
        self.sync_terminal_layout(target);
        if let Some(removed) = removed {
            self.retire_terminals(vec![removed], window, cx);
        }
    }

    fn sync_terminal_layout(&mut self, target: &str) {
        let Some(layout) = self.workspace.terminal.layouts.get(target) else {
            return;
        };
        let collapse = layout.leaf_count() <= 1;
        let single = layout
            .single_leaf_id()
            .and_then(|id| layout.terminal(id).cloned());
        let primary = self
            .workspace
            .terminal
            .terminals
            .get(target)
            .map(|primary| primary.entity_id());
        let primary_missing = !primary.is_some_and(|id| layout.contains(id));
        let focused = layout.terminal(layout.focused_id()).cloned();
        let promote = if collapse {
            self.workspace.terminal.layouts.remove(target);
            single
        } else if primary_missing {
            focused
        } else {
            None
        };
        let Some(pane) = promote else {
            return;
        };
        self.workspace
            .terminal
            .terminals
            .insert(target.to_owned(), pane.clone());
        if self.workspace.terminal.active_target.as_deref() == Some(target) {
            self.workspace.terminal.view = Some(pane);
        }
    }

    pub(in crate::app) fn close_terminal_or_split(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let split = self
            .workspace
            .terminal
            .active_target
            .as_ref()
            .and_then(|target| {
                self.workspace
                    .terminal
                    .layouts
                    .get(target)
                    .map(|layout| (target.clone(), layout.focused_id()))
            });
        let Some((target, focused)) = split else {
            self.close_terminal(window, cx);
            return;
        };
        self.remove_terminal_pane(&target, focused, window, cx);
        if let Some(pane) = self.active_terminal() {
            pane.update(cx, |terminal, cx| terminal.focus(window, cx));
        }
        cx.notify();
    }

    pub(in crate::app) fn focus_terminal_pane(
        &mut self,
        pane: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.workspace.terminal.active_target.clone() else {
            return;
        };
        let Some(layout) = self.workspace.terminal.layouts.get_mut(&target) else {
            return;
        };
        layout.set_focused(pane);
        let focused = layout.focused_terminal().cloned();
        if let Some(terminal) = focused {
            terminal.update(cx, |terminal, cx| terminal.focus(window, cx));
        }
        cx.notify();
    }

    pub(in crate::app) fn move_terminal_pane(
        &mut self,
        from: EntityId,
        to: EntityId,
        side: TerminalDropSide,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.workspace.terminal.active_target.clone() else {
            return;
        };
        let Some(layout) = self.workspace.terminal.layouts.get_mut(&target) else {
            return;
        };
        if !layout.move_pane(from, to, side) {
            return;
        }
        self.focus_terminal_pane(from, window, cx);
    }

    pub(in crate::app) fn set_terminal_split_ratio(
        &mut self,
        path: &[bool],
        ratio: f32,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.workspace.terminal.active_target.clone() else {
            return;
        };
        let Some(layout) = self.workspace.terminal.layouts.get_mut(&target) else {
            return;
        };
        layout.set_ratio(path, ratio.clamp(0.1, 0.9));
        cx.notify();
    }

    pub(in crate::app) fn cycle_terminal_focus(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.workspace.surface != AppSurface::Terminal {
            return;
        }
        let Some(target) = self.workspace.terminal.active_target.clone() else {
            return;
        };
        let Some(layout) = self.workspace.terminal.layouts.get_mut(&target) else {
            return;
        };
        let ids = layout.leaf_ids();
        if ids.len() < 2 {
            return;
        }
        let current = ids
            .iter()
            .position(|&id| id == layout.focused_id())
            .unwrap_or(0);
        let next = if forward {
            (current + 1) % ids.len()
        } else {
            (current + ids.len() - 1) % ids.len()
        };
        layout.set_focused(ids[next]);
        let focused = layout.focused_terminal().cloned();
        if let Some(terminal) = focused {
            terminal.update(cx, |terminal, cx| terminal.focus(window, cx));
        }
        cx.notify();
    }
}

#[cfg(test)]
#[path = "terminal_tests.rs"]
mod tests;
