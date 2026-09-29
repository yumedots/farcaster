use super::*;
use crate::agents::Backend;

fn identity(registry: &CallerRegistry, project: &Path, backend: Backend) -> CallerIdentity {
    registry.issue(
        project,
        CallerProfile {
            backend,
            provider: None,
            model: None,
            effort: None,
        },
    )
}

#[test]
fn process_metadata_identity_is_available_before_session_binding() {
    let registry = CallerRegistry::default();
    let caller = identity(&registry, Path::new("/project"), Backend::Pi);
    let (id, name) = caller.worker_identity().expect("launch identity");
    assert!(id.starts_with("worker-"));
    assert_ne!(id, caller.token());
    assert!(!name.is_empty());
    caller.bind("native-session");
    let context = context(&registry, &caller);
    assert_eq!((id, name), (context.worker_id, context.worker_name));
}

#[test]
fn transient_identity_keeps_its_locator_without_persisting_a_session() {
    let registry = CallerRegistry::default();
    let registrations = Arc::new(Mutex::new(0));
    let captured = registrations.clone();
    registry.set_execution_sinks(
        Some(Arc::new(move |_| {
            *captured.lock().expect("registrations") += 1;
            Ok(42)
        })),
        None,
    );

    let caller =
        identity(&registry, Path::new("/project"), Backend::Codex).without_session_persistence();
    caller.bind("ephemeral-title-thread");
    caller.begin_execution(Some("title"));

    assert_eq!(
        context(&registry, &caller).session,
        "ephemeral-title-thread"
    );
    assert_eq!(*registrations.lock().expect("registrations"), 0);
}

fn context(registry: &CallerRegistry, identity: &CallerIdentity) -> CallerContext {
    registry
        .resolve(identity.token())
        .expect("registered caller")
}

#[test]
fn execution_binding_is_captured_before_later_turns_and_cleared_on_rebind() {
    let registry = CallerRegistry::default();
    let turns = Arc::new(Mutex::new(Vec::new()));
    let captured = turns.clone();
    registry.set_execution_sinks(
        Some(Arc::new(|_| Ok(42))),
        Some(Arc::new(move |turn| {
            captured.lock().expect("turns").push(turn.clone());
            Ok(())
        })),
    );
    let identity = identity(&registry, Path::new("/project"), Backend::Cursor);
    identity.bind("native");
    identity.begin_execution(Some("first"));
    let (_, first) = registry
        .resolve_execution(identity.token())
        .expect("first execution");
    identity.set_activity(WorkerActivityState::Working);
    assert_eq!(
        registry
            .resolve_execution(identity.token())
            .expect("unchanged")
            .1,
        first
    );
    identity.begin_execution(Some("first"));
    assert_eq!(
        turns.lock().expect("turns").len(),
        1,
        "receipt replay cannot create another turn"
    );
    identity.begin_execution(Some("second"));
    assert_eq!(first.prompt_id.as_deref(), Some("first"));
    assert_eq!(
        registry
            .resolve_execution(identity.token())
            .expect("second")
            .1
            .prompt_id
            .as_deref(),
        Some("second")
    );
    identity.bind("another-session");
    assert!(registry.resolve_execution(identity.token()).is_err());
}

fn child(
    registry: &CallerRegistry,
    parent: &CallerContext,
    name: &str,
) -> Result<CallerIdentity, String> {
    registry.issue_as(
        &parent.project,
        CallerProfile {
            backend: parent.backend,
            provider: None,
            model: None,
            effort: None,
        },
        new_worker_id(),
        name.into(),
        Some(parent.worker_id.clone()),
    )
}

#[test]
fn top_level_workers_receive_distinct_human_names() {
    let registry = CallerRegistry::default();
    let first = identity(&registry, Path::new("/project"), Backend::Pi);
    let second = identity(&registry, Path::new("/project"), Backend::Pi);
    first.bind("session-1");
    second.bind("session-2");

    let first = context(&registry, &first);
    let second = context(&registry, &second);
    assert_ne!(first.worker_name, second.worker_name);
    assert!(crate::agents::valid_worker_name(&first.worker_name));
    assert!(!first.worker_name.starts_with("worker-"));
}

#[test]
fn child_names_are_valid_and_unique_within_the_parent() -> Result<(), String> {
    let registry = CallerRegistry::default();
    let first_parent = identity(&registry, Path::new("/project"), Backend::Pi);
    let second_parent = identity(&registry, Path::new("/project"), Backend::Pi);
    first_parent.bind("first-parent");
    second_parent.bind("second-parent");
    let first = context(&registry, &first_parent);
    let second = context(&registry, &second_parent);

    let _first_child = child(&registry, &first, "review")?;
    assert!(child(&registry, &first, "REVIEW").is_err());
    assert!(child(&registry, &first, "bad name").is_err());
    assert!(child(&registry, &second, "review").is_ok());
    Ok(())
}
