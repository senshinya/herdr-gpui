//! A private data endpoint for OpenSSH ProxyCommand children. Terminal bytes
//! never enter the app's JSON control queue or its UI thread.
use super::{Error, Manager, Result};
use herdr_client::{
    private_endpoint::{self, Connection, Listener},
    tailnet::{Binding, ProxyRequest},
};
use std::{
    collections::BTreeSet,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, PoisonError, RwLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

#[derive(Clone)]
pub struct Proxy(Arc<Owned>);
struct Owned {
    allowed: Arc<RwLock<BTreeSet<ProxyRequest>>>,
    directory: PathBuf,
    endpoint: PathBuf,
    finished: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
}

impl Proxy {
    pub fn bind(manager: Manager) -> Result<Self> {
        #[cfg(unix)]
        let directory = tempfile::Builder::new()
            .prefix("herdr-ts-")
            .tempdir_in("/tmp")?;
        #[cfg(not(unix))]
        let directory = tempfile::Builder::new().prefix("herdr-ts-").tempdir()?;
        #[cfg(unix)]
        let endpoint = directory.path().join("p");
        #[cfg(windows)]
        let endpoint = PathBuf::from(format!(
            r"\\.\pipe\herdr-tailnet-{}-{}-{}",
            private_endpoint::user_sid()?,
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let listener = Listener::bind(&endpoint)?;
        let state = Arc::new(Owned {
            allowed: Arc::new(RwLock::new(BTreeSet::new())),
            directory: directory.path().to_path_buf(),
            endpoint: endpoint.clone(),
            finished: Arc::new(AtomicBool::new(false)),
            stop: Arc::new(AtomicBool::new(false)),
        });
        let stop = state.stop.clone();
        let finished = state.finished.clone();
        let allowed = state.allowed.clone();
        thread::Builder::new()
            .name("tailscale-proxy".into())
            .spawn(move || {
                let active = Arc::new(AtomicUsize::new(0));
                while let Ok(connection) = listener.accept() {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    if active.fetch_add(1, Ordering::AcqRel) >= 64 {
                        active.fetch_sub(1, Ordering::AcqRel);
                        continue;
                    }
                    let (manager, allowed) = (manager.clone(), allowed.clone());
                    let active_guard = Active(active.clone());
                    let stopped = stop.clone();
                    let spawned = thread::Builder::new()
                        .name("tailscale-stream".into())
                        .spawn(move || {
                            let _active = active_guard;
                            let _ = serve(connection, &manager, &allowed, &stopped);
                        });
                    let _ = spawned;
                }
                while active.load(Ordering::Acquire) != 0 {
                    thread::sleep(Duration::from_millis(10));
                }
                private_endpoint::release(&endpoint);
                drop(directory);
                finished.store(true, Ordering::Release);
            })?;
        Ok(Self(state))
    }

    /// Blocking cleanup for the app's quit worker; Drop never joins on GPUI.
    pub fn shutdown(&self) -> Result<()> {
        self.0.stop.store(true, Ordering::Release);
        if self.0.finished.load(Ordering::Acquire) {
            return Ok(());
        }
        let _ = Connection::connect(&self.0.endpoint, Duration::from_secs(1));
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while !self.0.finished.load(Ordering::Acquire) {
            if std::time::Instant::now() >= deadline {
                return Err(Error::Timeout);
            }
            thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    pub fn endpoint(&self) -> &Path {
        &self.0.endpoint
    }
    pub fn directory(&self) -> &Path {
        &self.0.directory
    }
    pub fn allow(&self, binding: &Binding) -> Result<()> {
        binding.validate().map_err(|_| Error::InvalidConfig)?;
        let mut allowed = self
            .0
            .allowed
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        let request = ProxyRequest::from(binding);
        if allowed.len() >= 256 && !allowed.contains(&request) {
            return Err(Error::Limit);
        }
        allowed.insert(request);
        Ok(())
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let path = self.endpoint.clone();
        let _ = thread::Builder::new()
            .name("tailscale-proxy-stop".into())
            .spawn(move || {
                let _ = Connection::connect(&path, Duration::from_secs(1));
            });
    }
}

struct Active(Arc<AtomicUsize>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

fn read_request(reader: &mut impl Read) -> io::Result<ProxyRequest> {
    let mut prefix = [0; 4];
    reader.read_exact(&mut prefix)?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > 2048 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut data = vec![0; length];
    reader.read_exact(&mut data)?;
    serde_json::from_slice(&data).map_err(|_| io::ErrorKind::InvalidData.into())
}

fn authorized(allowed: &BTreeSet<ProxyRequest>, request: &ProxyRequest, generation: u64) -> bool {
    if request.generation != generation {
        return false;
    }
    let mut permission = request.clone();
    permission.generation = 0; // Authorization binds a peer/port; epochs fence operations.
    allowed.contains(&permission)
}

fn serve(
    connection: Connection,
    manager: &Manager,
    allowed: &RwLock<BTreeSet<ProxyRequest>>,
    stopped: &AtomicBool,
) -> io::Result<()> {
    let (mut reader, mut writer, closer) = connection.split()?;
    reader.set_timeout(Some(Duration::from_secs(2)))?;
    writer.set_timeout(Some(Duration::from_millis(200)))?;
    let request = read_request(&mut reader)?;
    if !authorized(
        &allowed.read().unwrap_or_else(PoisonError::into_inner),
        &request,
        manager.generation(),
    ) {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let engine = manager.engine().map_err(io::Error::other)?;
    if engine.identity() != request.engine
        || engine
            .snapshot()
            .map_err(io::Error::other)?
            .status
            .network_id
            != request.network
    {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let mut outbound = engine
        .dial(&request.peer, request.port)
        .map_err(io::Error::other)?;
    if request.generation != manager.generation() {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    writer.write_all(&[1])?;
    writer.flush()?;
    reader.set_timeout(Some(Duration::from_millis(200)))?;
    let done = AtomicBool::new(false);
    thread::scope(|scope| {
        let mut inbound = outbound.clone();
        let closer_ref = &closer;
        let done_ref = &done;
        scope.spawn(move || {
            let _ = transfer(
                &mut inbound,
                &mut writer,
                done_ref,
                stopped,
                manager,
                request.generation,
            );
            done_ref.store(true, Ordering::Release);
            closer_ref.close();
        });
        let result = transfer(
            &mut reader,
            &mut outbound,
            &done,
            stopped,
            manager,
            request.generation,
        );
        done.store(true, Ordering::Release);
        closer.close();
        result
    })
}

fn transfer(
    reader: &mut impl Read,
    writer: &mut impl Write,
    done: &AtomicBool,
    stopped: &AtomicBool,
    manager: &Manager,
    generation: u64,
) -> io::Result<()> {
    let mut buffer = [0; 8192];
    while !done.load(Ordering::Acquire)
        && !stopped.load(Ordering::Acquire)
        && manager.generation() == generation
    {
        let length = match reader.read(&mut buffer) {
            Ok(0) => return Ok(()),
            Ok(length) => length,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut
                        | io::ErrorKind::WouldBlock
                        | io::ErrorKind::Interrupted
                ) =>
            {
                continue;
            }
            Err(error) => return Err(error),
        };
        let mut remaining = &buffer[..length];
        while !remaining.is_empty()
            && !done.load(Ordering::Acquire)
            && !stopped.load(Ordering::Acquire)
            && manager.generation() == generation
        {
            match writer.write(remaining) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(length) => remaining = &remaining[length..],
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::TimedOut
                            | io::ErrorKind::WouldBlock
                            | io::ErrorKind::Interrupted
                    ) =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            }
        }
    }
    Ok(())
}

/// Internal CLI entry: binary stdin/stdout only. Returning ends this helper
/// process; its stdin reader is never joined while the peer may be idle.
pub fn stdio(endpoint: &Path, request: &ProxyRequest) -> io::Result<()> {
    let connection = Connection::connect(endpoint, Duration::from_secs(2))?;
    let (mut reader, mut writer, closer) = connection.split()?;
    reader.set_timeout(Some(Duration::from_secs(12)))?;
    let data = serde_json::to_vec(request).map_err(io::Error::other)?;
    if data.len() > 2048 {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    writer.write_all(&(data.len() as u32).to_be_bytes())?;
    writer.write_all(&data)?;
    writer.flush()?;
    let mut ready = [0];
    reader.read_exact(&mut ready)?;
    if ready != [1] {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    reader.set_timeout(None)?;
    let stdin_close = closer.try_clone()?;
    thread::Builder::new()
        .name("tailscale-proxy-stdin".into())
        .spawn(move || {
            let _ = io::copy(&mut io::stdin().lock(), &mut writer);
            stdin_close.close();
        })?;
    // SSH packets are binary. A line-buffered stdout must not hold a packet
    // while the other peer waits for it before sending the next one.
    let result = copy_flushed(&mut reader, &mut io::stdout().lock());
    closer.close();
    result
}

fn copy_flushed(reader: &mut impl Read, writer: &mut impl Write) -> io::Result<()> {
    let mut buffer = [0; 8192];
    loop {
        let length = match reader.read(&mut buffer) {
            Ok(0) => return writer.flush(),
            Ok(length) => length,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        writer.write_all(&buffer[..length])?;
        writer.flush()?;
    }
}

#[cfg(test)]
mod tests;
