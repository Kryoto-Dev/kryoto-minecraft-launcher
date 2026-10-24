//! Long jobs (installing a version, a Java runtime, a mod) and their progress.
//!
//! Every job is a `Task` in one list the UI shows in Downloads and the bottom
//! bar. The list goes to the UI as a whole (`tasks` event), at most ten times a
//! second, so a download of four thousand small asset files does not flood the
//! page with events.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    Running,
    Done,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub title: String,
    /// The instance this job is for, so its page can show it.
    pub instance: Option<String>,
    /// What it is doing now ("Libraries", "Assets", "Java 21").
    pub stage: String,
    pub state: TaskState,
    pub done_bytes: u64,
    pub total_bytes: u64,
    pub done_files: u64,
    pub total_files: u64,
    /// Bytes per second over the last few seconds.
    pub speed: u64,
    /// Everything the job downloaded, over all its batches.
    pub downloaded: u64,
    pub error: Option<String>,
    pub started: u64,
    pub finished: Option<u64>,
    /// What "Try again" does: the same job from the start (files already
    /// downloaded and checked are skipped).
    pub retry: Option<serde_json::Value>,
}

type Emit = Arc<dyn Fn(&[Task]) + Send + Sync>;
type Cancels = Arc<Mutex<Vec<(String, Arc<AtomicBool>)>>>;

#[derive(Clone)]
pub struct Tasks {
    inner: Arc<Mutex<Vec<Task>>>,
    cancels: Cancels,
    emit: Emit,
    last_emit: Arc<Mutex<Instant>>,
    pending: Arc<AtomicBool>,
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Tasks {
    pub fn new(emit: impl Fn(&[Task]) + Send + Sync + 'static) -> Self {
        Self {
            inner: Arc::default(),
            cancels: Arc::default(),
            emit: Arc::new(emit),
            last_emit: Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1))),
            pending: Arc::default(),
        }
    }

    /// No UI: for tests and tools.
    pub fn silent() -> Self {
        Self::new(|_| {})
    }

    pub fn list(&self) -> Vec<Task> {
        self.inner.lock().unwrap().clone()
    }

    /// Start a job. A job with the same title for the same instance that is
    /// still running is refused, so a double click does not install twice.
    pub fn start(&self, title: &str, instance: Option<&str>, retry: Option<serde_json::Value>) -> Result<TaskHandle, String> {
        let mut list = self.inner.lock().unwrap();
        if list
            .iter()
            .any(|t| t.state == TaskState::Running && t.title == title && t.instance.as_deref() == instance)
        {
            return Err(format!("{title} is already running."));
        }
        let mut bytes = [0u8; 6];
        let _ = getrandom::getrandom(&mut bytes);
        let id = format!("t{}-{}", now(), hex::encode(bytes));
        // Finished jobs older than the newest 30 drop off the list.
        let finished: Vec<usize> = list.iter().enumerate().filter(|(_, t)| t.state != TaskState::Running).map(|(i, _)| i).collect();
        if finished.len() > 30 {
            let drop: Vec<usize> = finished[..finished.len() - 30].to_vec();
            let mut i = 0;
            list.retain(|_| {
                let keep = !drop.contains(&i);
                i += 1;
                keep
            });
        }
        list.push(Task {
            id: id.clone(),
            title: title.into(),
            instance: instance.map(str::to_string),
            stage: "Starting".into(),
            state: TaskState::Running,
            done_bytes: 0,
            total_bytes: 0,
            done_files: 0,
            total_files: 0,
            speed: 0,
            downloaded: 0,
            error: None,
            started: now(),
            finished: None,
            retry,
        });
        drop(list);
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancels.lock().unwrap().push((id.clone(), cancel.clone()));
        self.flush(true);
        Ok(TaskHandle {
            id,
            tasks: self.clone(),
            cancel,
            speed: Arc::new(Mutex::new(Speed::default())),
            bytes: Arc::new(AtomicU64::new(0)),
        })
    }

    /// The title of a job still running for this instance, if any.
    pub fn running_for(&self, instance: &str) -> Option<String> {
        self.inner.lock().unwrap().iter().find(|t| t.state == TaskState::Running && t.instance.as_deref() == Some(instance)).map(|t| t.title.clone())
    }

    pub fn cancel(&self, id: &str) {
        if let Some((_, flag)) = self.cancels.lock().unwrap().iter().find(|(t, _)| t == id) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// Forget finished jobs.
    pub fn clear(&self) {
        self.inner.lock().unwrap().retain(|t| t.state == TaskState::Running);
        self.flush(true);
    }

    fn update(&self, id: &str, f: impl FnOnce(&mut Task)) {
        if let Some(t) = self.inner.lock().unwrap().iter_mut().find(|t| t.id == id) {
            f(t);
        }
    }

    /// Send the list, unless one went out in the last 100 ms (then the next
    /// update, or the final one, carries this change).
    fn flush(&self, force: bool) {
        let mut last = self.last_emit.lock().unwrap();
        if !force && last.elapsed() < Duration::from_millis(100) {
            self.pending.store(true, Ordering::Relaxed);
            return;
        }
        *last = Instant::now();
        self.pending.store(false, Ordering::Relaxed);
        let list = self.inner.lock().unwrap().clone();
        drop(last);
        (self.emit)(&list);
    }
}

