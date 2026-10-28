//! Mod loaders: Fabric, Quilt, Forge and NeoForge.
//!
//! Fabric and Quilt publish ready version files (`.../profile/json`) that
//! inherit from the vanilla one, so installing them is saving that file; its
//! libraries come from their maven repositories like any other.
//!
//! Forge and NeoForge are installed by their own official installers, run
//! headless (`--installClient`) into this launcher's data folder, which has
//! the `.minecraft` layout they expect. That is the only way that stays right
//! across their many installer formats: from 1.13 they patch the game with
//! processors, which the installer runs. Forge's oldest installers (before
//! 1.12.2's last builds) cannot run headless; for those the version file and
//! the Forge jar are taken out of the installer, which is all they did.

use std::io::Read;
use std::path::Path;
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::download::{self, FileJob, Verify};
use crate::instances::LoaderKind;
use crate::net;
use crate::paths::Paths;
use crate::tasks::TaskHandle;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersion {
    /// What the installer is named by (Forge's full maven version).
    pub version: String,
    /// What a player reads.
    pub label: String,
    pub stable: bool,
    pub recommended: bool,
}

/// Which loader versions exist for a Minecraft version, newest first.
pub async fn versions(paths: &Paths, kind: &LoaderKind, game: &str) -> Result<Vec<LoaderVersion>, String> {
    match kind {
        LoaderKind::Vanilla => Ok(Vec::new()),
        LoaderKind::Fabric => fabric_like("https://meta.fabricmc.net/v2", game).await,
        LoaderKind::Quilt => fabric_like("https://meta.quiltmc.org/v3", game).await,
        LoaderKind::Forge => forge_versions(paths, game).await,
        LoaderKind::NeoForge => neoforge_versions(paths, game).await,
    }
}

#[derive(Deserialize)]
struct FabricEntry {
    loader: FabricLoader,
}
#[derive(Deserialize)]
struct FabricLoader {
    version: String,
    #[serde(default)]
    stable: Option<bool>,
}

async fn fabric_like(base: &str, game: &str) -> Result<Vec<LoaderVersion>, String> {
    let url = format!("{base}/versions/loader/{}", url_part(game));
    let entries: Vec<FabricEntry> = match net::get_json(&url).await {
        Ok(e) => e,
        // An unknown game version is a 400 at Fabric and Quilt: no loader for it.
        Err(e) if e.contains("400") || e.contains("404") => Vec::new(),
        Err(e) => return Err(e),
    };
    // Quilt lists its loaders in no particular order (0.20.0-beta.9 first,
    // 0.24.0 sixth), so the newest is found by sorting, not by position.
    let mut out: Vec<LoaderVersion> = entries
        .into_iter()
        .map(|e| {
            let stable = e.loader.stable.unwrap_or(!is_prerelease(&e.loader.version));
            LoaderVersion { label: e.loader.version.clone(), version: e.loader.version, stable, recommended: false }
        })
        .collect();
    out.sort_by(|a, b| version_cmp(&b.label, &a.label));
    if let Some(first) = out.iter_mut().find(|v| v.stable) {
        first.recommended = true;
    }
    Ok(out)
}

fn is_prerelease(version: &str) -> bool {
    ["alpha", "beta", "pre", "rc"].iter().any(|tag| version.contains(tag))
}

fn url_part(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// A loader list, cached for an hour (Forge's maven metadata is large).
async fn cached_text(paths: &Paths, name: &str, url: &str) -> Result<String, String> {
    let file = paths.cache().join(name);
    let fresh = std::fs::metadata(&file)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age.as_secs() < 3600);
    if fresh {
        if let Ok(text) = std::fs::read_to_string(&file) {
            return Ok(text);
        }
    }
    match net::get_text(url).await {
        Ok(text) => {
            let _ = std::fs::create_dir_all(paths.cache());
            let _ = std::fs::write(&file, &text);
            Ok(text)
        }
        Err(e) => std::fs::read_to_string(&file).map_err(|_| e),
    }
}

/// Forge before Minecraft 1.5.2 came as zips to copy into the game jar, with
/// no installer; up to 1.6.4 some builds still lack one.
fn forge_installers(game: &str) -> Option<bool> {
    let mut numbers = game.split('.').map(|p| p.parse::<u32>().ok());
    let (Some(Some(1)), Some(Some(minor))) = (numbers.next(), numbers.next()) else { return Some(true) };
    let patch = numbers.next().flatten().unwrap_or(0);
    match (minor, patch) {
        (0..=4, _) | (5, 0..=1) => None,
        (5, _) | (6, _) => Some(false),
        _ => Some(true),
    }
}

