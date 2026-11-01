use super::{Config, Engine, Error, Result, Snapshot, State, Status};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
    },
    thread,
    time::Duration,
};

#[derive(Default)]
struct Shared {
    snapshot: Snapshot,
    engine: Option<Engine>,
    pending_logouts: usize,
}

enum Command {
    Shutdown(SyncSender<()>),
    Connect {
        config: Config,
        root: PathBuf,
        namespace: String,
        generation: u64,
    },
    Disconnect {
        done: Option<SyncSender<Result<()>>>,
        logout: bool,
        generation: u64,
    },
}

/// Cloneable UI handle. Public methods only exchange bounded messages and
/// prepared state; engine/keyring/process work runs on the owned worker.
#[derive(Clone)]
pub struct Manager {
    commands: SyncSender<Command>,
    shared: Arc<Mutex<Shared>>,
    generation: Arc<AtomicU64>,
}

impl Manager {
    pub fn new() -> Result<Self> {
        let (commands, incoming) = mpsc::sync_channel(64);
        let shared = Arc::new(Mutex::new(Shared::default()));
        let generation = Arc::new(AtomicU64::new(0));
        let state = shared.clone();
        let epoch = generation.clone();
        thread::Builder::new()
            .name("tailscale-network".into())
            .spawn(move || run(incoming, state, epoch))?;
        Ok(Self {
            commands,
            shared,
            generation,
        })
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub fn snapshot(&self) -> Snapshot {
        let shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        let mut snapshot = shared.snapshot.clone();
        if shared.pending_logouts > 0 {
            snapshot.status.state = State::Disconnecting;
        }
        snapshot.generation = self.generation.load(Ordering::Acquire);
        snapshot
    }

    /// Preflight for a settings save. May read private identity metadata;
    /// callers run it off the UI thread before persisting a changed server/store.
    pub fn validate_transition(
        &self,
        previous: &Config,
        next: &Config,
        root: &std::path::Path,
        namespace: &str,
    ) -> Result<()> {
        next.validate()?;
        let previous = (previous.clone(), root.to_path_buf(), namespace.to_owned());
        let next = (next.clone(), root.to_path_buf(), namespace.to_owned());
        identity_transition(Some(&previous), &next, self.engine().ok().as_ref())
    }

    /// Blocking shutdown, for the application quit worker, never the UI.
    pub fn shutdown(&self) -> Result<()> {
        let (done, finished) = mpsc::sync_channel(1);
        self.commands
            .send(Command::Shutdown(done))
            .map_err(|_| Error::Worker)?;
        finished
            .recv_timeout(Duration::from_secs(15))
            .map_err(|_| Error::Timeout)
    }

    pub fn connect(&self, config: Config, root: PathBuf, namespace: String) -> Result<()> {
        config.validate()?;
        self.send(move |generation| Command::Connect {
            config,
            root,
            namespace,
            generation,
        })
    }

    pub fn disconnect(&self, logout: bool) -> Result<()> {
        self.send(move |generation| Command::Disconnect {
            done: None,
            logout,
            generation,
        })
    }

    /// Wait for ownership-checked completion before the app persists a disconnect.
    /// This blocks and is only used by the settings/quit background workers.
    pub fn disconnect_wait(&self, logout: bool) -> Result<()> {
        let (done, completed) = mpsc::sync_channel(1);
        self.send(move |generation| Command::Disconnect {
            done: Some(done),
            logout,
            generation,
        })?;
        completed
            .recv_timeout(Duration::from_secs(15))
            .map_err(|_| Error::Timeout)?
    }

    /// Proxy workers alone may borrow an engine. It never reaches GPUI.
    pub fn engine(&self) -> Result<Engine> {
        self.shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .engine
            .clone()
            .ok_or(Error::NotReady)
    }

    fn send(&self, build: impl FnOnce(u64) -> Command) -> Result<()> {
        // Serialize enqueue and epoch publication with the worker's epoch check.
        let mut shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        let generation = self.generation.load(Ordering::Acquire).wrapping_add(1);
        let command = build(generation);
        let pending = match &command {
            Command::Connect { config, .. } if config.enabled => State::Connecting,
            Command::Disconnect { .. } => State::Disconnecting,
            _ => State::Disconnected,
        };
        let logout = matches!(command, Command::Disconnect { logout: true, .. });
        self.commands
            .try_send(command)
            .map_err(|error| match error {
                TrySendError::Full(_) => Error::Busy,
                TrySendError::Disconnected(_) => Error::Worker,
            })?;
        if logout {
            shared.pending_logouts += 1;
        }
        self.generation.store(generation, Ordering::Release);
        shared.engine = None;
        shared.snapshot = Snapshot {
            status: Status {
                state: pending,
                ..Status::default()
            },
            ..Snapshot::default()
        };
        Ok(())
    }
}

fn identity_transition(
    previous: Option<&Identity>,
    next: &Identity,
    active: Option<&Engine>,
) -> Result<()> {
    let Some(previous) = previous else {
        return Ok(());
    };
    if previous.0.control_url == next.0.control_url
        && previous.0.allow_plaintext_credentials == next.0.allow_plaintext_credentials
    {
        return Ok(());
    }
    let stored = match active {
        Some(_) => return Err(Error::IdentityChange),
        None => Engine::new(&previous.0, &previous.1, &previous.2)?.has_identity()?,
    };
    if stored {
        Err(Error::IdentityChange)
    } else {
        Ok(())
    }
}

type Identity = (Config, PathBuf, String);

fn run(incoming: Receiver<Command>, shared: Arc<Mutex<Shared>>, desired: Arc<AtomicU64>) {
    let mut active: Option<Engine> = None;
    let mut active_identity: Option<Identity> = None;
    let mut generation = 0;
    let mut last: Option<Identity> = None;
    let mut logout_failed = false;
    loop {
        match incoming.recv_timeout(Duration::from_millis(150)) {
            Ok(Command::Shutdown(done)) => {
                shared.lock().unwrap_or_else(PoisonError::into_inner).engine = None;
                if let Some(engine) = active.take() {
                    let _ = engine.disconnect();
                }
                let _ = done.send(());
                return;
            }
            Ok(command) => {
                let completion = match &command {
                    Command::Disconnect { done, .. } => done.clone(),
                    _ => None,
                };
                let epoch = match &command {
                    Command::Connect { generation, .. }
                    | Command::Disconnect { generation, .. } => *generation,
                    Command::Shutdown(_) => continue,
                };
                {
                    let mut state = shared.lock().unwrap_or_else(PoisonError::into_inner);
                    if epoch != desired.load(Ordering::Acquire)
                        && !matches!(
                            command,
                            Command::Disconnect { logout: true, .. }
                                | Command::Disconnect { done: Some(_), .. }
                        )
                    {
                        // Even a canceled start identifies what a subsequent ordered logout deletes.
                        if !logout_failed
                            && let Command::Connect {
                                config,
                                root,
                                namespace,
                                ..
                            } = command
                        {
                            let next = (config, root, namespace);
                            if identity_transition(last.as_ref(), &next, active.as_ref()).is_ok() {
                                last = Some(next);
                            }
                        }
                        continue;
                    }
                    state.engine = None;
                }
                let result = match command {
                    Command::Connect {
                        config,
                        root,
                        namespace,
                        ..
                    } => {
                        let next = (config.clone(), root.clone(), namespace.clone());
                        if logout_failed {
                            Err(Error::LogoutPending)
                        } else if let Err(error) =
                            identity_transition(last.as_ref(), &next, active.as_ref())
                        {
                            Err(error)
                        } else {
                            if let Some(old) = active.take() {
                                let _ = old.disconnect();
                            }
                            active_identity = None;
                            last = Some((config.clone(), root.clone(), namespace.clone()));
                            if config.enabled {
                                Engine::new(&config, &root, &namespace).and_then(|engine| {
                                    engine.start()?;
                                    active = Some(engine);
                                    active_identity = last.clone();
                                    Ok(())
                                })
                            } else {
                                Ok(())
                            }
                        }
                    }
                    Command::Disconnect { logout, .. } => {
                        let old = active.take();
                        let same_identity = active_identity.take() == last;
                        let result = if logout && same_identity && old.is_some() {
                            old.map_or(Ok(()), |engine| engine.logout())
                        } else {
                            old.map_or(Ok(()), |engine| engine.disconnect())
                                .and_then(|()| {
                                    if let Some((config, root, namespace)) = &last {
                                        let engine = Engine::new(config, root, namespace)?;
                                        if logout { engine.logout() } else { Ok(()) }
                                    } else {
                                        Ok(())
                                    }
                                })
                        };
                        if logout {
                            logout_failed = result.is_err();
                            let mut state = shared.lock().unwrap_or_else(PoisonError::into_inner);
                            state.pending_logouts = state.pending_logouts.saturating_sub(1);
                        }
                        result
                    }
                    Command::Shutdown(_) => continue,
                };
                generation = epoch;
                let mut state = shared.lock().unwrap_or_else(PoisonError::into_inner);
                if desired.load(Ordering::Acquire) == generation {
                    match &result {
                        Ok(()) => {
                            state.engine = active.clone();
                            state.snapshot = Snapshot::default();
                        }
                        Err(error) => state.snapshot = failed(error),
                    }
                }
                drop(state);
                if let Some(completion) = completion {
                    let _ = completion.send(result);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if let Some(engine) = &active {
            let result = engine.snapshot();
            let stop = match &result {
                Ok(snapshot) => snapshot.status.state == State::Failed,
                Err(_) => true,
            };
            let snapshot = result.unwrap_or_else(failed);
            if stop {
                if let Some(engine) = active.take() {
                    let _ = engine.disconnect();
                }
                active_identity = None;
            }
            let mut state = shared.lock().unwrap_or_else(PoisonError::into_inner);
            if desired.load(Ordering::Acquire) == generation {
                state.engine = active.clone();
                state.snapshot = snapshot;
            }
        }
    }
    shared.lock().unwrap_or_else(PoisonError::into_inner).engine = None;
    if let Some(engine) = active {
        let _ = engine.disconnect();
    }
}

fn failed(error: impl std::fmt::Display) -> Snapshot {
    Snapshot {
        status: Status {
            state: State::Failed,
            error: Some(error.to_string()),
            ..Status::default()
        },
        ..Snapshot::default()
    }
}

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;
