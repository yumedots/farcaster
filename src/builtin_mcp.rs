use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(test)]
use std::sync::{Mutex, MutexGuard};

static ENABLED: AtomicBool = AtomicBool::new(true);

#[cfg(test)]
static EXCLUSIVE: Mutex<()> = Mutex::new(());

pub(crate) fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub(crate) fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

/// A test that reads `enabled` through a spawned process, and a test that
/// changes it, must hold this for as long as the value matters. Without it one
/// test's toggle decides another test's child environment.
#[cfg(test)]
pub(crate) fn exclusive_for_test() -> MutexGuard<'static, ()> {
    EXCLUSIVE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Holds the exclusive claim on the builtin MCP while it is disabled, and puts
/// back whatever the value was rather than assuming it was on.
#[cfg(test)]
pub(crate) struct McpDisabledForTest {
    _exclusive: MutexGuard<'static, ()>,
    previous: bool,
}

#[cfg(test)]
impl McpDisabledForTest {
    pub(crate) fn new() -> Self {
        let exclusive = exclusive_for_test();
        let previous = enabled();
        set_enabled(false);
        Self {
            _exclusive: exclusive,
            previous,
        }
    }
}

#[cfg(test)]
impl Drop for McpDisabledForTest {
    fn drop(&mut self) {
        set_enabled(self.previous);
    }
}
