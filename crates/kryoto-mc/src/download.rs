//! Downloading many files at once, checked, resumable by skipping.
//!
//! A file that is already on disk with the right size (and, when asked, the
//! right SHA-1) is never downloaded again. Everything else is fetched to a
//! `.part` file beside its target, hashed while it streams, and only renamed
//! into place once size and hash match, so an interrupted download can never
//! leave a broken library behind. Each file gets three tries.

use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::{stream, StreamExt};
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;

use crate::net;
use crate::tasks::TaskHandle;

#[derive(Clone, Debug)]
pub struct FileJob {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

/// How hard to look at a file that is already there.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Verify {
    /// Present with the expected size. Asset objects are named by their hash,
    /// so this is enough for them, and it keeps starting a game fast.
    Size,
    /// Present, the expected size and the expected SHA-1 (a repair).
    Hash,
}

pub fn sha1_file(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Whether `job` is already satisfied on disk.
pub fn present(job: &FileJob, verify: Verify) -> bool {
    let Ok(meta) = std::fs::metadata(&job.path) else { return false };
    if !meta.is_file() {
        return false;
    }
    if let Some(size) = job.size {
        if meta.len() != size {
            return false;
        }
    } else if meta.len() == 0 {
        return false;
    }
    match (verify, &job.sha1) {
        (Verify::Hash, Some(expected)) => sha1_file(&job.path).map(|h| h.eq_ignore_ascii_case(expected)).unwrap_or(false),
        // Without a size to compare, a file is only trusted after a hash check.
        (Verify::Size, Some(expected)) if job.size.is_none() => {
            sha1_file(&job.path).map(|h| h.eq_ignore_ascii_case(expected)).unwrap_or(false)
        }
        _ => true,
    }
}

/// Fetch every job that is not already there, `parallel` at a time.
pub async fn download_all(mut jobs: Vec<FileJob>, verify: Verify, task: &TaskHandle, parallel: usize) -> Result<(), String> {
    // The same file can be listed twice (a library shared by two versions in
    // one inheritance chain); fetching it twice at once would race on .part.
    jobs.sort_by(|a, b| a.path.cmp(&b.path));
    jobs.dedup_by(|a, b| a.path == b.path);

    let checking = jobs.len() > 200 || verify == Verify::Hash;
    if checking {
        task.stage("Checking files");
    }
    let missing: Vec<FileJob> = {
        let jobs = jobs.clone();
        tokio::task::spawn_blocking(move || jobs.into_iter().filter(|j| !present(j, verify)).collect())
            .await
            .map_err(|e| e.to_string())?
    };
    task.check()?;
    if missing.is_empty() {
        return Ok(());
    }
    let total: u64 = missing.iter().map(|j| j.size.unwrap_or(0)).sum();
    task.batch(missing.len() as u64, total);

    let results: Vec<Result<(), String>> = stream::iter(missing)
        .map(|job| async move {
            let result = fetch_with_retries(&job, task).await;
            task.file_done();
            result
        })
        .buffer_unordered(parallel.max(1))
        .collect()
        .await;
    task.check()?;
    let failures: Vec<String> = results.into_iter().filter_map(Result::err).collect();
    match failures.len() {
        0 => Ok(()),
        1 => Err(failures[0].clone()),
        n => Err(format!("{n} files failed to download. The first: {}", failures[0])),
    }
}

/// One file, three tries.
pub async fn fetch_with_retries(job: &FileJob, task: &TaskHandle) -> Result<(), String> {
    let mut last = String::new();
    for attempt in 0..3u64 {
        task.check()?;
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(1000 * attempt * attempt)).await;
        }
        match fetch(job, task).await {
            Ok(()) => return Ok(()),
            Err(Fatal(e)) => return Err(e),
            Err(Retry(e)) => last = e,
        }
    }
    Err(last)
}

enum Failure {
    Retry(String),
    Fatal(String),
}
use Failure::{Fatal, Retry};

fn name_of(job: &FileJob) -> String {
    job.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| job.url.clone())
}

async fn fetch(job: &FileJob, task: &TaskHandle) -> Result<(), Failure> {
    let name = name_of(job);
    if let Some(dir) = job.path.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| Fatal(format!("Couldn't create the folder for {name}: {e}")))?;
    }
    let response = net::client().get(&job.url).send().await.map_err(|e| Retry(format!("{name}: {}", net::explain(&job.url, &e))))?;
    let status = response.status();
    if !status.is_success() {
        let message = format!("{name}: {} answered {status}.", net::host(&job.url));
        return Err(if status.is_client_error() { Fatal(message) } else { Retry(message) });
    }
    let part = job.path.with_extension(match job.path.extension() {
        Some(ext) => format!("{}.part", ext.to_string_lossy()),
        None => "part".into(),
    });
    let mut file = tokio::fs::File::create(&part)
        .await
        .map_err(|e| Fatal(format!("Couldn't write {name}: {e}")))?;
    let mut hasher = Sha1::new();
    let mut written = 0u64;
    let mut body = response.bytes_stream();
    while let Some(chunk) = body.next().await {
        if task.cancelled() {
            drop(file);
            let _ = tokio::fs::remove_file(&part).await;
            return Err(Fatal(crate::tasks::CANCELLED.into()));
        }
        let chunk = chunk.map_err(|e| Retry(format!("{name}: {}", net::explain(&job.url, &e))))?;
        hasher.update(&chunk);
        file.write_all(&chunk).await.map_err(|e| Fatal(disk_error(&name, &e)))?;
        written += chunk.len() as u64;
        task.add_bytes(chunk.len() as u64);
    }
    file.flush().await.map_err(|e| Fatal(disk_error(&name, &e)))?;
    drop(file);

    let bad = |why: String| {
        let part = part.clone();
        async move {
            let _ = tokio::fs::remove_file(&part).await;
            Err(Retry(why))
        }
    };
    if let Some(size) = job.size {
        if size != written {
            return bad(format!("{name} arrived incomplete ({written} of {size} bytes).")).await;
        }
    }
    if let Some(expected) = &job.sha1 {
        let got = hex::encode(hasher.finalize());
        if !got.eq_ignore_ascii_case(expected) {
            return bad(format!("{name} arrived damaged (its checksum doesn't match).")).await;
        }
    }
    // Replace whatever was there (a damaged copy, or one of the wrong size).
    let _ = tokio::fs::remove_file(&job.path).await;
    tokio::fs::rename(&part, &job.path)
        .await
        .map_err(|e| Fatal(format!("Couldn't put {name} in place: {e}")))?;
    Ok(())
}

fn disk_error(name: &str, e: &std::io::Error) -> String {
    // ERROR_DISK_FULL (112) on Windows, ENOSPC (28) elsewhere.
    if matches!(e.raw_os_error(), Some(112) | Some(28)) {
        "The disk is full. Free some space and try again.".into()
    } else {
        format!("Couldn't write {name}: {e}")
    }
}
