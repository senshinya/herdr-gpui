//! Process-owned embedded Tailnet runtime. UI reads prepared snapshots only.
mod error;
mod helper;
use crate::{Result, config::Config};
pub(crate) use error::Error;
pub(crate) use helper::run as run_helper;
use herdr_client::tailnet::{Binding, Route};
use herdr_remote::tailscale::{Manager, Snapshot, State, proxy::Proxy};
use std::{
    path::PathBuf,
    sync::{Mutex, OnceLock, PoisonError},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Selection {
    pub generation: u64,
    pub network: String,
    pub peer: herdr_remote::tailscale::Peer,
}

struct Runtime {
    manager: Manager,
    proxy: Proxy,
    config: Mutex<herdr_remote::tailscale::Config>,
    root: PathBuf,
    namespace: String,
    operation: Mutex<()>,
}
static RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// Runs before the UI loop. No identity is opened until enabled by the user.
pub(crate) fn initialize(config: &Config, development: bool) -> Result<()> {
    if RUNTIME.get().is_some() {
        return Ok(());
    }
    let root = herdr_client::local_path::state_dir()
        .ok_or(Error::Unavailable)?
        .join(if development {
            "tailscale-dev"
        } else {
            "tailscale"
        });
    let manager = Manager::new().map_err(Error::from)?;
    let proxy = Proxy::bind(manager.clone()).map_err(Error::from)?;
    let namespace = if development {
        "development"
    } else {
        "release"
    }
    .to_owned();
    let runtime = Runtime {
        manager,
        proxy,
        config: Mutex::new(config.tailscale.clone()),
        root,
        namespace,
        operation: Mutex::new(()),
    };
    runtime
        .manager
        .connect(
            config.tailscale.clone(),
            runtime.root.clone(),
            runtime.namespace.clone(),
        )
        .map_err(Error::from)?;
    for device in config.devices.values() {
        if let Some(binding) = &device.tailnet
            && let Err(error) = install_route(&runtime, binding, false)
        {
            tracing::warn!(%error, "Could not restore an embedded Tailnet route");
        }
    }
    let _ = RUNTIME.set(runtime);
    Ok(())
}

pub(crate) fn install(cx: &mut gpui::App) {
    cx.on_app_quit(|cx| {
        cx.background_executor().spawn(async {
            if let Some(runtime) = RUNTIME.get() {
                let _operation = runtime
                    .operation
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                if let Err(error) = runtime.manager.shutdown() {
                    tracing::warn!(%error, "Embedded Tailscale shutdown did not finish");
                }
                if let Err(error) = runtime.proxy.shutdown() {
                    tracing::warn!(%error, "Embedded Tailscale proxy cleanup did not finish");
                }
            }
        })
    })
    .detach();
}

pub(crate) fn snapshot() -> Snapshot {
    RUNTIME
        .get()
        .map_or_else(Snapshot::default, |runtime| runtime.manager.snapshot())
}

/// Config watchers enqueue changes once, regardless of how many windows reload.
/// This only touches cached state and the manager's bounded command queue.
pub(crate) fn refresh(config: &herdr_remote::tailscale::Config) {
    let Some(runtime) = RUNTIME.get() else {
        return;
    };
    let Ok(_operation) = runtime.operation.try_lock() else {
        return;
    };
    let mut current = runtime
        .config
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if *current == *config {
        return;
    }
    if current.control_url != config.control_url
        || current.allow_plaintext_credentials != config.allow_plaintext_credentials
    {
        tracing::warn!(
            "Changed Tailscale server/store requires applying from Settings after sign out"
        );
        return;
    }
    match runtime.manager.connect(
        config.clone(),
        runtime.root.clone(),
        runtime.namespace.clone(),
    ) {
        Ok(()) => *current = config.clone(),
        Err(error) => tracing::warn!(%error, "Could not apply embedded Tailscale configuration"),
    }
}

/// Background only: disk persistence, keyring policy and SSH config resolution.
pub(crate) fn apply(config: herdr_remote::tailscale::Config) -> Result<()> {
    let runtime = RUNTIME.get().ok_or(Error::Unavailable)?;
    let _operation = runtime
        .operation
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let previous = runtime
        .config
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    runtime
        .manager
        .validate_transition(&previous, &config, &runtime.root, &runtime.namespace)
        .map_err(Error::from)?;
    Config::save_tailscale(&config)?;
    runtime
        .manager
        .connect(
            config.clone(),
            runtime.root.clone(),
            runtime.namespace.clone(),
        )
        .map_err(Error::from)?;
    *runtime
        .config
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = config;
    Ok(())
}

/// Background only, after confirmation in Settings.
pub(crate) fn disconnect(logout: bool) -> Result<()> {
    let runtime = RUNTIME.get().ok_or(Error::Unavailable)?;
    let _operation = runtime
        .operation
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let mut config = runtime
        .config
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    config.enabled = false;
    runtime
        .manager
        .disconnect_wait(logout)
        .map_err(Error::from)?;
    Config::save_tailscale(&config)?;
    *runtime
        .config
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = config;
    Ok(())
}

/// Called by an Add Device worker after the user has selected a peer.
pub(crate) fn bind_peer(selection: &Selection, target: &str) -> Result<Binding> {
    let runtime = RUNTIME.get().ok_or(Error::Unavailable)?;
    let current = runtime.manager.snapshot();
    if selection.generation != current.generation || selection.network != current.status.network_id
    {
        return Err(Error::Native(herdr_remote::tailscale::Error::PeerUnavailable).into());
    }
    let peer = selection.peer.id.as_str();
    let engine = runtime.manager.engine().map_err(Error::from)?;
    let status = engine.snapshot().map_err(Error::from)?.status;
    if status.state != State::Connected
        || !status.peers.iter().any(|p| p.id == peer && p.available())
    {
        return Err(Error::Native(herdr_remote::tailscale::Error::PeerUnavailable).into());
    }
    let binding = Binding::new(
        engine.identity().into(),
        status.network_id,
        peer.into(),
        target,
    )?;
    if runtime.manager.generation() != selection.generation {
        return Err(Error::Native(herdr_remote::tailscale::Error::PeerUnavailable).into());
    }
    install_route(runtime, &binding, true)?;
    Ok(binding)
}

fn generation() -> u64 {
    RUNTIME
        .get()
        .map_or(0, |runtime| runtime.manager.generation())
}

fn install_route(runtime: &Runtime, binding: &Binding, resolve: bool) -> Result<()> {
    let prepare = if resolve {
        Route::prepare
    } else {
        Route::deferred
    };
    let route = prepare(
        binding.clone(),
        runtime.proxy.endpoint().to_path_buf(),
        std::env::current_exe()?,
        runtime.proxy.directory(),
    )?;
    runtime.proxy.allow(binding).map_err(Error::from)?;
    herdr_client::tailnet::install(route.with_generation(generation))?;
    Ok(())
}

#[cfg(test)]
mod tests;
