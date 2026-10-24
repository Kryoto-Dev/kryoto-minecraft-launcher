//! Where everything lives on disk.
//!
//! The layout under the data folder mirrors the official launcher's
//! `.minecraft` (`versions/`, `libraries/`, `assets/`), so files are shared
//! between instances and the Forge and NeoForge installers, which expect that
//! layout, can install straight into it. Each instance gets its own game
//! folder for saves, mods, packs and options.
//!
//! ```text
//! <data>/
//!   versions/<id>/<id>.json, <id>.jar      vanilla and loader versions
//!   libraries/                             maven layout
//!   assets/indexes, objects, virtual, log_configs
//!   runtimes/<component>/                  Mojang's Java runtimes
//!   natives/<version>/                     extracted native libraries
//!   instances/<id>/instance.json           an instance's settings
//!   instances/<id>/minecraft/              its game folder
//!   cache/                                 version lists, loader lists
//!   logs/launcher.log
//! ```

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }
    pub fn version_dir(&self, id: &str) -> PathBuf {
        self.versions().join(id)
    }
    pub fn version_json(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.json"))
    }
    pub fn version_jar(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.jar"))
    }
    pub fn libraries(&self) -> PathBuf {
        self.root.join("libraries")
    }
    pub fn library(&self, maven_path: &str) -> PathBuf {
        join_relative(&self.libraries(), maven_path)
    }
    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }
    pub fn asset_index(&self, id: &str) -> PathBuf {
        self.assets().join("indexes").join(format!("{id}.json"))
    }
    pub fn asset_object(&self, hash: &str) -> PathBuf {
        self.assets().join("objects").join(&hash[..2]).join(hash)
    }
    pub fn runtimes(&self) -> PathBuf {
        self.root.join("runtimes")
    }
    pub fn natives(&self, version: &str) -> PathBuf {
        self.root.join("natives").join(version)
    }
    pub fn instances(&self) -> PathBuf {
        self.root.join("instances")
    }
    pub fn instance(&self, id: &str) -> PathBuf {
        self.instances().join(id)
    }
    pub fn game_dir(&self, id: &str) -> PathBuf {
        self.instance(id).join("minecraft")
    }
    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
    pub fn settings(&self) -> PathBuf {
        self.root.join("settings.json")
    }
    pub fn accounts(&self) -> PathBuf {
        self.root.join("accounts.json")
    }
    pub fn temp(&self) -> PathBuf {
        self.root.join("cache").join("tmp")
    }
}

/// `base` joined with a relative, forward-slashed path from a manifest, refusing
/// anything that would climb out of `base` (`..`, absolute paths, drive letters).
pub fn join_relative(base: &Path, relative: &str) -> PathBuf {
    let mut out = base.to_path_buf();
    for part in relative.split(['/', '\\']) {
        if part.is_empty() || part == "." || part == ".." || part.contains(':') {
            continue;
        }
        out.push(part);
    }
    out
}

/// A file name safe on every platform, from anything a user or a server typed.
pub fn safe_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_control() || r#"<>:"/\|?*"#.contains(c) { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches(['.', ' ']).to_string();
    if trimmed.is_empty() {
        "file".into()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_stay_inside() {
        let base = Path::new("/data");
        assert_eq!(join_relative(base, "a/b/c.jar"), Path::new("/data/a/b/c.jar"));
        assert_eq!(join_relative(base, "../../etc/passwd"), Path::new("/data/etc/passwd"));
        assert_eq!(join_relative(base, "C:/Windows/x"), Path::new("/data/Windows/x"));
    }

    #[test]
    fn file_names_are_cleaned() {
        assert_eq!(safe_file_name("my:mod?.jar"), "my_mod_.jar");
        assert_eq!(safe_file_name("..."), "file");
    }
}
