//! What is in an instance's game folder: mods, resource packs, shader packs
//! and worlds, read from the files themselves.
//!
//! Turning a mod off renames it to `.disabled`, the convention every loader
//! and launcher understands. Files added from Modrinth are remembered in
//! `content.json` beside the instance, so their pages, icons and
//! dependencies are known; files dropped in by hand are listed all the same.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths::Paths;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Mod,
    ResourcePack,
    ShaderPack,
    World,
}

impl Kind {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "mod" => Ok(Self::Mod),
            "resourcepack" => Ok(Self::ResourcePack),
            "shaderpack" => Ok(Self::ShaderPack),
            "world" => Ok(Self::World),
            _ => Err(format!("Unknown kind of content: {s}")),
        }
    }
}

/// The folder for a kind of content. Versions before 1.6 call resource packs
/// texture packs and read them from another folder.
pub fn folder(kind: Kind, game_version: &str, version_type: &str) -> &'static str {
    match kind {
        Kind::Mod => "mods",
        Kind::ShaderPack => "shaderpacks",
        Kind::World => "saves",
        Kind::ResourcePack if uses_texture_packs(game_version, version_type) => "texturepacks",
        Kind::ResourcePack => "resourcepacks",
    }
}

fn uses_texture_packs(game_version: &str, version_type: &str) -> bool {
    if version_type == "old_beta" || version_type == "old_alpha" {
        return true;
    }
    let mut parts = game_version.split('.');
    matches!((parts.next(), parts.next().and_then(|m| m.parse::<u32>().ok())), (Some("1"), Some(minor)) if minor < 6)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub version_number: Option<String>,
}

/// `content.json`: file name (in its folder) to where it came from.
pub type Index = HashMap<String, Source>;

fn index_path(paths: &Paths, instance: &str) -> PathBuf {
    paths.instance(instance).join("content.json")
}

pub fn read_index(paths: &Paths, instance: &str) -> Index {
    std::fs::read(index_path(paths, instance)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn write_index(paths: &Paths, instance: &str, index: &Index) -> Result<(), String> {
    std::fs::write(index_path(paths, instance), serde_json::to_vec_pretty(index).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

pub(crate) fn index_key(kind: Kind, file: &str) -> String {
    format!("{kind:?}/{}", file.trim_end_matches(".disabled")).to_lowercase()
}

pub fn remember(paths: &Paths, instance: &str, kind: Kind, file: &str, source: Source) -> Result<(), String> {
    let mut index = read_index(paths, instance);
    index.insert(index_key(kind, file), source);
    write_index(paths, instance, &index)
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub file: String,
    pub kind: Kind,
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Option<String>,
    pub enabled: bool,
    pub size: u64,
    pub modified: u64,
    /// A data: URL of the pack's or mod's own icon.
    pub icon: Option<String>,
    pub source: Option<Source>,
}

pub fn list(paths: &Paths, instance: &str, kind: Kind, game_version: &str, version_type: &str) -> Vec<Item> {
    let dir = paths.game_dir(instance).join(folder(kind, game_version, version_type));
    let index = read_index(paths, instance);
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut items: Vec<Item> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let file = e.file_name().to_string_lossy().into_owned();
            let meta = e.metadata().ok()?;
            let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            let lower = file.to_lowercase();
            let (enabled, wanted) = match kind {
                Kind::Mod => (!lower.ends_with(".disabled"), lower.trim_end_matches(".disabled").ends_with(".jar")),
                Kind::ResourcePack | Kind::ShaderPack => {
                    (!lower.ends_with(".disabled"), meta.is_dir() || lower.trim_end_matches(".disabled").ends_with(".zip"))
                }
                Kind::World => (true, meta.is_dir() && path.join("level.dat").exists()),
            };
            if !wanted {
                return None;
            }
            let mut item = Item {
                name: file.trim_end_matches(".disabled").trim_end_matches(".jar").trim_end_matches(".zip").to_string(),
                file: file.clone(),
                kind,
                version: None,
                description: None,
                authors: None,
                enabled,
                size: if meta.is_dir() { 0 } else { meta.len() },
                modified,
                icon: None,
                source: index.get(&index_key(kind, &file)).cloned(),
            };
            match kind {
                Kind::Mod => read_mod(&path, &mut item),
                Kind::ResourcePack => read_pack(&path, &mut item),
                Kind::World => item.icon = std::fs::read(path.join("icon.png")).ok().filter(|b| b.len() < 256 * 1024).map(|b| data_url(&b)),
                Kind::ShaderPack => {}
            }
            if let Some(source) = &item.source {
                if item.icon.is_none() {
                    item.icon = source.icon_url.clone();
                }
                if item.name == file.trim_end_matches(".disabled").trim_end_matches(".jar").trim_end_matches(".zip") {
                    item.name = source.title.clone();
                }
            }
            Some(item)
        })
        .collect();
    items.sort_by(|a, b| {
        if kind == Kind::World {
            b.modified.cmp(&a.modified)
        } else {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        }
    });
    items
}

fn data_url(png: &[u8]) -> String {
    use base64::Engine;
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))
}

