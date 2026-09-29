mod adapter;
mod contract;
mod core;

pub(crate) use adapter::project_shell_environment;
pub(crate) use adapter::{
    annotate_history_message, app_shell_environment, apply_project_trust, available_access_modes,
    backend_display_name, backend_statuses, default_login_shell, delete_session_family,
    discover_sessions_for, effort_label, external_session_identity, generate_session_title,
    load_configuration_catalog, load_session_history, move_session_family, program_available,
    program_available_in, project_trust, project_trust_description, rename_session,
    saved_project_trust, spawn_session, supports_auto_title_generation, supports_reasoning_effort,
    supports_reasoning_reset, supports_sandbox_discovery, supports_session_fork,
    supports_session_move, supports_startup_command, supports_steering, validate_launch,
    validate_session_move,
};
pub(crate) use contract::extensions;
pub(crate) use contract::{
    AgentLaunchConfig, Backend, ConfigurationCatalog, DiscoveredHistory, DiscoveredSession,
    DiscoveredUsage, HarnessAccessMode, PeerMessage, PromptOutcome, PromptPresentation,
    QueuedPrompt, SandboxState, SessionActivityKind, SessionCommand, SessionContextUsage,
    SessionEvent, SessionHistory, SessionLaunch, SessionMetadata, SessionOperation,
    SessionResponse, SessionResponseErrorKind, SessionResponsePayload, SessionStart,
    SessionTransport, SessionUsage, SessionUsageTokens, WorkerContext, WorkerInput,
    WorkerInputResponse, effort_rank, model_efforts, valid_worker_name,
};
pub(crate) use core::{
    CallerProfile, CallerRegistry, ChildSessionOutcome, CommonTool, PromptStore, TokenUsage,
    ToolCategory, ToolMetadata, ToolReviewState, WorkerActivity, WorkerActivityState, WorkerEvent,
    WorkerLaunch, WorkerModelSelection, WorkerSendMode, WorkerSession, WorkerSessionFactory,
    WorkerUsage, begin_prompt, complete_prompt_with_receipt, enqueue_prompt_with_presentation,
    fail_prompt, has_queued_prompts_for, is_child_input_id, mark_prompt_delivery_unknown,
    queued_prompts,
};
#[cfg(test)]
pub(crate) use core::{WorkerAssignment, WorkerExecution, WorkerFamilyLink, WorkerRouting};

#[cfg(test)]
pub(crate) use adapter::live_tests::support as live_e2e_support;
