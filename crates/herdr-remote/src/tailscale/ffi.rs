//! All raw pointers live here. Go copies inputs; output buffers and storage
//! context are borrowed only for a call. Destroy joins Go before freeing Storage.
use super::{Error, Result, Snapshot, credentials::Storage};
use std::{
    io,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::PoisonError,
};
use zeroize::Zeroizing;

type StoreFn =
    unsafe extern "C" fn(usize, u32, *const u8, usize, *mut u8, usize, *mut usize) -> i32;

#[cfg_attr(windows, link(name = "herdr_tailscale", kind = "raw-dylib"))]
#[cfg_attr(not(windows), link(name = "herdr_tailscale", kind = "static"))]
unsafe extern "C" {
    fn herdr_ts_abi() -> u32;
    fn herdr_ts_create(
        data: *const u8,
        length: usize,
        callback: StoreFn,
        context: usize,
        result: *mut u64,
    ) -> i32;
    fn herdr_ts_command(id: u64, action: u32) -> i32;
    fn herdr_ts_destroy(id: u64);
    fn herdr_ts_snapshot(id: u64, output: *mut u8, capacity: usize, length: *mut usize) -> i32;
    fn herdr_ts_dial(id: u64, peer: *const u8, length: usize, port: u16, result: *mut u64) -> i32;
    fn herdr_ts_read(
        id: u64,
        stream: u64,
        output: *mut u8,
        capacity: usize,
        length: *mut usize,
    ) -> i32;
    fn herdr_ts_write(
        id: u64,
        stream: u64,
        data: *const u8,
        length: usize,
        written: *mut usize,
    ) -> i32;
    fn herdr_ts_close_stream(id: u64, stream: u64);
}

#[derive(Debug)]
pub(super) struct Native {
    id: u64,
    store: Box<Storage>,
}

impl Native {
    pub(super) fn new(config: &[u8], store: Storage) -> Result<Self> {
        // SAFETY: the linked library exports this fixed, no-argument ABI probe.
        if unsafe { herdr_ts_abi() } != 1 {
            return Err(Error::NativeVersion);
        }
        let store = Box::new(store);
        let context = std::ptr::from_ref(store.as_ref()) as usize;
        let mut id = 0;
        // SAFETY: config and result live through the call. Storage's Box address
        // stays fixed until destroy has stopped all callbacks in Drop.
        check(unsafe {
            herdr_ts_create(
                config.as_ptr(),
                config.len(),
                storage_callback,
                context,
                &mut id,
            )
        })?;
        Ok(Self { id, store })
    }

    pub(super) fn command(&self, action: u32) -> Result<()> {
        if action == 1 {
            self.store.require_available()?;
        }
        // SAFETY: this object's live engine handle is not freed while borrowed.
        let code = unsafe { herdr_ts_command(self.id, action) };
        self.storage_error()?;
        check(code)
    }

    pub(super) fn snapshot(&self) -> Result<Snapshot> {
        self.storage_error()?;
        let mut data = Zeroizing::new(vec![0; 4 * 1024 * 1024]);
        let mut length = 0;
        // SAFETY: data provides its declared capacity; the ABI bounds the write.
        check(unsafe { herdr_ts_snapshot(self.id, data.as_mut_ptr(), data.len(), &mut length) })?;
        if length > data.len() {
            return Err(Error::Limit);
        }
        Ok(serde_json::from_slice(&data[..length])?)
    }

    pub(super) fn dial(&self, peer: &str, port: u16) -> Result<u64> {
        let mut stream = 0;
        // SAFETY: inputs and result live through the call, which copies peer.
        check(unsafe { herdr_ts_dial(self.id, peer.as_ptr(), peer.len(), port, &mut stream) })?;
        Ok(stream)
    }

