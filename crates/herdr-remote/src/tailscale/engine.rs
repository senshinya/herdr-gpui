use super::{Config, Error, Result, Snapshot, credentials::Storage, ffi::Native};
use serde::Serialize;
use std::{
    fs::{self, File, TryLockError},
    io::{self, Read, Write},
    path::Path,
    sync::{Arc, Mutex, PoisonError},
};

#[derive(Debug)]
struct Owned {
    // Dropped before the lock, so no second process can use the identity early.
    native: Native,
    lock: Mutex<Option<IdentityLock>>,
    identity: String,
}

#[derive(Debug)]
struct IdentityLock(File);

impl Drop for IdentityLock {
    fn drop(&mut self) {
        // Closing this descriptor alone leaves a flock held by any child that
        // inherited its open file description before CLOEXEC took effect.
        let _ = self.0.unlock();
    }
}

/// An in-process network handle. Creating, starting, snapshotting and dropping
/// it may block; application code uses Manager and proxy workers, never GPUI.
#[derive(Clone, Debug)]
pub struct Engine(Arc<Owned>);

impl Engine {
    pub fn new(config: &Config, root: &Path, namespace: &str) -> Result<Self> {
        config.validate()?;
        private_directory(root)?;
        let root = fs::canonicalize(root)?;
        let identity = super::credentials::fingerprint(format!(
            "{root:?}\0{namespace}\0{}",
            config.control_url
        ));
        let directory = root.join(&identity);
        private_directory(&directory)?;
        let path = directory.join("identity.lock");
        #[cfg(unix)]
        let lock = File::from(
            rustix::fs::open(
                &path,
                rustix::fs::OFlags::CREATE
                    | rustix::fs::OFlags::RDWR
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            )
            .map_err(|source| Error::Path {
                path: path.clone(),
                source: source.into(),
            })?,
        );
        #[cfg(not(unix))]
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|source| Error::Path {
                path: path.clone(),
                source,
            })?;
        lock.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => Error::Busy,
            TryLockError::Error(source) => Error::Path { path, source },
        })?;
        let lock = IdentityLock(lock);
        #[derive(Serialize)]
        struct NativeConfig<'a> {
            directory: &'a str,
            hostname: &'a str,
            control_url: &'a str,
        }
        let native_config = serde_json::to_vec(&NativeConfig {
            directory: directory.to_str().ok_or(Error::PathEncoding)?,
            hostname: &config.hostname,
            control_url: &config.control_url,
        })?;
        let storage = Storage::new(config, directory, identity.clone());
        let native = Native::new(&native_config, storage)?;
        Ok(Self(Arc::new(Owned {
            native,
            lock: Mutex::new(Some(lock)),
            identity,
        })))
    }

    pub fn identity(&self) -> &str {
        &self.0.identity
    }
    pub fn start(&self) -> Result<()> {
        let lock = self.0.lock.lock().unwrap_or_else(PoisonError::into_inner);
        if lock.is_none() {
            return Err(Error::Closed);
        }
        self.0.native.command(1)
    }
    pub fn disconnect(&self) -> Result<()> {
        let mut lock = self.0.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(owned) = lock.as_ref() else {
            return Err(Error::Closed);
        };
        self.0.native.command(2)?;
        owned.0.unlock()?;
        lock.take();
        Ok(())
    }
    pub fn logout(&self) -> Result<()> {
        // Ownership covers shutdown and credential deletion as one operation.
        // An old Stream may retain this handle after a new owner reconnects.
        let mut lock = self.0.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(owned) = lock.as_ref() else {
            return Err(Error::Closed);
        };
        let result = self.0.native.command(3);
        self.0.native.command(2)?;
        self.0.native.clear_identity()?;
        owned.0.unlock()?;
        lock.take();
        match result {
            Err(Error::NotReady | Error::Closed) => Ok(()),
            result => result,
        }
    }
    pub(super) fn has_identity(&self) -> Result<bool> {
        self.0.native.has_identity()
    }
    pub fn snapshot(&self) -> Result<Snapshot> {
        self.0.native.snapshot()
    }
    pub fn dial(&self, peer: &str, port: u16) -> Result<Stream> {
        let id = self.0.native.dial(peer, port)?;
        Ok(Stream(Arc::new(OpenStream {
            engine: self.clone(),
            id,
        })))
    }
}

pub(super) fn private_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt};
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)?;
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_dir()
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o077 != 0
        {
            return Err(herdr_credentials::Error::Permissions.into());
        }
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(path)?;
        if !fs::symlink_metadata(path)?.is_dir() {
            return Err(herdr_credentials::Error::Permissions.into());
        }
    }
    Ok(())
}

#[derive(Debug)]
struct OpenStream {
    engine: Engine,
    id: u64,
}
impl Drop for OpenStream {
    fn drop(&mut self) {
        self.engine.0.native.close_stream(self.id);
    }
}

/// Clone for one reader and one writer on proxy worker threads.
#[derive(Clone, Debug)]
pub struct Stream(Arc<OpenStream>);
impl Read for Stream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.0.engine.0.native.read(self.0.id, bytes)
    }
}
impl Write for Stream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.engine.0.native.write(self.0.id, bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