/// Whether a Forge build has an installer, from the file list Forge's site
/// keeps for each build. A list that can't be read counts as yes.
async fn forge_build_has_installer(paths: &Paths, version: String) -> Option<String> {
    let url = format!("https://files.minecraftforge.net/net/minecraftforge/forge/{version}/meta.json");
    let meta: serde_json::Value = cached_text(paths, &format!("forge-meta-{version}.json"), &url).await.ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    meta.get("classifiers").is_none_or(|c| c.get("installer").is_some()).then_some(version)
}

async fn forge_versions(paths: &Paths, game: &str) -> Result<Vec<LoaderVersion>, String> {
    let Some(every_build) = forge_installers(game) else { return Ok(Vec::new()) };
    let xml = cached_text(paths, "forge-maven-metadata.xml", "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml").await?;
    let promos: serde_json::Value = cached_text(paths, "forge-promotions.json", "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json")
        .await
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let recommended = promos["promos"][format!("{game}-recommended")].as_str().map(str::to_string);
    let re = regex::Regex::new(r"<version>([^<]+)</version>").expect("valid regex");
    let prefix = format!("{game}-");
    let mut out: Vec<LoaderVersion> = re
        .captures_iter(&xml)
        .map(|c| c[1].to_string())
        .filter(|v| v.starts_with(&prefix))
        .map(|v| {
            let short = v[prefix.len()..].split('-').next().unwrap_or("").to_string();
            LoaderVersion { recommended: recommended.as_deref() == Some(short.as_str()), label: short, version: v, stable: true }
        })
        .collect();
    if !every_build {
        use futures_util::StreamExt;
        let builds: Vec<String> = out.iter().map(|v| v.version.clone()).collect();
        let with_installer: std::collections::HashSet<String> = futures_util::stream::iter(builds)
            .map(|build| forge_build_has_installer(paths, build))
            .buffer_unordered(8)
            .filter_map(|v| async move { v })
            .collect()
            .await;
        out.retain(|v| with_installer.contains(&v.version));
    }
    out.sort_by(|a, b| version_cmp(&b.label, &a.label));
    Ok(out)
}

/// The Minecraft version a NeoForge version is for: `21.1.77` is 1.21.1,
/// `20.4.237` is 1.20.4, `21.0.x` is 1.21, and from Minecraft 26 on the
/// numbers match the game's (`26.3.0.97` is 26.3).
pub fn neoforge_game(version: &str) -> Option<String> {
    let numbers: Vec<&str> = version.split(['.', '-', '+']).collect();
    let a: u32 = numbers.first()?.parse().ok()?;
    let b: u32 = numbers.get(1)?.parse().ok()?;
    if a >= 26 {
        let c: u32 = numbers.get(2).and_then(|c| c.parse().ok()).unwrap_or(0);
        Some(if c == 0 { format!("{a}.{b}") } else { format!("{a}.{b}.{c}") })
    } else {
        Some(if b == 0 { format!("1.{a}") } else { format!("1.{a}.{b}") })
    }
}

#[derive(Deserialize)]
struct NeoVersions {
    versions: Vec<String>,
}

async fn neoforge_versions(paths: &Paths, game: &str) -> Result<Vec<LoaderVersion>, String> {
    // 1.20.1 is NeoForge's fork of Forge, published under its old name.
    let (cache, url, own) = if game == "1.20.1" {
        ("neoforge-forge-1.20.1.json", "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/forge", false)
    } else {
        ("neoforge-versions.json", "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge", true)
    };
    let text = cached_text(paths, cache, url).await?;
    let list: NeoVersions = serde_json::from_str(&text).map_err(|e| format!("NeoForge's version list couldn't be read: {e}"))?;
    let mut out: Vec<LoaderVersion> = list
        .versions
        .into_iter()
        .filter(|v| if own { neoforge_game(v).as_deref() == Some(game) } else { v.starts_with("1.20.1-") })
        .map(|v| {
            let label = v.strip_prefix("1.20.1-").unwrap_or(&v).to_string();
            LoaderVersion { stable: !v.contains("beta") && !v.contains("alpha"), recommended: false, label, version: v }
        })
        .collect();
    out.sort_by(|a, b| version_cmp(&b.label, &a.label));
    if let Some(first) = out.iter_mut().find(|v| v.stable) {
        first.recommended = true;
    }
    Ok(out)
}

