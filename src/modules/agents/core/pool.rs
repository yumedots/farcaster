use crate::agents::Backend;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use super::worker::{WorkerLaunch, WorkerSessionFactory};
use crate::modules::agents::contract::{WorkerContext, WorkerSnapshot, WorkerStatus};

#[derive(Clone)]
pub(crate) struct WorkerPool {
    inner: Arc<PoolInner>,
}

struct PoolInner {
    factories: BTreeMap<Backend, Arc<dyn WorkerSessionFactory>>,
    app_proxy: Mutex<Option<String>>,
    updates: async_channel::Sender<()>,
    update_receiver: async_channel::Receiver<()>,
    state: Mutex<PoolState>,
}

#[derive(Default)]
struct PoolState {
    sequence: u64,
    records: BTreeMap<String, WorkerRecord>,
    stopping_families: BTreeSet<(PathBuf, Backend, String)>,
}

struct WorkerRecord {
    snapshot: Arc<Mutex<WorkerSnapshot>>,
    launch: WorkerLaunch,
    parent_backend: Option<Backend>,
}

impl WorkerPool {
    pub(crate) fn new(
        factories: BTreeMap<Backend, Arc<dyn WorkerSessionFactory>>,
    ) -> Result<Self, String> {
        let (updates, update_receiver) = async_channel::unbounded();
        Ok(Self {
            inner: Arc::new(PoolInner {
                factories,
                app_proxy: Mutex::new(None),
                updates,
                update_receiver,
                state: Mutex::new(PoolState::default()),
            }),
        })
    }

    pub(crate) fn updates(&self) -> async_channel::Receiver<()> {
        self.inner.update_receiver.clone()
    }

