//! Modrinth modpacks (`.mrpack`), the file the Modrinth app exports and
//! servers hand their players: a zip with `modrinth.index.json` (the
//! Minecraft version, the loader and every mod's download) and `overrides/`
//! (configs and anything else to copy into the game folder as it is).
//!
//! Importing one makes an instance in two steps. `create` reads the pack,
//! checks every path in it and makes the instance, keeping a copy of the pack
//! beside it; `add_files` downloads the pack's files and copies its overrides
//! in, as a task that can be tried again. Installing the game and the loader
//! is then the instance's normal install.
//!
//! Format: <https://support.modrinth.com/en/articles/8802351-modrinth-modpack-format-mrpack>

use std::collections::HashMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::content::Kind;
use crate::core::Core;
use crate::download::{self, FileJob, Verify};
use crate::instances::{self, Instance, Loader, LoaderKind};
use crate::paths::Paths;
use crate::{loaders, meta, modrinth, settings};

/// The copy of the pack an imported instance keeps, for "Try again".
const KEPT: &str = "modpack.mrpack";

/// Where a pack's files may come from (the format's own list).
const HOSTS: &[&str] = &["cdn.modrinth.com", "github.com", "raw.githubusercontent.com", "gitlab.com"];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    #[serde(default)]
    pub format_version: u32,
    #[serde(default)]
    pub game: String,
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub files: Vec<IndexFile>,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexFile {
    pub path: String,
    #[serde(default)]
    pub hashes: HashMap<String, String>,
    #[serde(default)]
    pub env: Option<Env>,
    #[serde(default)]
    pub downloads: Vec<String>,
    #[serde(default)]
    pub file_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct Env {
    #[serde(default)]
    pub client: Option<String>,
}

impl IndexFile {
    /// Server-only files (a server's own tools) stay out of a player's game.
    fn for_client(&self) -> bool {
        self.env.as_ref().and_then(|e| e.client.as_deref()) != Some("unsupported")
    }
}

/// A path from a pack, relative to the game folder, or `None` when it would
/// land anywhere else (`..`, an absolute path, a drive letter).
fn inside(relative: &str) -> Option<PathBuf> {
    let path = Path::new(relative);
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::Normal(p) if !p.to_string_lossy().contains(':') => out.push(p),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

pub fn read(file: &Path) -> Result<Index, String> {
    let zip_file = std::fs::File::open(file).map_err(|e| format!("Couldn't open the modpack: {e}"))?;
    let mut zip = zip::ZipArchive::new(zip_file).map_err(|_| "That file isn't a Modrinth modpack (.mrpack).".to_string())?;
    let mut entry = zip.by_name("modrinth.index.json").map_err(|_| "That file isn't a Modrinth modpack: it has no modrinth.index.json.".to_string())?;
    let mut text = String::new();
    entry.read_to_string(&mut text).map_err(|e| format!("The modpack's index couldn't be read: {e}"))?;
    let index: Index = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("The modpack's index is damaged: {e}"))?;
    if index.game != "minecraft" {
        return Err("That modpack isn't for Minecraft.".into());
    }
    if index.format_version != 1 {
        return Err(format!("That modpack uses format {}, which this launcher doesn't know yet.", index.format_version));
    }
    for f in index.files.iter().filter(|f| f.for_client()) {
        if inside(&f.path).is_none() {
            return Err(format!("The modpack wants to put a file outside its instance ({}), so it wasn't imported.", f.path));
        }
        let url = f.downloads.first().ok_or_else(|| format!("The modpack has no download for {}.", f.path))?;
        let host = url::Url::parse(url).ok().filter(|u| u.scheme() == "https").and_then(|u| u.host_str().map(str::to_string));
        if !host.as_deref().is_some_and(|h| HOSTS.contains(&h)) {
            return Err(format!("The modpack downloads {} from {}, which Modrinth packs may not use, so it wasn't imported.", f.path, crate::net::host(url)));
        }
        if !f.hashes.contains_key("sha1") {
            return Err(format!("The modpack has no checksum for {}, so it wasn't imported.", f.path));
        }
    }
    Ok(index)
}

/// The loader a pack names, and its version.
fn loader_of(dependencies: &HashMap<String, String>) -> (LoaderKind, Option<String>) {
    for (key, kind) in [("neoforge", LoaderKind::NeoForge), ("forge", LoaderKind::Forge), ("quilt-loader", LoaderKind::Quilt), ("fabric-loader", LoaderKind::Fabric)] {
        if let Some(v) = dependencies.get(key) {
            return (kind, Some(v.clone()));
        }
    }
    (LoaderKind::Vanilla, None)
}

/// Make an instance from a pack. Nothing is downloaded yet.
pub async fn create(paths: &Paths, file: &Path) -> Result<Instance, String> {
    let index = read(file)?;
    let game = index.dependencies.get("minecraft").cloned().ok_or("The modpack doesn't say which Minecraft it's for.")?;
    let manifest = meta::manifest(paths, false).await?;
    let version_type = manifest
        .versions
        .iter()
        .find(|v| v.id == game)
        .map(|v| v.kind.clone())
        .ok_or_else(|| format!("The modpack is for Minecraft {game}, which isn't in Mojang's version list."))?;
    let (kind, wanted) = loader_of(&index.dependencies);
    // Forge's installers are named by their full maven version
    // (`1.20.1-47.2.0`); packs give the short one.
    let version = match (&kind, wanted) {
        (LoaderKind::Forge | LoaderKind::NeoForge, Some(v)) => {
            let all = loaders::versions(paths, &kind, &game).await?;
            let found = all.into_iter().find(|l| l.label == v || l.version == v).map(|l| l.version);
            Some(found.ok_or_else(|| format!("{} {v} for Minecraft {game} couldn't be found.", kind.label()))?)
        }
        (_, v) => v,
    };
    let instance = instances::create(paths, &index.name, &game, &version_type, Loader { kind, version }, None)?;
    if let Err(e) = std::fs::copy(file, paths.instance(&instance.id).join(KEPT)) {
        let _ = instances::delete(paths, &instance.id);
        return Err(format!("Couldn't keep a copy of the modpack: {e}"));
    }
    Ok(instance)
}

/// Download a pack instance's files and copy its overrides in, as a task.
pub async fn add_files(core: &Core, id: &str) -> Result<(), String> {
    let instance = instances::get(&core.paths, id)?;
    if let Some(title) = core.tasks.running_for(id) {
        return Err(format!("Wait for \"{title}\" to finish first."));
    }
    let task = core.tasks.start(
        &format!("Adding {}'s mods", instance.name),
        Some(id),
        Some(serde_json::json!({ "command": "modpack_files", "args": { "id": id } })),
    )?;
    let result = add_files_in(core, &instance, &task).await;
    task.finish(&result);
    (core.emit_instances)();
    result
}

async fn add_files_in(core: &Core, instance: &Instance, task: &crate::tasks::TaskHandle) -> Result<(), String> {
    let pack = core.paths.instance(&instance.id).join(KEPT);
    let index = read(&pack)?;
    let game_dir = core.paths.game_dir(&instance.id);
    let files: Vec<&IndexFile> = index.files.iter().filter(|f| f.for_client()).collect();
    let jobs: Vec<FileJob> = files
        .iter()
        .filter_map(|f| {
            Some(FileJob { url: f.downloads.first()?.clone(), path: game_dir.join(inside(&f.path)?), sha1: f.hashes.get("sha1").cloned(), size: f.file_size })
        })
        .collect();
    task.stage("Mods");
    let parallel = settings::load(&core.paths).parallel_downloads.clamp(1, 16) as usize;
    download::download_all(jobs, Verify::Size, task, parallel).await?;

    task.stage("Settings and configs");
    let dir = game_dir.clone();
    tokio::task::spawn_blocking(move || copy_overrides(&pack, &dir)).await.map_err(|e| e.to_string())??;

    // So the mods show their Modrinth names and icons. Best effort: the pack
    // is complete without it.
    let known: Vec<(Kind, String, String)> = files
        .iter()
        .filter_map(|f| {
            let (folder, name) = f.path.split_once('/')?;
            let kind = match folder {
                "mods" => Kind::Mod,
                "resourcepacks" => Kind::ResourcePack,
                "shaderpacks" => Kind::ShaderPack,
                _ => return None,
            };
            (!name.contains('/')).then(|| Some((kind, name.to_string(), f.hashes.get("sha1")?.clone())))?
        })
        .collect();
    task.stage("Mod names");
    modrinth::remember_by_hash(&core.paths, &instance.id, &known).await;
    Ok(())
}

/// `overrides/`, then `client-overrides/` over it, into the game folder.
fn copy_overrides(pack: &Path, game_dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(pack).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    for prefix in ["overrides", "client-overrides"] {
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            let Some(name) = entry.enclosed_name() else { continue };
            let Ok(relative) = name.strip_prefix(prefix) else { continue };
            let Some(relative) = inside(&relative.to_string_lossy()) else { continue };
            let target = game_dir.join(relative);
            if entry.is_dir() {
                std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
                continue;
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out = std::fs::File::create(&target).map_err(|e| format!("Couldn't write {}: {e}", target.display()))?;
            std::io::copy(&mut entry, &mut out).map_err(|e| format!("Couldn't write {}: {e}", target.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_paths_stay_in_the_game_folder() {
        assert_eq!(inside("mods/sodium.jar"), Some(PathBuf::from("mods").join("sodium.jar")));
        assert_eq!(inside("./config/a.toml"), Some(PathBuf::from("config").join("a.toml")));
        assert_eq!(inside("../evil.jar"), None);
        assert_eq!(inside("mods/../../evil.jar"), None);
        assert_eq!(inside("/etc/passwd"), None);
        assert_eq!(inside("C:/Windows/x.dll"), None);
        assert_eq!(inside(""), None);
    }

    #[test]
    fn loaders_from_dependencies() {
        let deps = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<HashMap<_, _>>();
        assert_eq!(loader_of(&deps(&[("minecraft", "1.21.1"), ("fabric-loader", "0.16.10")])), (LoaderKind::Fabric, Some("0.16.10".into())));
        assert_eq!(loader_of(&deps(&[("minecraft", "1.20.1"), ("forge", "47.2.0")])), (LoaderKind::Forge, Some("47.2.0".into())));
        assert_eq!(loader_of(&deps(&[("minecraft", "26.3"), ("neoforge", "26.3.0.99")])), (LoaderKind::NeoForge, Some("26.3.0.99".into())));
        assert_eq!(loader_of(&deps(&[("minecraft", "1.21.1")])), (LoaderKind::Vanilla, None));
    }

    fn pack(index: &serde_json::Value, extra: &[(&str, &str)]) -> PathBuf {
        use std::io::Write;
        let path = std::env::temp_dir().join(format!("kryoto-mrpack-{}-{}.mrpack", std::process::id(), crate::tasks::now() as u128 * 1000 + extra.len() as u128 + index.to_string().len() as u128));
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("modrinth.index.json", options).unwrap();
        zip.write_all(index.to_string().as_bytes()).unwrap();
        for (name, body) in extra {
            zip.start_file(*name, options).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    #[test]
    fn reads_a_pack_and_refuses_bad_ones() {
        let file = |path: &str, url: &str| serde_json::json!({ "path": path, "hashes": { "sha1": "00", "sha512": "00" }, "downloads": [url], "fileSize": 1 });
        let good = serde_json::json!({
            "formatVersion": 1, "game": "minecraft", "versionId": "1", "name": "Friends",
            "files": [file("mods/sodium.jar", "https://cdn.modrinth.com/data/x/sodium.jar"),
                      { "path": "mods/server-only.jar", "hashes": {}, "downloads": ["https://example.com/x.jar"], "env": { "client": "unsupported", "server": "required" } }],
            "dependencies": { "minecraft": "1.21.1", "fabric-loader": "0.16.10" }
        });
        let path = pack(&good, &[("overrides/config/sodium.json", "{}"), ("client-overrides/options.txt", "fov:1")]);
        let index = read(&path).unwrap();
        assert_eq!(index.name, "Friends");
        assert_eq!(index.files.iter().filter(|f| f.for_client()).count(), 1);

        let out = std::env::temp_dir().join(format!("kryoto-mrpack-out-{}", std::process::id()));
        copy_overrides(&path, &out).unwrap();
        assert_eq!(std::fs::read_to_string(out.join("config").join("sodium.json")).unwrap(), "{}");
        assert_eq!(std::fs::read_to_string(out.join("options.txt")).unwrap(), "fov:1");
        let _ = std::fs::remove_dir_all(&out);
        let _ = std::fs::remove_file(&path);

        let mut escape = good.clone();
        escape["files"][0]["path"] = "../../evil.jar".into();
        let path = pack(&escape, &[]);
        assert!(read(&path).unwrap_err().contains("outside"));
        let _ = std::fs::remove_file(&path);

        let mut elsewhere = good.clone();
        elsewhere["files"][0]["downloads"][0] = "https://evil.example/sodium.jar".into();
        let path = pack(&elsewhere, &[("x", "")]);
        assert!(read(&path).unwrap_err().contains("evil.example"));
        let _ = std::fs::remove_file(&path);
    }
}
