//! Java: the runtime each Minecraft version wants, provisioned from Mojang.
//!
//! Every version file names a runtime component (`jre-legacy` is Java 8 for
//! everything up to 1.16, `java-runtime-gamma` is 17, `-delta` is 21,
//! `-epsilon` is 25...). Mojang publishes those runtimes for each platform,
//! file by file with SHA-1s, at the address below, the same ones the official
//! launcher uses. So a player never installs Java, and an old version never
//! runs on a Java too new for it.
//!
//! A player can still point an instance (or every instance) at their own Java;
//! `probe` reads its version so the UI can say when it does not fit.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::{Deserialize, Serialize};

use crate::download::{self, FileJob, Verify};
use crate::net;
use crate::paths::Paths;
use crate::tasks::TaskHandle;

const RUNTIMES_URL: &str =
    "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

/// Mojang's name for this platform in the runtime list.
pub fn platform_key() -> &'static str {
    if cfg!(windows) {
        if cfg!(target_arch = "aarch64") {
            "windows-arm64"
        } else if cfg!(target_arch = "x86") {
            "windows-x86"
        } else {
            "windows-x64"
        }
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "mac-os-arm64"
        } else {
            "mac-os"
        }
    } else if cfg!(target_arch = "x86") {
        "linux-i386"
    } else {
        "linux"
    }
}

#[derive(Deserialize)]
struct RuntimeEntry {
    manifest: RuntimeManifestRef,
    version: RuntimeVersion,
}

#[derive(Deserialize)]
struct RuntimeManifestRef {
    url: String,
}

#[derive(Deserialize)]
struct RuntimeVersion {
    name: String,
}

#[derive(Deserialize)]
struct RuntimeManifest {
    files: HashMap<String, RuntimeFile>,
}

#[derive(Deserialize)]
struct RuntimeFile {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    executable: bool,
    #[serde(default)]
    downloads: Option<RuntimeDownloads>,
    /// Where a link points (macOS and Linux runtimes).
    #[serde(default)]
    #[cfg_attr(not(unix), allow(dead_code))]
    target: Option<String>,
}

#[derive(Deserialize)]
struct RuntimeDownloads {
    raw: RawDownload,
}

#[derive(Deserialize)]
struct RawDownload {
    url: String,
    sha1: String,
    size: u64,
}

pub fn runtime_dir(paths: &Paths, component: &str) -> PathBuf {
    paths.runtimes().join(component)
}

/// The java executable inside a runtime folder.
pub fn java_in(dir: &Path) -> PathBuf {
    if cfg!(windows) {
        dir.join("bin").join("java.exe")
    } else if cfg!(target_os = "macos") {
        dir.join("jre.bundle").join("Contents").join("Home").join("bin").join("java")
    } else {
        dir.join("bin").join("java")
    }
}

/// Readable runtime names for the UI.
pub fn component_label(component: &str, major: u32) -> String {
    let _ = component;
    format!("Java {major}")
}

/// Make sure the runtime `component` is installed and complete; returns its
/// java executable. Files already there are kept.
pub async fn ensure_runtime(paths: &Paths, component: &str, major: u32, task: &TaskHandle, parallel: usize) -> Result<PathBuf, String> {
    let dir = runtime_dir(paths, component);
    let marker = dir.join(".kryoto-runtime.json");
    let label = component_label(component, major);
    task.stage(&label);

    // Offline, an installed runtime is used as it is.
    let files: Vec<(String, RuntimeFile)> = match fetch_manifest(component).await {
        Ok((manifest, version)) => {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(&marker, serde_json::json!({ "component": component, "version": version }).to_string());
            manifest.files.into_iter().collect()
        }
        Err(e) => {
            let java = java_in(&dir);
            if java.exists() {
                return Ok(java);
            }
            return Err(format!("{label} isn't installed yet and couldn't be downloaded. {e}"));
        }
    };

    let mut jobs = Vec::new();
    let mut executables = Vec::new();
    for (path, file) in &files {
        let target = crate::paths::join_relative(&dir, path);
        match file.kind.as_str() {
            "directory" => {
                std::fs::create_dir_all(&target).map_err(|e| format!("Couldn't create {}: {e}", target.display()))?;
            }
            "file" => {
                if let Some(d) = &file.downloads {
                    jobs.push(FileJob { url: d.raw.url.clone(), path: target.clone(), sha1: Some(d.raw.sha1.clone()), size: Some(d.raw.size) });
                    if file.executable {
                        executables.push(target);
                    }
                }
            }
            // Links are only used on macOS and Linux, inside the runtime.
            _ => {}
        }
    }
    download::download_all(jobs, Verify::Size, task, parallel).await?;
    #[cfg(unix)]
    for exe in executables {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755));
    }
    #[cfg(not(unix))]
    drop(executables);
    #[cfg(unix)]
    for (path, file) in &files {
        if let (true, Some(target)) = (file.kind == "link", &file.target) {
            let link = crate::paths::join_relative(&dir, path);
            if std::fs::symlink_metadata(&link).is_err() {
                let _ = std::os::unix::fs::symlink(target, &link);
            }
        }
    }

    let java = java_in(&dir);
    if !java.exists() {
        return Err(format!("{label} downloaded, but its java program is missing. Try again, or choose your own Java in Settings."));
    }
    Ok(java)
}

