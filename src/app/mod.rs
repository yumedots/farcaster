use crate::agents::Backend;
mod bootstrap;
mod change_detection;
mod composer;
mod event_projection;
pub(crate) mod extensions;
pub(crate) mod infrastructure;
#[cfg(test)]
mod live_e2e_tests;
#[cfg(test)]
pub(crate) mod test_support;
#[allow(unused_imports)]
pub(crate) use infrastructure::{launch, paths, persistence, shell_environment};
mod navigation;
mod project;
pub(crate) mod runtime;
mod session;
mod session_folders;
pub(crate) mod ui;
pub(crate) mod views;
mod workspace;
use change_detection::*;
pub(crate) use composer::ComposerImage;
pub(crate) use composer::ComposerPaste;
use composer::submissions::PendingSubmission;
use composer::{completion as composer_completion, file_mentions};
pub(crate) use navigation::{PICKER_KEY_CONTEXT, PickerScope, ProjectPickerIntent};
use project::{registry as project_registry, repository};
use session::{archive, drafts, status::roots_waiting_for_descendants};
pub(crate) use views::OVERLAY_KEY_CONTEXT;
pub(crate) use views::transcript::list::TRANSCRIPT_SELECTION_KEY_CONTEXT;
use views::{
    ComposerView, InactiveSessionRailView, RunPanelView, SessionRailKind, SessionRailView,
    TranscriptView,
};

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use gpui::{
    AppContext as _, Context, Entity, FocusHandle, Focusable as _, Image, PathPromptOptions,
    RenderImage, Subscription, SystemNotification, Task, Window, actions,
};
use gpui_component::input::{InputEvent, InputState, TextareaState};
use gpui_libghostty::Terminal;
use workspace::editor_session::EditorSession;

use crate::{
    agent_activity::AgentActivity,
    app::composer::sessions::{
        ComposerSessions, ComposerSnapshot, HistoryNavigation, draft_target, project_target,
        session_target,
    },
    app::extensions::ExtensionUiState,
    app::views::transcript::list::TranscriptListState,
    projects,
    protocol::Model,
    runtime::{RuntimeCommand, RuntimeEvent, RuntimeHandle, RuntimeSnapshot},
    sessions::{
        SessionRootIndex, SessionSummary, SessionTarget, descendant_sessions_for_root,
        root_session_for_path,
    },
};
#[cfg(test)]
use crate::{app::views::transcript::transcript_splice, protocol::ExtensionUiRequest};

const SYSTEM_NOTIFICATION_TAG: &str = "farcaster-agent";
pub(crate) const COMPOSER_KEY_CONTEXT: &str = "FarcasterComposer";
pub(crate) const APP_SHORTCUT_CONTEXT: &str = "FarcasterApp && input == app";
pub(crate) const APP_INPUT_CONTEXT: &str = "FarcasterApp input=app";
pub(crate) const NATIVE_INPUT_CONTEXT: &str = "FarcasterApp input=native";
pub(crate) const CHAT_INPUT_CONTEXT: &str = "FarcasterApp input=app surface=chat";
pub(crate) const CHAT_SHORTCUT_CONTEXT: &str = "FarcasterApp && input == app && surface == chat";

#[derive(Debug, Eq, PartialEq)]
enum CurrentCloseTarget {
    Draft(String),
    Session(PathBuf),
    None,
}

actions!(
    farcaster,
    [
        DismissSurface,
        QuitApplication,
        SubmitFollowUp,
        SwitchSession0,
        SwitchSession1,
        SwitchSession2,
        SwitchSession3,
        SwitchSession4,
        SwitchSession5,
        SwitchSession6,
        SwitchSession7,
        SwitchSession8,
        SwitchSession9,
        NewSession,
        AddProject,
        SetSandbox,
        SetRuntime,
        SetHarness,
        RestoreSession,
        ShowActionPicker,
        PickerBack,
        PickerNavigateBack,
        FocusSessionSearch,
        FocusComposer,
        ShowEditor,
        OpenTranscriptScratch,
        ShowTerminal,
        PreviousSession,
        NextSession,
        NextTranscriptSession,
        PreviousTranscriptSession,
        ToggleArchivedSessions,
        SubmitPrompt,
        AbortRun,
        ComposerEscape,
        CloseCurrent,
        SplitTerminalRight,
        SplitTerminalDown,
        TerminalFocusNext,
        TerminalFocusPrevious,
        ComposerHistoryPrevious,
        ComposerHistoryNext,
        ComposerCompletionPrevious,
        ComposerCompletionNext,
        ShowKeybindings,
        IncreaseTranscriptFontSize,
        DecreaseTranscriptFontSize
    ]
);

#[derive(Clone, Debug, Eq, PartialEq, gpui::Action)]
#[action(namespace = farcaster, no_json)]
pub(crate) struct RemoveProject {
    pub(crate) path: PathBuf,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum AppSurface {
    #[default]
    Chat,
    Editor,
    Terminal,
    Diff,
}

enum PostRenderFocus {
    ActiveSurface(Option<FocusHandle>),
    ImagePreview,
}

#[derive(Clone)]
struct SessionTitleEdit {
    path: PathBuf,
    project: PathBuf,
    original: String,
}

#[derive(Clone)]
pub(crate) struct ImagePreview {
    pub(crate) image: Arc<Image>,
    pub(crate) index: usize,
    pub(crate) total: usize,
}

/// An open in-app diff, keyed by the change it was opened from so it can follow
/// the working copy as it moves.
pub(crate) struct RepositoryDiff {
    pub(crate) key: crate::repository::DiffTargetKey,
    pub(crate) path: PathBuf,
    pub(crate) layer: crate::repository::ChangeLayer,
    pub(crate) additions: u64,
    pub(crate) deletions: u64,
    pub(crate) diff: Option<crate::repository::FileDiff>,
    pub(crate) error: Option<String>,
    pub(crate) applying: Option<usize>,
    /// Whether the changed lines are shown against each other or stacked.
    /// Side by side is what a diff opens as, the way an editor does it.
    pub(crate) split: bool,
    pub(crate) hide_unchanged: bool,
    pub(crate) opened: Vec<usize>,