#[derive(Default)]
struct Speed {
    samples: Vec<(Instant, u64)>,
}

/// One running job, handed to whatever does the work.
#[derive(Clone)]
pub struct TaskHandle {
    pub id: String,
    tasks: Tasks,
    cancel: Arc<AtomicBool>,
    speed: Arc<Mutex<Speed>>,
    bytes: Arc<AtomicU64>,
}

pub const CANCELLED: &str = "Cancelled.";

impl TaskHandle {
    pub fn stage(&self, stage: &str) {
        self.tasks.update(&self.id, |t| t.stage = stage.into());
        self.tasks.flush(true);
    }

    /// Reset the counters for a new batch of files.
    pub fn batch(&self, files: u64, bytes: u64) {
        self.bytes.store(0, Ordering::Relaxed);
        self.speed.lock().unwrap().samples.clear();
        self.tasks.update(&self.id, |t| {
            t.total_files = files;
            t.total_bytes = bytes;
            t.done_files = 0;
            t.done_bytes = 0;
        });
        self.tasks.flush(true);
    }

    pub fn add_bytes(&self, n: u64) {
        let done = self.bytes.fetch_add(n, Ordering::Relaxed) + n;
        let speed = {
            let mut s = self.speed.lock().unwrap();
            let now = Instant::now();
            s.samples.push((now, done));
            s.samples.retain(|(at, _)| now.duration_since(*at) < Duration::from_secs(3));
            match (s.samples.first(), s.samples.last()) {
                (Some((a, x)), Some((b, y))) if b.duration_since(*a) > Duration::from_millis(300) => {
                    (y.saturating_sub(*x) as f64 / b.duration_since(*a).as_secs_f64()) as u64
                }
                _ => 0,
            }
        };
        self.tasks.update(&self.id, |t| {
            t.done_bytes = done;
            t.downloaded += n;
            if speed > 0 {
                t.speed = speed;
            }
        });
        self.tasks.flush(false);
    }

    pub fn file_done(&self) {
        self.tasks.update(&self.id, |t| t.done_files += 1);
        self.tasks.flush(false);
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn check(&self) -> Result<(), String> {
        if self.cancelled() {
            Err(CANCELLED.into())
        } else {
            Ok(())
        }
    }

    pub fn finish(&self, result: &Result<(), String>) {
        self.tasks.update(&self.id, |t| {
            t.finished = Some(now());
            t.speed = 0;
            match result {
                Ok(()) => {
                    t.state = TaskState::Done;
                    t.stage = "Done".into();
                    t.done_bytes = t.total_bytes;
                    t.done_files = t.total_files;
                }
                Err(e) if e == CANCELLED => {
                    t.state = TaskState::Cancelled;
                    t.stage = "Cancelled".into();
                }
                Err(e) => {
                    t.state = TaskState::Failed;
                    t.stage = "Failed".into();
                    t.error = Some(e.clone());
                }
            }
        });
        self.tasks.cancels.lock().unwrap().retain(|(id, _)| id != &self.id);
        self.tasks.flush(true);
    }
}
