use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Mutex;
use tokio::sync::oneshot;

use std::sync::LazyLock;

static FILE_MUTATION_QUEUES: LazyLock<Mutex<HashMap<String, QueueState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

struct QueueState {
    current: Option<Pin<Box<dyn Future<Output = ()> + Send>>>,
    waker: Option<oneshot::Sender<()>>,
}

/// Serialize file mutation operations targeting the same file.
/// Operations for different files still run in parallel.
pub async fn with_file_mutation_queue<T, F, Fut>(file_path: &str, f: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    let key = resolve_key(file_path).await;

    // Wait for our turn
    let wait = {
        let mut queues = FILE_MUTATION_QUEUES.lock().unwrap();
        let entry = queues.entry(key.clone()).or_insert_with(|| QueueState {
            current: None,
            waker: None,
        });

        if entry.current.is_none() {
            None
        } else {
            Some(wait_for_queue(key.clone()))
        }
    };

    if let Some(fut) = wait {
        fut.await;
    }

    // Create the next queue future
    let (tx, rx) = oneshot::channel::<()>();
    let next_fut: Pin<Box<dyn Future<Output = ()> + Send>> = Box::pin(async move {
        let _ = rx.await;
    });

    {
        let mut queues = FILE_MUTATION_QUEUES.lock().unwrap();
        queues.insert(
            key.clone(),
            QueueState {
                current: Some(next_fut),
                waker: None,
            },
        );
    }

    // Run the operation
    let result = f().await;

    // Signal the next waiter
    let _ = tx.send(());
    {
        let mut queues = FILE_MUTATION_QUEUES.lock().unwrap();
        if let Some(entry) = queues.get(&key)
            && entry.current.is_none()
        {
            queues.remove(&key);
        }
    }

    result
}

async fn resolve_key(file_path: &str) -> String {
    let path = Path::new(file_path);
    if path.exists() {
        dunce::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string()
    } else {
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        };
        resolved.to_string_lossy().to_string()
    }
}

async fn wait_for_queue(key: String) {
    let (tx, rx) = oneshot::channel::<()>();
    {
        let mut queues = FILE_MUTATION_QUEUES.lock().unwrap();
        if let Some(entry) = queues.get_mut(&key) {
            entry.waker = Some(tx);
        }
    }
    let _ = rx.await;
}