    pub(crate) widest_left: gpui::Pixels,
    pub(crate) widest_right: gpui::Pixels,
    pub(crate) rows: Vec<crate::repository::DiffRow>,
    pub(crate) scroll: gpui::ScrollHandle,
    generation: u64,
}

impl RepositoryDiff {
    pub(crate) fn new(
        key: crate::repository::DiffTargetKey,
        path: PathBuf,
        layer: crate::repository::ChangeLayer,
    ) -> Self {
        Self {
            key,
            path,
            layer,
            additions: 0,
            deletions: 0,
            diff: None,
            error: None,
            applying: None,
            split: true,
            hide_unchanged: false,
            opened: Vec::new(),
            widest_left: gpui::px(0.0),
            widest_right: gpui::px(0.0),
            rows: Vec::new(),
            scroll: gpui::ScrollHandle::new(),
            generation: 0,
        }
    }

    pub(crate) fn preparing(&self) -> bool {
        self.diff.is_none() && self.error.is_none()
    }

    pub(crate) fn split_reading(&self) -> bool {
        self.split
            && !self
                .diff
                .as_ref()
                .is_some_and(crate::repository::FileDiff::is_new_file)
    }

    pub(crate) fn set_diff(
        &mut self,
        diff: crate::repository::FileDiff,
        hide_unchanged: bool,
        text_system: &gpui::TextSystem,
    ) {
        self.additions = diff.hunks.iter().map(|hunk| hunk.additions as u64).sum();
        self.deletions = diff.hunks.iter().map(|hunk| hunk.deletions as u64).sum();
        let widths = widest_line_widths(&diff, text_system);
        self.widest_left = gpui::px(widths.old);
        self.widest_right = gpui::px(widths.new);
        self.hide_unchanged = hide_unchanged;
        self.diff = Some(diff);
        self.opened.clear();
        self.build_rows();
    }

    pub(crate) fn set_split(&mut self, split: bool) {
        if self.split == split {
            return;
        }
        self.split = split;
        self.build_rows();
        self.scroll
            .set_offset(gpui::point(gpui::px(0.0), gpui::px(0.0)));
    }

    pub(crate) fn set_hide_unchanged(&mut self, hidden: bool) {
        if self.hide_unchanged == hidden {
            return;
        }
        self.hide_unchanged = hidden;
        self.opened.clear();
        self.build_rows();
    }

    pub(crate) fn toggle_span(&mut self, span: usize) {
        match self.opened.iter().position(|open| *open == span) {
            Some(index) => {
                self.opened.remove(index);
            }
            None => self.opened.push(span),
        }
        self.build_rows();
    }

    fn build_rows(&mut self) {
        let split = self.split;
        let hidden = self.hide_unchanged;
        let opened = self.opened.clone();
        self.rows = self
            .diff
            .as_ref()
            .map_or_else(Vec::new, |diff| diff.rows(split, hidden, &opened));
    }

    /// Which hunk actions this file's section allows.
    pub(crate) fn actions(&self) -> &'static [crate::repository::HunkApply] {
        use crate::repository::{ChangeLayer, HunkApply};
        match self.layer {
            ChangeLayer::Index => &[HunkApply::Unstage],
            ChangeLayer::WorkingTree | ChangeLayer::Untracked => {
                &[HunkApply::Stage, HunkApply::Revert]
            }
            ChangeLayer::Conflict => &[],
            ChangeLayer::JujutsuWorkingCopy => &[],
        }
    }

    pub(crate) fn next_generation(&mut self) -> u64 {
        self.generation = self.generation.saturating_add(1);
        self.generation
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }
}

pub(in crate::app) fn diff_text(text: &str) -> String {
    text.replace('\t', "    ")
}

fn widest_line_widths(
    file: &crate::repository::FileDiff,
    text_system: &gpui::TextSystem,
) -> crate::repository::SideWidths {
    let font_size = crate::app::ui::theme::theme().type_scale.caption;
    let font_id = text_system.resolve_font(&gpui::font(crate::app::ui::theme::MONO_FONT_FAMILY));
    let cell = text_system
        .ch_advance(font_id, font_size)
        .unwrap_or_else(|_| gpui::px(0.0));
    let width = |text: &str| {
        diff_text(text)
            .chars()
            .map(|character| {
                if character.is_ascii() {
                    cell
                } else {
                    text_system
                        .advance(font_id, font_size, character)
                        .map_or(cell, |advance| advance.width)
                }
            })
            .fold(gpui::px(0.0), |width, character| width + character)
    };
    file.widest_sides(|text| f32::from(width(text)))
}

pub(crate) struct FarcasterApp {
    runtime: RuntimeHandle,
    pub(crate) snapshot: Arc<RuntimeSnapshot>,
    runtime_generation: u64,
    project: project::ProjectState,
    sessions: session::SessionState,
    activity: session::ActivityState,
    composer: composer::ComposerState,
    navigation: navigation::NavigationState,
    workspace: workspace::WorkspaceState,
    settings: workspace::SettingsState,
    extensions: extensions::ExtensionState,
    views: views::AppViews,
    overlays: views::AppOverlays,
    lifecycle: infrastructure::AppLifecycle,
}

#[cfg(test)]
mod tests;
