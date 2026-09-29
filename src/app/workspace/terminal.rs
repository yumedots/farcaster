use std::path::PathBuf;

use gpui::{Context, Window};

use super::{AppSurface, FarcasterApp, LoginBanner, spawn_workspace_terminal};

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
        }
        self.workspace.terminal.view = None;
    }

    pub(in crate::app) fn forget_terminal_for_target(&mut self, target: &str) {
        self.workspace.terminal.terminals.remove(target);
        if self.workspace.terminal.active_target.as_deref() == Some(target) {
            self.workspace.terminal.view = None;
            self.workspace.terminal.active_target = None;
        }
    }

    /// Repaints every live terminal with the active theme without restarting it.
    pub(in crate::app) fn apply_terminal_theme(&mut self, cx: &mut Context<Self>) {
        let theme = crate::app::ui::theme::terminal_theme();
        for terminal in self.workspace.terminal.terminals.values() {
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
        if let Some(terminal) = self.workspace.terminal.view.as_ref() {
            terminal.update(cx, |terminal, _| terminal.set_hidden_rendering(rendered));
        }
    }

    pub(in crate::app) fn hide_terminal(&self, cx: &mut Context<Self>) {
        if let Some(terminal) = self.workspace.terminal.view.as_ref() {
            terminal.update(cx, |terminal, _| terminal.set_visible(false));
        }
    }

    pub(in crate::app) fn restore_terminal_visibility(&self, cx: &mut Context<Self>) {
        if self.workspace.surface == AppSurface::Terminal
            && let Some(terminal) = self.workspace.terminal.view.as_ref()
        {
            terminal.update(cx, |terminal, _| terminal.set_visible(true));
        }
    }

    pub(in crate::app) fn close_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hide_terminal(cx);
        self.clear_terminal_process();
        self.show_chat_surface(window, cx);
    }
}