/// Whether loader version `a` is the same as or newer than `b`.
pub fn newer_or_same(a: &str, b: &str) -> bool {
    version_cmp(a, b) != std::cmp::Ordering::Less
}

/// Numeric-aware ordering for dotted versions, where a pre-release comes
/// before its release (`0.30.0-beta.8` < `0.30.0`) and `+build` breaks ties.
fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    // Numbers compare as numbers, words as words, and a number before a word.
    fn parts(s: &str) -> Vec<Result<u64, &str>> {
        s.split(['.', '-']).filter(|p| !p.is_empty()).map(|p| p.parse::<u64>().map_err(|_| p)).collect()
    }
    let pieces = |s: &str| {
        let (main, build) = s.split_once('+').unwrap_or((s, ""));
        let (release, pre) = main.split_once('-').unwrap_or((main, ""));
        (release.to_string(), pre.to_string(), build.to_string())
    };
    let (ra, pa, ba) = pieces(a);
    let (rb, pb, bb) = pieces(b);
    let release = |s: &str| s.split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    release(&ra)
        .cmp(&release(&rb))
        .then_with(|| match (pa.is_empty(), pb.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => parts(&pa).cmp(&parts(&pb)),
        })
        .then_with(|| parts(&ba).cmp(&parts(&bb)))
}

/* ── Installing ─────────────────────────────────────────── */

/// Install a loader version for `game` (already installed as vanilla) and
/// return the id of the version file to start.
pub async fn install(paths: &Paths, kind: &LoaderKind, game: &str, version: &str, java: &Path, task: &TaskHandle, parallel: usize) -> Result<String, String> {
    match kind {
        LoaderKind::Vanilla => Ok(game.to_string()),
        LoaderKind::Fabric => install_profile_json(paths, "https://meta.fabricmc.net/v2", "Fabric", game, version, task).await,
        LoaderKind::Quilt => install_profile_json(paths, "https://meta.quiltmc.org/v3", "Quilt", game, version, task).await,
        LoaderKind::Forge => {
            let url = format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{version}/forge-{version}-installer.jar");
            run_installer(paths, "Forge", game, &url, java, task, parallel).await
        }
        LoaderKind::NeoForge => {
            let url = if version.starts_with("1.20.1-") {
                format!("https://maven.neoforged.net/releases/net/neoforged/forge/{version}/forge-{version}-installer.jar")
            } else {
                format!("https://maven.neoforged.net/releases/net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar")
            };
            run_installer(paths, "NeoForge", game, &url, java, task, parallel).await
        }
    }
}