    pub(super) fn read(&self, stream: u64, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let mut length = 0;
        // SAFETY: output is exclusively borrowed, with exactly capacity bytes.
        let code = unsafe {
            herdr_ts_read(
                self.id,
                stream,
                output.as_mut_ptr(),
                output.len().min(65536),
                &mut length,
            )
        };
        check(code).map_err(to_io)?;
        if length > output.len() {
            return Err(io::Error::other(Error::Limit));
        }
        Ok(length)
    }

    pub(super) fn write(&self, stream: u64, input: &[u8]) -> io::Result<usize> {
        if input.is_empty() {
            return Ok(0);
        }
        let mut written = 0;
        // SAFETY: input is borrowed for this call only and is not mutated by Go.
        let code = unsafe {
            herdr_ts_write(
                self.id,
                stream,
                input.as_ptr(),
                input.len().min(65536),
                &mut written,
            )
        };
        check(code).map_err(to_io)?;
        if written > input.len() {
            return Err(io::Error::other(Error::Limit));
        }
        Ok(written)
    }

    pub(super) fn close_stream(&self, stream: u64) {
        // SAFETY: the ABI tolerates a stream already closed by engine shutdown.
        unsafe { herdr_ts_close_stream(self.id, stream) };
    }

    pub(super) fn has_identity(&self) -> Result<bool> {
        self.store.has_identity()
    }

    pub(super) fn clear_identity(&self) -> Result<()> {
        self.store.clear()
    }

    fn storage_error(&self) -> Result<()> {
        match self
            .store
            .error
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl Drop for Native {
    fn drop(&mut self) {
        // SAFETY: this is the unique owning handle; destroy finishes callbacks
        // before Box<Storage> is released. Callers drop it off the UI thread.
        unsafe { herdr_ts_destroy(self.id) };
    }
}

fn check(code: i32) -> Result<()> {
    match code {
        0 => Ok(()),
        -1 => Err(Error::InvalidConfig),
        -2 => Err(Error::Closed),
        -3 => Err(Error::NotReady),
        -4 => Err(Error::PeerUnavailable),
        -5 => Err(Error::Busy),
        -6 => Err(Error::Limit),
        -7 => Err(Error::Storage),
        -9 => Err(Error::Timeout),
        -10 => Err(Error::Environment),
        _ => Err(Error::Io(io::ErrorKind::ConnectionAborted.into())),
    }
}

fn to_io(error: Error) -> io::Error {
    match error {
        Error::Timeout => io::Error::new(io::ErrorKind::TimedOut, error),
        Error::Closed => io::Error::new(io::ErrorKind::NotConnected, error),
        _ => io::Error::other(error),
    }
}

unsafe extern "C" fn storage_callback(
    context: usize,
    op: u32,
    key: *const u8,
    key_length: usize,
    data: *mut u8,
    capacity: usize,
    length: *mut usize,
) -> i32 {
    if context == 0
        || key.is_null()
        || key_length == 0
        || key_length > 256
        || capacity > 65536
        || length.is_null()
        || (capacity > 0 && data.is_null())
    {
        return -1;
    }
    // SAFETY: native calls this only while the boxed context and borrowed buffers
    // are alive; no pointer or slice escapes the callback. Panics never cross C.
    catch_unwind(AssertUnwindSafe(|| unsafe {
        let store = &*(context as *const Storage);
        let key = std::slice::from_raw_parts(key, key_length);
        let result = match op {
            0 => match store.read(key) {
                Ok(Some(bytes)) if bytes.len() <= capacity => {
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
                    *length = bytes.len();
                    return 0;
                }
                Ok(Some(_)) => Err(Error::Limit),
                Ok(None) => return 1,
                Err(error) => Err(error),
            },
            1 => store.write(
                key,
                Some(if capacity == 0 {
                    &[]
                } else {
                    std::slice::from_raw_parts(data, capacity)
                }),
            ),
            2 => store.write(key, None),
            _ => Err(Error::InvalidConfig),
        };
        if let Err(error) = result {
            *store.error.lock().unwrap_or_else(PoisonError::into_inner) = Some(error);
            -1
        } else {
            *length = 0;
            0
        }
    }))
    .unwrap_or(-1)
}
