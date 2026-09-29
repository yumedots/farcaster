use crate::agents::Backend;
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, mpsc},
    thread,
    time::SystemTime,
};

use super::*;
use crate::agents::{WorkerLaunch, WorkerPool, WorkerSession, WorkerSessionFactory};
use crate::sessions::UsageSummary;
use serde_json::json;

fn summary(project: &Path, id: &str, parent: Option<&str>) -> SessionSummary {
    SessionSummary::from_cached(
        id.into(),
        project.join(format!("{id}.jsonl")),
        project.to_owned(),
        id.into(),
        String::new(),
        String::new(),
        parent.map(str::to_owned),
        SystemTime::now(),
        0,
        UsageSummary::default(),
        false,
        true,
        String::new(),
    )
}

struct UnusedFactory;

impl WorkerSessionFactory for UnusedFactory {
    fn create(&self, _launch: WorkerLaunch) -> Result<Box<dyn WorkerSession>, String> {
        Err("no worker can be started".into())
    }
}

fn empty_pool() -> Result<WorkerPool, String> {
    let factory: Arc<dyn WorkerSessionFactory> = Arc::new(UnusedFactory);
    WorkerPool::new(BTreeMap::from([(Backend::Pi, factory)]))
}

fn supervisor_for_family(
    state: StateStore,
    sessions: Vec<SessionSummary>,
) -> (Supervisor, mpsc::Receiver<RuntimeEvent>) {
    let (_commands, command_rx) = mpsc::channel();
    let (events_tx, events) = mpsc::channel();
    let (wake, _) = async_channel::bounded(1);
    let (_, configuration_rx) = mpsc::channel();
    (
        Supervisor {
            process_command: AgentLaunchConfig::default(),
            command_rx,
            event_tx: UiEventSender {
                events: events_tx,
                wake,
            },
            supervisor_thread: thread::current(),
            catalog_key: "catalog".into(),
            actors: HashMap::new(),
            selected: "catalog".into(),
            selected_project: sessions[0].project.clone(),
            selected_session: None,
            generation: 0,
            latest: HashMap::new(),
            catalog_sessions: sessions,
            catalog_generation: 7,
            actor_paths: HashMap::new(),
            failed_actor_shutdowns: HashMap::new(),
            interacted: HashSet::new(),
            document_revisions: HashMap::new(),
            pending_extensions: HashMap::new(),
            active_dialogs: HashMap::new(),
            needs_input: HashSet::new(),
            clock: 0,
            last_touch: HashMap::new(),
            configurations: HarnessConfigurationStore::default(),
            catalog_state: Some(state),
            configuration_catalogs: Vec::new(),
            configuration_rx,
            configuration_tx: None,
            configuration_requests: HashSet::new(),
            requested_access_modes: HashMap::new(),
            published_statuses: HashMap::new(),
            recovery: Default::default(),
            published_recovery_selection: None,
        },
        events,
    )
}

fn archived(database: &Path, path: &Path) -> Result<bool, String> {
    let state = StateStore::open_at(database)?;
    let path = crate::sessions::normalize_session_path(path);
    Ok(state
        .cached_sessions("")?
        .into_iter()
        .find(|session| session.path == path)
        .ok_or_else(|| format!("missing session {}", path.display()))?
        .archived)
}

#[test]
fn retrying_actor_blocks_destructive_family_commands() {
    let mut snapshot = RuntimeSnapshot::default();
    assert!(!session_actor_has_active_work(&snapshot, false));
    Arc::make_mut(&mut snapshot.conversation).reduce(&json!({
        "type": "auto_retry_start",
        "attempt": 1,
    }));

    assert!(!snapshot.conversation.running);
    assert!(snapshot.conversation.retrying);
    assert!(session_actor_has_active_work(&snapshot, false));
}

#[test]
fn supervisor_does_not_archive_or_report_stopped_when_actor_close_fails() -> Result<(), String> {
    let temp = tempfile::tempdir().map_err(|error| error.to_string())?;
    let database = temp.path().join("state.sqlite3");
    let root = summary(temp.path(), "root", None);
    let mut state = StateStore::open_at(&database)?;
    state.replace_sessions(std::slice::from_ref(&root))?;
    let pool = empty_pool()?;
    let (mut supervisor, events) = supervisor_for_family(state, vec![root.clone()]);
    let key = format!("session:{}", root.path.display());
    let (commands, _command_rx) = mpsc::channel();
    let (_event_tx, actor_events) = mpsc::channel();
    let join = thread::spawn(|| Err("actor transport close failed".to_owned()));
    supervisor
        .actor_paths
        .insert(root.path.clone(), key.clone());
    supervisor.actors.insert(
        key,
        SessionRuntimeHandle {
            commands,
            events: actor_events,
            thread: join.thread().clone(),
            join,
        },
    );

    crate::app::worker_pool::with_test_worker_pool(pool, || {
        assert!(
            supervisor.handle_session_family_command(&RuntimeCommand::StopSessionFamily {
                path: root.path.clone(),
            })
        );
        assert!(!archived(&database, &root.path)?);
        let first_events = events.try_iter().collect::<Vec<_>>();
        assert!(first_events.iter().any(|event| matches!(
            event,
            RuntimeEvent::SessionsFailed { message, .. }
                if message.contains("actor transport close failed")
        )));
        assert!(first_events.iter().all(|event| !matches!(
            event,
            RuntimeEvent::SessionStatus { status, .. } if status == "Stopped"
        )));
        assert!(
            supervisor.handle_session_family_command(&RuntimeCommand::StopSessionFamily {
                path: root.path.clone(),
            })
        );
        assert!(!archived(&database, &root.path)?);
        let retry_events = events.try_iter().collect::<Vec<_>>();
        assert!(retry_events.iter().any(|event| matches!(
            event,
            RuntimeEvent::SessionsFailed { message, .. }
                if message.contains("actor transport close failed")
        )));
        assert!(retry_events.iter().all(|event| !matches!(
            event,
            RuntimeEvent::SessionStatus { status, .. } if status == "Stopped"
        )));
        Ok(())
    })
}
