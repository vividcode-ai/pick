//! File system watching utilities

use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub const FS_WATCH_RETRY_DELAY_MS: u64 = 5000;

/// A simple file watcher handle
pub struct FsWatcher {
    _thread: thread::JoinHandle<()>,
    pub(crate) shutdown: mpsc::Sender<()>,
}

/// Close a watcher if it exists
pub fn close_watcher(watcher: Option<FsWatcher>) {
    if let Some(w) = watcher {
        let _ = w.shutdown.send(());
    }
}

/// Watch a file or directory with an error handler
/// Uses polling-based watching as a simple cross-platform approach
pub fn watch_with_error_handler<F>(path: &Path, callback: F, on_error: Box<dyn Fn() + Send>)
    -> Option<FsWatcher>
where
    F: Fn(String) + Send + 'static,
{
    let watch_path = path.to_path_buf();
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    let handle = thread::spawn(move || {
        // Simple polling-based watcher
        let mut last_content = std::fs::read_to_string(&watch_path).ok();
        let mut has_errored = false;

        loop {
            if shutdown_rx.try_recv().is_ok() {
                return;
            }

            match std::fs::read_to_string(&watch_path) {
                Ok(current) => {
                    has_errored = false;
                    if last_content.as_ref() != Some(&current) {
                        last_content = Some(current);
                        callback(watch_path.to_string_lossy().to_string());
                    }
                }
                Err(_) if !has_errored => {
                    has_errored = true;
                    on_error();
                }
                _ => {}
            }

            thread::sleep(Duration::from_millis(500));
        }
    });

    Some(FsWatcher { _thread: handle, shutdown: shutdown_tx })
}
