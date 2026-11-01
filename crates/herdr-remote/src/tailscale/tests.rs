use super::*;

mod config;
#[cfg(unix)]
mod credentials;
mod engine;

// Native handles have a process-wide budget of four. Tests of independent
// identities coordinate here so they exercise ownership rather than that cap.
pub(super) fn native_engine_guard() -> std::sync::MutexGuard<'static, ()> {
    static NATIVE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    NATIVE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
