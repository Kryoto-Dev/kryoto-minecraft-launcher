//! Instances: a Minecraft version, maybe a mod loader, and a game folder of
//! its own (saves, mods, packs, options), with launch settings that override
//! the global ones.

use serde::{Deserialize, Serialize};

use crate::paths::Paths;
use crate::tasks::now;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LoaderKind {
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

impl LoaderKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Vanilla => "Vanilla",
            Self::Fabric => "Fabric",
            Self::Quilt => "Quilt",
            Self::Forge => "Forge",
            Self::NeoForge => "NeoForge",
        }
    }
    /// Modrinth's name for it.
    pub fn modrinth(&self) -> Option<&'static str> {
        match self {
            Self::Vanilla => None,
            Self::Fabric => Some("fabric"),
            Self::Quilt => Some("quilt"),
            Self::Forge => Some("forge"),
            Self::NeoForge => Some("neoforge"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Loader {
    pub kind: LoaderKind,
    #[serde(default)]
    pub version: Option<String>,
}

/// Settings an instance can override; `None` follows Settings.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOverrides {
    #[serde(default)]
    pub java_path: Option<String>,
    #[serde(default)]
    pub memory_mb: Option<u32>,
    #[serde(default)]
    pub jvm_args: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub fullscreen: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub name: String,
    /// The Minecraft version (vanilla id).
    pub game_version: String,
    pub loader: Loader,
    /// The version file the game starts from: the vanilla id, or the loader's
    /// (`fabric-loader-0.16.10-1.21.1`). Set once installed.
    #[serde(default)]
    pub launch_version: Option<String>,
    /// "release", "snapshot", "old_beta", "old_alpha".
    #[serde(default)]
    pub version_type: String,
    #[serde(default)]
    pub installed: bool,
    pub created: u64,
    #[serde(default)]
    pub last_played: Option<u64>,
    #[serde(default)]
    pub play_seconds: u64,
    /// One of the launcher's block icons, by name.
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub overrides: LaunchOverrides,
}

impl Instance {
    pub fn label(&self) -> String {
        match &self.loader.kind {
            LoaderKind::Vanilla => self.game_version.clone(),
            kind => format!("{} {}", kind.label(), self.game_version),
        }
    }
}

fn file(paths: &Paths, id: &str) -> std::path::PathBuf {
    paths.instance(id).join("instance.json")
}

pub fn list(paths: &Paths) -> Vec<Instance> {
    let Ok(dir) = std::fs::read_dir(paths.instances()) else { return Vec::new() };
    let mut out: Vec<Instance> = dir
        .flatten()
        .filter_map(|e| std::fs::read(e.path().join("instance.json")).ok())
        .filter_map(|b| serde_json::from_slice(&b).ok())
        .collect();
    out.sort_by_key(|i| std::cmp::Reverse(i.last_played.unwrap_or(i.created)));
    out
}

pub fn get(paths: &Paths, id: &str) -> Result<Instance, String> {
    let bytes = std::fs::read(file(paths, id)).map_err(|_| "That instance doesn't exist any more.".to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| format!("The instance's settings file is damaged: {e}"))
}

pub fn save(paths: &Paths, instance: &Instance) -> Result<(), String> {
    let dir = paths.instance(&instance.id);
    std::fs::create_dir_all(paths.game_dir(&instance.id)).map_err(|e| format!("Couldn't create the instance folder: {e}"))?;
    let tmp = dir.join("instance.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(instance).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, file(paths, &instance.id)).map_err(|e| format!("Couldn't save the instance: {e}"))
}

/// A folder name from the instance's name, unique among the others.
pub fn new_id(paths: &Paths, name: &str) -> String {
    let mut slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    slug.truncate(40);
    if slug.is_empty() {
        slug = "instance".into();
    }
    let mut id = slug.clone();
    let mut n = 2;
    while paths.instance(&id).exists() {
        id = format!("{slug}-{n}");
        n += 1;
    }
    id
}

pub fn create(paths: &Paths, name: &str, game_version: &str, version_type: &str, loader: Loader, icon: Option<String>) -> Result<Instance, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give the instance a name.".into());
    }
    let instance = Instance {
        id: new_id(paths, name),
        name: name.chars().take(64).collect(),
        game_version: game_version.into(),
        loader,
        launch_version: None,
        version_type: version_type.into(),
        installed: false,
        created: now(),
        last_played: None,
        play_seconds: 0,
        icon,
        overrides: LaunchOverrides::default(),
    };
    save(paths, &instance)?;
    Ok(instance)
}

pub fn delete(paths: &Paths, id: &str) -> Result<(), String> {
    let dir = paths.instance(id);
    if !dir.join("instance.json").exists() {
        return Err("That instance doesn't exist any more.".into());
    }
    std::fs::remove_dir_all(&dir).map_err(|e| format!("Couldn't delete the instance (is the game still running?): {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_slugs() {
        let dir = std::env::temp_dir().join(format!("kryoto-ids-{}", now()));
        let paths = Paths::new(dir.clone());
        assert_eq!(new_id(&paths, "My Cool World!"), "my-cool-world");
        std::fs::create_dir_all(paths.instance("my-cool-world")).unwrap();
        assert_eq!(new_id(&paths, "My Cool World!"), "my-cool-world-2");
        assert_eq!(new_id(&paths, "???"), "instance");
        let _ = std::fs::remove_dir_all(dir);
    }
}
