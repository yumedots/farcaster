use crate::app::*;

use super::TerminalLayout;

pub(in crate::app) struct WorkspaceState {
    pub(in crate::app) editor: EditorState,
    pub(in crate::app) terminal: TerminalState,
    pub(in crate::app) native_surface_snapshot: Option<Arc<RenderImage>>,
    pub(in crate::app) terminal_snapshots: HashMap<gpui::EntityId, Arc<RenderImage>>,
    pub(in crate::app) native_surface_covered: bool,
    pub(in crate::app) native_surface_refresh: Option<Task<()>>,
    pub(in crate::app) tooltip_watch: Option<Subscription>,
    pub(in crate::app) session_rail_hidden: bool,
    pub(in crate::app) run_panel_hidden: bool,
    pub(in crate::app) surface: AppSurface,
    pub(in crate::app) session_surfaces: HashMap<String, AppSurface>,
    pub(in crate::app) diffs: Vec<RepositoryDiff>,
    pub(in crate::app) active_diff: Option<crate::repository::DiffTargetKey>,
    pub(in crate::app) diff_return: Option<AppSurface>,
    pub(in crate::app) runtime_picker: workspace::runtime_picker::RuntimePickerState,
    pub(in crate::app) send_to_chat: Option<workspace::send_to_chat::SendToChat>,
    pub(in crate::app) send_to_chat_capture: Option<Task<()>>,
    pub(in crate::app) code_tasks: workspace::code_tasks::CodeTasks,
}

pub(in crate::app) struct EditorState {
    pub(in crate::app) view: Option<Entity<EditorSession>>,
    pub(in crate::app) active_review: Option<workspace::review::ActiveReview>,
    pub(in crate::app) project_editors: HashMap<(PathBuf, u64), Entity<EditorSession>>,
    pub(in crate::app) session_tabs: HashMap<String, u64>,
    pub(in crate::app) ready: bool,
    pub(in crate::app) request_generation: u64,
    pub(in crate::app) return_focus: Option<FocusHandle>,
}

pub(in crate::app) struct TerminalState {
    pub(in crate::app) view: Option<Entity<Terminal>>,
    pub(in crate::app) terminals: HashMap<String, Entity<Terminal>>,
    pub(in crate::app) active_target: Option<String>,
    pub(in crate::app) layouts: HashMap<String, TerminalLayout>,
    pub(in crate::app) closing: Vec<Entity<Terminal>>,
    pub(in crate::app) hovered_handle: Option<gpui::EntityId>,
    pub(in crate::app) dragging_pane: Option<gpui::EntityId>,
    pub(in crate::app) drop_side:
        std::rc::Rc<std::cell::RefCell<Option<(gpui::EntityId, super::TerminalDropSide)>>>,
    pub(in crate::app) pane_bounds:
        std::rc::Rc<std::cell::RefCell<HashMap<gpui::EntityId, gpui::Bounds<gpui::Pixels>>>>,
}

pub(in crate::app) struct SettingsState {
    pub(in crate::app) themes: workspace::theme_settings::ThemeSettings,
    pub(in crate::app) network_proxy_input: Entity<InputState>,
    pub(in crate::app) network_proxy_error: Option<String>,
    pub(in crate::app) text_editor: Option<String>,
    pub(in crate::app) text_editor_input: Entity<InputState>,
    pub(in crate::app) text_editor_error: Option<String>,
    pub(in crate::app) proxy_save: Option<Task<()>>,
    pub(in crate::app) expand_transcript_folders: bool,
    pub(in crate::app) transcript_error: Option<String>,
    pub(in crate::app) stage_changes_like_vscode: bool,
    pub(in crate::app) source_control_icon_only: bool,
    pub(in crate::app) hide_unchanged_lines: bool,
    pub(in crate::app) hide_split_borders: bool,
    pub(in crate::app) terminal_error: Option<String>,
    pub(in crate::app) source_control_view: crate::app::ui::change_tree::ChangeView,
    pub(in crate::app) source_control_sort: crate::app::ui::change_tree::ChangeSort,
    pub(in crate::app) source_control_error: Option<String>,
    pub(in crate::app) _network_proxy_subscription: Subscription,
    pub(in crate::app) _text_editor_subscription: Subscription,
}
