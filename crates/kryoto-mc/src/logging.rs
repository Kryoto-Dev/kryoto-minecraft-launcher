//! The launcher's own log (`logs/launcher.log`), and keeping it to one copy.

use std::io::Write;
use std::sync::{Mutex, OnceLock};

use crate::paths::Paths;

static LOCK: Mutex<()> = Mutex::new(());
const MAX_BYTES: u64 = 2 * 1024 * 1024;

/// One line in the log; the file starts over (keeping one old copy) at 2 MB.
pub fn write(paths: &Paths, level: &str, scope: &str, message: &str) {
    let _guard = LOCK.lock();
    let dir = paths.logs();
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join("launcher.log");
    if std::fs::metadata(&file).map(|m| m.len() > MAX_BYTES).unwrap_or(false) {
        let _ = std::fs::rename(&file, dir.join("launcher.old.log"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&file) {
        let _ = writeln!(f, "{} [{level}] {scope}: {}", crate::tasks::now(), message.replace('\n', " | "));
    }
    if cfg!(debug_assertions) {
        eprintln!("[{level}] {scope}: {message}");
    }
}

/// Hold a lock file for as long as this process lives; false when another
/// copy of the launcher already holds it. Two copies would download into the
/// same folders at once.
pub fn claim_single_instance(paths: &Paths) -> bool {
    static HELD: OnceLock<std::fs::File> = OnceLock::new();
    let path = paths.root.join(".lock");
    let file = match std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&path) {
        Ok(f) => f,
        Err(_) => return true,
    };
    match file.try_lock() {
        Ok(()) => {
            let _ = HELD.set(file);
            true
        }
        Err(std::fs::TryLockError::WouldBlock) => false,
        // A file system without locks: nothing to guard with.
        Err(std::fs::TryLockError::Error(_)) => true,
    }
}