async fn install_profile_json(paths: &Paths, base: &str, name: &str, game: &str, version: &str, task: &TaskHandle) -> Result<String, String> {
    task.stage(&format!("{name} {version}"));
    let url = format!("{base}/versions/loader/{}/{}/profile/json", url_part(game), url_part(version));
    let json: serde_json::Value = net::get_json(&url).await.map_err(|e| format!("Couldn't get {name} {version} for Minecraft {game}. {e}"))?;
    let id = json["id"].as_str().ok_or_else(|| format!("{name}'s version file has no id."))?.to_string();
    std::fs::create_dir_all(paths.version_dir(&id)).map_err(|e| e.to_string())?;
    std::fs::write(paths.version_json(&id), serde_json::to_vec_pretty(&json).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Couldn't save {name}'s version file: {e}"))?;
    Ok(id)
}

async fn run_installer(paths: &Paths, name: &str, game: &str, url: &str, java: &Path, task: &TaskHandle, parallel: usize) -> Result<String, String> {
    task.stage(&format!("{name} installer"));
    let file_name = url.rsplit('/').next().unwrap_or("installer.jar").to_string();
    let installer = paths.cache().join("installers").join(&file_name);
    download::download_all(vec![FileJob { url: url.into(), path: installer.clone(), sha1: None, size: None }], Verify::Size, task, parallel)
        .await
        .map_err(|e| {
            if e.contains("404") {
                format!("This {name} build for Minecraft {game} was never released with an installer, so the launcher can't set it up. Pick another {name} version, or another loader, in the instance's settings.")
            } else {
                e
            }
        })?;

    let (profile, version_json) = read_installer(&installer)?;
    // Oldest format: the version file is inside install_profile.json.
    if let Some(info) = profile.get("versionInfo") {
        return install_legacy(paths, name, &installer, &profile, info);
    }
    let version_json = version_json.ok_or_else(|| format!("This {name} installer has no version file."))?;
    let id = version_json["id"].as_str().ok_or_else(|| format!("{name}'s version file has no id."))?.to_string();

    // The installers want the official launcher's profile list to exist.
    let profiles = paths.root.join("launcher_profiles.json");
    if !profiles.exists() {
        std::fs::write(&profiles, r#"{"profiles":{}}"#).map_err(|e| e.to_string())?;
    }

    task.stage(&format!("Running the {name} installer"));
    let mut command = tokio::process::Command::new(java);
    command
        .arg("-jar")
        .arg(&installer)
        .arg("--installClient")
        .arg(&paths.root)
        // The installer writes its log beside where it runs.
        .current_dir(installer.parent().unwrap_or(&paths.root))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    crate::java::no_window(&mut command);
    let mut child = command.spawn().map_err(|e| format!("Couldn't start the {name} installer: {e}"))?;
    let stdout = child.stdout.take().expect("piped");
    let stderr = child.stderr.take().expect("piped");
    let tail = std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::<String>::new()));
    let collect = |stream: Box<dyn tokio::io::AsyncRead + Unpin + Send>, tail: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<String>>>| async move {
        let mut lines = BufReader::new(stream).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let mut t = tail.lock().unwrap();
            t.push_back(line);
            if t.len() > 40 {
                t.pop_front();
            }
        }
    };
    let out_task = tokio::spawn(collect(Box::new(stdout), tail.clone()));
    let err_task = tokio::spawn(collect(Box::new(stderr), tail.clone()));
    let status = loop {
        if task.cancelled() {
            let _ = child.kill().await;
            return Err(crate::tasks::CANCELLED.into());
        }
        match tokio::time::timeout(std::time::Duration::from_millis(500), child.wait()).await {
            Ok(status) => break status.map_err(|e| e.to_string())?,
            Err(_) => continue,
        }
    };
    let _ = out_task.await;
    let _ = err_task.await;
    let log: Vec<String> = tail.lock().unwrap().iter().cloned().collect();
    let log_text = log.join("\n");
    if !status.success() || !paths.version_json(&id).exists() {
        let unknown_option = log_text.contains("installClient") && (log_text.contains("not a recognized option") || log_text.contains("UnrecognizedOption"));
        let hint = if unknown_option {
            format!("This {name} installer is too old to run on its own.")
        } else if log_text.contains("Failed to download") || log_text.contains("UnknownHost") {
            format!("The {name} installer couldn't download its files. Check your connection and try again.")
        } else {
            format!("The {name} installer stopped (exit code {}).", status.code().unwrap_or(-1))
        };
        let last: Vec<&str> = log.iter().rev().take(8).map(String::as_str).collect::<Vec<_>>().into_iter().rev().collect();
        return Err(format!("{hint}\n\n{}", last.join("\n")));
    }
    copy_client_jar(paths, game, &id)?;
    Ok(id)
}

fn read_installer(installer: &Path) -> Result<(serde_json::Value, Option<serde_json::Value>), String> {
    let file = std::fs::File::open(installer).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| "The downloaded installer is damaged. Try again.".to_string())?;
    let mut read = |name: &str| -> Option<serde_json::Value> {
        let mut entry = zip.by_name(name).ok()?;
        let mut text = String::new();
        entry.read_to_string(&mut text).ok()?;
        serde_json::from_str(&text).ok()
    };
    let profile = read("install_profile.json").ok_or("The installer has no install_profile.json.")?;
    let version = read("version.json");
    Ok((profile, version))
}

