//! Session resource cleanup registry.

use std::sync::Mutex;

static CLEANUP_HANDLERS: Mutex<Vec<Box<dyn Fn(Option<&str>) + Send>>> = Mutex::new(Vec::new());

/// Register a cleanup function for session resources.
/// Returns a deregistration function.
pub fn register_session_resource_cleanup<F>(cleanup: F) -> impl Fn()
where
    F: Fn(Option<&str>) + Send + 'static,
{
    let mut handlers = CLEANUP_HANDLERS.lock().unwrap();
    let idx = handlers.len();
    handlers.push(Box::new(cleanup));

    move || {
        if let Ok(mut handlers) = CLEANUP_HANDLERS.lock()
            && idx < handlers.len()
        {
            drop(handlers.swap_remove(idx));
        }
    }
}

/// Invoke all registered cleanup handlers.
pub fn cleanup_session_resources(session_id: Option<&str>) {
    let handlers = CLEANUP_HANDLERS.lock().unwrap();
    for handler in handlers.iter() {
        handler(session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_register_and_cleanup() {
        static CALLED: AtomicBool = AtomicBool::new(false);
        let _deregister = register_session_resource_cleanup(|_| {
            CALLED.store(true, Ordering::SeqCst);
        });
        cleanup_session_resources(Some("test-session"));
        assert!(CALLED.load(Ordering::SeqCst));
    }

    #[test]
    fn test_deregister() {
        static CALLED: AtomicBool = AtomicBool::new(false);
        let deregister = register_session_resource_cleanup(|_| {
            CALLED.store(true, Ordering::SeqCst);
        });
        deregister();
        CALLED.store(false, Ordering::SeqCst);
        cleanup_session_resources(None);
        assert!(!CALLED.load(Ordering::SeqCst));
    }
}