    pub(crate) fn restore_families(
        &self,
        families: impl IntoIterator<Item = super::WorkerFamilyLink>,
    ) -> Result<(), String> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| "worker pool state is unavailable".to_owned())?;
        for family in families {
            let Some(routing) = family.routing else {
                continue;
            };
            if !crate::agents::valid_worker_name(&routing.name)
                || family.child_session.trim().is_empty()
                || family.parent_session.trim().is_empty()
                || routing.assignment.execution.harness != family.child_backend
            {
                zlog::warn!("Ignore invalid saved worker route for {}", routing.name);
                continue;
            }
            let project = match canonical_directory(&family.project) {
                Ok(project) => project,
                Err(error) => {
                    zlog::warn!("Ignore saved worker route {}: {error}", routing.name);
                    continue;
                }
            };
            if state.records.values().any(|record| {
                record.launch.project == project
                    && record.launch.parent_session == family.parent_session
                    && record.parent_backend == Some(family.parent_backend)
                    && record
                        .launch
                        .worker_name
                        .eq_ignore_ascii_case(&routing.name)
            }) {
                return Err(format!(
                    "ambiguous saved worker route for child {}",
                    routing.name
                ));
            }
            if !self.inner.factories.contains_key(&family.child_backend) {
                zlog::warn!(
                    "Ignore saved worker route {} for unavailable backend {}",
                    routing.name,
                    family.child_backend
                );
                continue;
            }
            state.sequence = state.sequence.saturating_add(1);
            let id = worker_id(state.sequence)?;
            let snapshot = WorkerSnapshot {
                id: id.clone(),
                backend: family.child_backend,
                project: project.clone(),
                session_locator: Some(family.child_session.clone()),
                status: WorkerStatus::Idle,
                output: None,
                error: None,
                pending_input: None,
            };
            state.records.insert(
                id.clone(),
                WorkerRecord {
                    snapshot: Arc::new(Mutex::new(snapshot)),
                    launch: WorkerLaunch {
                        worker_id: id,
                        worker_name: routing.name,
                        project,
                        parent_session: family.parent_session,
                        parent_worker_id: None,
                        context: WorkerContext::Resume {
                            session_locator: family.child_session,
                        },
                        provider: Some(routing.assignment.execution.provider.clone()),
                        model: Some(routing.assignment.execution.model.clone()),
                        effort: routing.assignment.execution.effort.clone(),
                        access_mode: routing.access_mode,
                        app_proxy: None,
                        ephemeral: false,
                    },
                    parent_backend: Some(family.parent_backend),
                },
            );
        }
        Ok(())
    }

    pub(crate) fn set_app_proxy(&self, proxy: Option<String>) -> Result<(), String> {
        *self
            .inner
            .app_proxy
            .lock()
            .map_err(|_| "worker proxy configuration is unavailable".to_owned())? = proxy;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn fence_family(&self, parent: &super::caller::CallerContext) -> Result<(), String> {
        self.inner
            .state
            .lock()
            .map_err(|_| "worker pool state is unavailable".to_owned())?
            .stopping_families
            .insert((
                parent.project.clone(),
                parent.backend,
                parent.session.clone(),
            ));
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn app_proxy(&self) -> Result<Option<String>, String> {
        self.inner
            .app_proxy
            .lock()
            .map(|proxy| proxy.clone())
            .map_err(|_| "worker proxy configuration is unavailable".to_owned())
    }

    pub(crate) fn snapshots(&self) -> Result<Vec<WorkerSnapshot>, String> {
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| "worker pool state is unavailable".to_owned())?;
        state.records.values().map(snapshot).collect()
    }

    pub(crate) fn stop_session_family(
        &self,
        project: &Path,
        sessions: &[(Backend, PathBuf)],
    ) -> Result<usize, String> {
        let project = canonical_directory(project)?;
        let mut sessions = sessions
            .iter()
            .map(|(backend, path)| (*backend, path.to_string_lossy().into_owned()))
            .collect::<BTreeSet<_>>();
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| "worker pool state is unavailable".to_owned())?;
        expand_family_sessions(&state, &project, &mut sessions);
        let family_keys = sessions
            .iter()
            .map(|(backend, locator)| (project.clone(), *backend, locator.clone()))
            .collect::<BTreeSet<_>>();
        state.stopping_families.extend(family_keys);
        let ids = state
            .records
            .iter()
            .filter_map(|(id, record)| {
                let current = snapshot(record).ok()?;
                (current.project == project
                    && (record.parent_backend.as_ref().map_or_else(
                        || {
                            sessions
                                .iter()
                                .any(|(_, locator)| locator == &record.launch.parent_session)
                        },
                        |backend| {
                            sessions.contains(&(*backend, record.launch.parent_session.clone()))
                        },
                    ) || current.session_locator.as_ref().is_some_and(|locator| {
                        sessions.contains(&(current.backend, locator.clone()))
                    })))
                .then(|| id.clone())
            })
            .collect::<Vec<_>>();
        for id in &ids {
            let Some(record) = state.records.get_mut(id) else {
                continue;
            };
            if let Ok(mut current) = record.snapshot.lock() {
                current.status = WorkerStatus::Stopped;
                current.pending_input = None;
            }
        }
        if !ids.is_empty() {
            notify(&self.inner.updates);
        }
        Ok(ids.len())
    }

    pub(crate) fn finish_session_family_stop(
        &self,
        project: &Path,
        sessions: &[(Backend, PathBuf)],
    ) -> Result<(), String> {
        let project = canonical_directory(project)?;
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| "worker pool state is unavailable".to_owned())?;
        let mut sessions = sessions
            .iter()
            .map(|(backend, locator)| (*backend, locator.to_string_lossy().into_owned()))
            .collect::<BTreeSet<_>>();
        expand_family_sessions(&state, &project, &mut sessions);
        for (backend, locator) in sessions {
            state
                .stopping_families
                .remove(&(project.clone(), backend, locator));
        }
        Ok(())
    }
}

fn canonical_directory(path: &Path) -> Result<PathBuf, String> {
    let path = path
        .canonicalize()
        .map_err(|error| format!("resolve worker project {}: {error}", path.display()))?;
    if !path.is_dir() {
        return Err(format!(
            "worker project is not a directory: {}",
            path.display()
        ));
    }
    Ok(path)
}

fn worker_id(sequence: u64) -> Result<String, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is unavailable".to_owned())?
        .as_nanos();
    Ok(format!("worker-{nanos}-{sequence}"))
}

fn expand_family_sessions(
    state: &PoolState,
    project: &Path,
    sessions: &mut BTreeSet<(Backend, String)>,
) {
    loop {
        let descendants = state
            .records
            .values()
            .filter_map(|record| {
                let current = snapshot(record).ok()?;
                (current.project == project
                    && record.parent_backend.as_ref().is_some_and(|backend| {
                        sessions.contains(&(*backend, record.launch.parent_session.clone()))
                    }))
                .then(|| {
                    current
                        .session_locator
                        .map(|locator| (current.backend, locator))
                })
                .flatten()
            })
            .collect::<Vec<_>>();
        let before = sessions.len();
        sessions.extend(descendants);
        if sessions.len() == before {
            break;
        }
    }
}

fn notify(updates: &async_channel::Sender<()>) {
    let _ = updates.try_send(());
}

fn snapshot(record: &WorkerRecord) -> Result<WorkerSnapshot, String> {
    record
        .snapshot
        .lock()
        .map(|snapshot| snapshot.clone())
        .map_err(|_| "worker state is unavailable".to_owned())
}