/// The oldest Forge installers: write the version file, unpack the Forge jar
/// into the libraries, copy the game jar. Libraries are fetched at launch.
fn install_legacy(paths: &Paths, name: &str, installer: &Path, profile: &serde_json::Value, info: &serde_json::Value) -> Result<String, String> {
    let id = info["id"].as_str().ok_or_else(|| format!("{name}'s version file has no id."))?.to_string();
    let game = profile["install"]["minecraft"].as_str().or_else(|| info["inheritsFrom"].as_str()).unwrap_or_default().to_string();
    let mut info = info.clone();
    if info.get("inheritsFrom").is_none() && !game.is_empty() {
        info["inheritsFrom"] = serde_json::Value::String(game.clone());
    }
    if info.get("jar").is_none() && !game.is_empty() {
        info["jar"] = serde_json::Value::String(game.clone());
    }
    std::fs::create_dir_all(paths.version_dir(&id)).map_err(|e| e.to_string())?;
    std::fs::write(paths.version_json(&id), serde_json::to_vec_pretty(&info).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;

    let coords = profile["install"]["path"].as_str().ok_or("The installer doesn't say where Forge goes.")?;
    let inner = profile["install"]["filePath"].as_str().ok_or("The installer doesn't name its Forge jar.")?;
    let target = paths.library(&crate::meta::maven_path(coords).ok_or("The installer names Forge oddly.")?);
    if !target.exists() {
        let file = std::fs::File::open(installer).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        let mut entry = zip.by_name(inner).map_err(|_| "The installer is missing its Forge jar.".to_string())?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(target.parent().expect("library path has a parent")).map_err(|e| e.to_string())?;
        std::fs::write(&target, bytes).map_err(|e| e.to_string())?;
    }
    // The Forge jar is unpacked, not downloaded: point its library entry at
    // nothing so nothing tries to fetch it.
    let mut info: serde_json::Value = serde_json::from_slice(&std::fs::read(paths.version_json(&id)).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if let Some(libs) = info["libraries"].as_array_mut() {
        for lib in libs {
            if lib["name"].as_str() == Some(coords) {
                lib["downloads"] = serde_json::json!({ "artifact": { "path": crate::meta::maven_path(coords), "url": "" } });
                if let Some(obj) = lib.as_object_mut() {
                    obj.remove("url");
                }
            }
        }
    }
    std::fs::write(paths.version_json(&id), serde_json::to_vec_pretty(&info).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    copy_client_jar(paths, &game, &id)?;
    Ok(id)
}

/// The official launcher runs a loader version with a copy of the game jar
/// named after it, and Forge's module rules (`-DignoreList=...${version_name}.jar`)
/// count on that name.
fn copy_client_jar(paths: &Paths, game: &str, id: &str) -> Result<(), String> {
    let own = paths.version_jar(id);
    let vanilla = paths.version_jar(game);
    let same = match (std::fs::metadata(&own), std::fs::metadata(&vanilla)) {
        (Ok(a), Ok(b)) => a.len() == b.len(),
        _ => false,
    };
    if !same && vanilla.exists() {
        std::fs::copy(&vanilla, &own).map_err(|e| format!("Couldn't copy the game jar for {id}: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neoforge_versions_map_to_game_versions() {
        assert_eq!(neoforge_game("21.1.77").as_deref(), Some("1.21.1"));
        assert_eq!(neoforge_game("21.0.167").as_deref(), Some("1.21"));
        assert_eq!(neoforge_game("20.4.237").as_deref(), Some("1.20.4"));
        assert_eq!(neoforge_game("20.2.3-beta").as_deref(), Some("1.20.2"));
        assert_eq!(neoforge_game("26.3.0.97").as_deref(), Some("26.3"));
        assert_eq!(neoforge_game("26.1.1.4").as_deref(), Some("26.1.1"));
        assert_ne!(neoforge_game("0.25w14craftmine.3-beta").as_deref(), Some("26.3"));
    }

    #[test]
    fn versions_sort_numerically() {
        let mut v = vec!["47.1.9", "47.1.10", "47.2.0"];
        v.sort_by(|a, b| version_cmp(b, a));
        assert_eq!(v, ["47.2.0", "47.1.10", "47.1.9"]);
        // Quilt's list for 26.3, in the order its server gives it.
        let mut q = vec!["0.20.0-beta.9", "0.24.0", "0.30.0-beta.8", "0.30.0-beta.10", "0.29.2", "0.30.0", "0.31.0-beta.4"];
        q.sort_by(|a, b| version_cmp(b, a));
        assert_eq!(q, ["0.31.0-beta.4", "0.30.0", "0.30.0-beta.10", "0.30.0-beta.8", "0.29.2", "0.24.0", "0.20.0-beta.9"]);
        let mut f = vec!["0.4.0+build.105", "0.4.0+build.106", "0.3.7"];
        f.sort_by(|a, b| version_cmp(b, a));
        assert_eq!(f, ["0.4.0+build.106", "0.4.0+build.105", "0.3.7"]);
    }

    #[test]
    fn forge_before_its_installer() {
        assert_eq!(forge_installers("1.1"), None);
        assert_eq!(forge_installers("1.4.7"), None);
        assert_eq!(forge_installers("1.5.1"), None);
        assert_eq!(forge_installers("1.5.2"), Some(false));
        assert_eq!(forge_installers("1.6.4"), Some(false));
        assert_eq!(forge_installers("1.7.10"), Some(true));
        assert_eq!(forge_installers("26.3"), Some(true));
    }
}