fn zip_text(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<String> {
    let mut entry = zip.by_name(name).ok()?;
    if entry.size() > 1024 * 1024 {
        return None;
    }
    let mut text = String::new();
    entry.read_to_string(&mut text).ok()?;
    Some(text)
}

fn zip_bytes(zip: &mut zip::ZipArchive<std::fs::File>, name: &str, max: u64) -> Option<Vec<u8>> {
    let mut entry = zip.by_name(name.trim_start_matches('/')).ok()?;
    if entry.size() > max {
        return None;
    }
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn read_mod(path: &Path, item: &mut Item) {
    let Ok(file) = std::fs::File::open(path) else { return };
    let Ok(mut zip) = zip::ZipArchive::new(file) else { return };
    if let Some(json) = zip_text(&mut zip, "fabric.mod.json").and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
        if let Some(name) = json["name"].as_str() {
            item.name = name.into();
        }
        item.version = json["version"].as_str().map(str::to_string);
        item.description = json["description"].as_str().map(str::to_string);
        item.authors = json["authors"].as_array().map(|a| {
            a.iter().filter_map(|x| x.as_str().or_else(|| x["name"].as_str())).collect::<Vec<_>>().join(", ")
        });
        let icon = json["icon"].as_str().map(str::to_string).or_else(|| json["icon"].as_object().and_then(|o| o.values().next_back()).and_then(|v| v.as_str()).map(str::to_string));
        item.icon = icon.and_then(|i| zip_bytes(&mut zip, &i, 256 * 1024)).map(|b| data_url(&b));
        return;
    }
    if let Some(json) = zip_text(&mut zip, "quilt.mod.json").and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
        let q = &json["quilt_loader"];
        if let Some(name) = q["metadata"]["name"].as_str() {
            item.name = name.into();
        }
        item.version = q["version"].as_str().map(str::to_string);
        item.description = q["metadata"]["description"].as_str().map(str::to_string);
        item.icon = q["metadata"]["icon"].as_str().and_then(|i| zip_bytes(&mut zip, i, 256 * 1024)).map(|b| data_url(&b));
        return;
    }
    let toml = zip_text(&mut zip, "META-INF/neoforge.mods.toml").or_else(|| zip_text(&mut zip, "META-INF/mods.toml"));
    if let Some(toml) = toml {
        let field = |key: &str| {
            regex::Regex::new(&format!(r#"(?m)^\s*{key}\s*=\s*"([^"]*)""#)).ok()?.captures(&toml).map(|c| c[1].to_string())
        };
        if let Some(name) = field("displayName") {
            item.name = name;
        }
        item.version = field("version").filter(|v| !v.contains("${"));
        item.authors = field("authors");
        item.description = regex::Regex::new(r#"(?s)description\s*=\s*'''(.*?)'''"#)
            .ok()
            .and_then(|r| r.captures(&toml).map(|c| c[1].trim().to_string()))
            .or_else(|| field("description"));
        item.icon = field("logoFile").and_then(|i| zip_bytes(&mut zip, &i, 256 * 1024)).map(|b| data_url(&b));
        return;
    }
    if let Some(info) = zip_text(&mut zip, "mcmod.info").and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
        let first = if info.is_array() { info[0].clone() } else { info["modList"][0].clone() };
        if let Some(name) = first["name"].as_str() {
            item.name = name.into();
        }
        item.version = first["version"].as_str().map(str::to_string);
        item.description = first["description"].as_str().map(str::to_string);
    }
}

fn read_pack(path: &Path, item: &mut Item) {
    let (meta, icon) = if path.is_dir() {
        (std::fs::read_to_string(path.join("pack.mcmeta")).ok(), std::fs::read(path.join("pack.png")).ok())
    } else {
        let Ok(file) = std::fs::File::open(path) else { return };
        let Ok(mut zip) = zip::ZipArchive::new(file) else { return };
        (zip_text(&mut zip, "pack.mcmeta"), zip_bytes(&mut zip, "pack.png", 256 * 1024))
    };
    item.icon = icon.filter(|b| b.len() < 256 * 1024).map(|b| data_url(&b));
    if let Some(json) = meta.and_then(|t| serde_json::from_str::<serde_json::Value>(t.trim_start_matches('\u{feff}')).ok()) {
        item.description = text_component(&json["pack"]["description"]);
    }
}

/// A text component (string, object or list) as plain text, without the `§` codes.
fn text_component(v: &serde_json::Value) -> Option<String> {
    fn raw(v: &serde_json::Value) -> String {
        match v {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(a) => a.iter().map(raw).collect(),
            serde_json::Value::Object(o) => {
                let mut s = o.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string();
                if let Some(extra) = o.get("extra") {
                    s.push_str(&raw(extra));
                }
                s
            }
            _ => String::new(),
        }
    }
    let text = raw(v);
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    Some(out.trim().to_string()).filter(|s| !s.is_empty())
}

fn checked_file(paths: &Paths, instance: &str, kind: Kind, game_version: &str, version_type: &str, file: &str) -> Result<PathBuf, String> {
    if file.contains(['/', '\\']) || file.starts_with('.') {
        return Err("That isn't a file in this instance.".into());
    }
    let path = paths.game_dir(instance).join(folder(kind, game_version, version_type)).join(file);
    if !path.exists() {
        return Err("That file isn't there any more.".into());
    }
    Ok(path)
}

pub fn set_enabled(paths: &Paths, instance: &str, kind: Kind, gv: &str, vt: &str, file: &str, enabled: bool) -> Result<String, String> {
    let path = checked_file(paths, instance, kind, gv, vt, file)?;
    let name = file.to_string();
    let target = if enabled { name.trim_end_matches(".disabled").to_string() } else if name.ends_with(".disabled") { name } else { format!("{name}.disabled") };
    if target == file {
        return Ok(target);
    }
    std::fs::rename(&path, path.with_file_name(&target)).map_err(|e| format!("Couldn't rename {file} (is the game running?): {e}"))?;
    Ok(target)
}

pub fn delete(paths: &Paths, instance: &str, kind: Kind, gv: &str, vt: &str, file: &str) -> Result<(), String> {
    let path = checked_file(paths, instance, kind, gv, vt, file)?;
    let result = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
    result.map_err(|e| format!("Couldn't delete {file} (is the game running?): {e}"))?;
    let mut index = read_index(paths, instance);
    if index.remove(&index_key(kind, file)).is_some() {
        write_index(paths, instance, &index)?;
    }
    Ok(())
}

/// Copy files a player picked or dropped into the right folders: jars are
/// mods, zips with a `pack.mcmeta` resource packs, zips with `shaders/`
/// shader packs. Returns what went where.
pub fn import(paths: &Paths, instance: &str, gv: &str, vt: &str, files: &[PathBuf], only: Option<Kind>) -> Result<Vec<(String, Kind)>, String> {
    let mut done = Vec::new();
    for source in files {
        let name = source.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or("A file has no name.")?;
        let kind = match only {
            Some(kind) => kind,
            None => detect(source).ok_or_else(|| format!("{name} isn't a mod, resource pack or shader pack."))?,
        };
        let dir = paths.game_dir(instance).join(folder(kind, gv, vt));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::copy(source, dir.join(crate::paths::safe_file_name(&name))).map_err(|e| format!("Couldn't copy {name}: {e}"))?;
        done.push((name, kind));
    }
    Ok(done)
}

fn detect(path: &Path) -> Option<Kind> {
    let lower = path.extension()?.to_string_lossy().to_lowercase();
    if lower == "jar" {
        return Some(Kind::Mod);
    }
    if lower != "zip" {
        return None;
    }
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    if zip.by_name("pack.mcmeta").is_ok() {
        return Some(Kind::ResourcePack);
    }
    let shaders = (0..zip.len()).any(|i| zip.by_index(i).map(|e| e.name().starts_with("shaders/")).unwrap_or(false));
    shaders.then_some(Kind::ShaderPack)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_pack_era() {
        assert_eq!(folder(Kind::ResourcePack, "1.5.2", "release"), "texturepacks");
        assert_eq!(folder(Kind::ResourcePack, "b1.7.3", "old_beta"), "texturepacks");
        assert_eq!(folder(Kind::ResourcePack, "1.6.1", "release"), "resourcepacks");
        assert_eq!(folder(Kind::ResourcePack, "26.3", "release"), "resourcepacks");
    }

    #[test]
    fn text_components() {
        let v = serde_json::json!([{"text": "§aGreen "}, "plain"]);
        assert_eq!(text_component(&v).as_deref(), Some("Green plain"));
    }

    #[test]
    fn base64() {
        assert_eq!(data_url(b"hi!"), "data:image/png;base64,aGkh");
        assert_eq!(data_url(b"hi"), "data:image/png;base64,aGk=");
    }
}