async fn fetch_manifest(component: &str) -> Result<(RuntimeManifest, String), String> {
    let all: HashMap<String, HashMap<String, Vec<RuntimeEntry>>> = net::get_json(RUNTIMES_URL).await?;
    let platform = platform_key();
    let entry = all
        .get(platform)
        .and_then(|p| p.get(component))
        .and_then(|list| list.first())
        .ok_or_else(|| format!("Mojang has no {component} runtime for {platform}. Choose your own Java for this instance in its settings."))?;
    let manifest: RuntimeManifest = net::get_json(&entry.manifest.url).await?;
    Ok((manifest, entry.version.name.clone()))
}

/* ── A player's own Java ────────────────────────────────── */

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaInfo {
    pub path: String,
    pub version: String,
    pub major: u32,
    pub arch: String,
    pub vendor: String,
}

/// Run `java -XshowSettings:properties -version` and read what it says.
pub async fn probe(path: &Path) -> Result<JavaInfo, String> {
    if !path.is_file() {
        return Err(format!("{} doesn't exist.", path.display()));
    }
    let mut command = tokio::process::Command::new(path);
    command.args(["-XshowSettings:properties", "-version"]).stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
    no_window(&mut command);
    let out = tokio::time::timeout(std::time::Duration::from_secs(20), command.output())
        .await
        .map_err(|_| "That Java didn't answer within 20 seconds.".to_string())?
        .map_err(|e| format!("Couldn't run {}: {e}", path.display()))?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stderr), String::from_utf8_lossy(&out.stdout));
    let prop = |name: &str| {
        text.lines()
            .find_map(|l| l.trim().strip_prefix(name).and_then(|r| r.trim_start().strip_prefix('=')).map(|v| v.trim().to_string()))
    };
    let version = prop("java.version").ok_or_else(|| "That doesn't look like a Java program.".to_string())?;
    let spec = prop("java.specification.version").unwrap_or_else(|| version.clone());
    Ok(JavaInfo {
        path: path.display().to_string(),
        major: parse_major(&spec),
        version,
        arch: prop("os.arch").unwrap_or_default(),
        vendor: prop("java.vendor").unwrap_or_default(),
    })
}

/// "1.8" -> 8, "17" -> 17, "21.0.2" -> 21.
pub fn parse_major(spec: &str) -> u32 {
    let mut parts = spec.split(['.', '_', '-', '+']);
    let first: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    if first == 1 {
        parts.next().and_then(|p| p.parse().ok()).unwrap_or(0)
    } else {
        first
    }
}

/// Start console programs (java.exe) without a console window flashing up.
pub fn no_window(command: &mut tokio::process::Command) {
    #[cfg(windows)]
    {
        command.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    let _ = command;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn majors() {
        assert_eq!(parse_major("1.8"), 8);
        assert_eq!(parse_major("17"), 17);
        assert_eq!(parse_major("21.0.2"), 21);
        assert_eq!(parse_major("1.8.0_51"), 8);
    }
}
