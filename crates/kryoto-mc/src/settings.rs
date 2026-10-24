//! The launcher's settings: launch defaults every instance starts from, and
//! the look (the same palettes and corners as kryo.to and Kryoto Desktop).

use serde::{Deserialize, Serialize};

use crate::paths::Paths;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// A Java for every instance; `None` uses Mojang's runtime per version.
    pub java_path: Option<String>,
    pub memory_mb: u32,
    pub jvm_args: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    /// What the launcher does once the game is running: "keep", "minimize", "close".
    pub after_launch: String,
    /// Files downloaded at once.
    pub parallel_downloads: u32,
    pub palette: String,
    pub radius: String,
    pub font: String,
    pub first_run_done: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            java_path: None,
            memory_mb: default_memory(),
            jvm_args: String::new(),
            width: 854,
            height: 480,
            fullscreen: false,
            after_launch: "keep".into(),
            parallel_downloads: 16,
            palette: "oled".into(),
            radius: "pill".into(),
            font: "teletext".into(),
            first_run_done: false,
        }
    }
}

/// Total memory in MB.
pub fn total_memory_mb() -> u64 {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    system.total_memory() / (1024 * 1024)
}

/// A quarter of the computer's memory, between 2 and 6 GB: enough for vanilla
/// and a modest mod list without starving the system.
fn default_memory() -> u32 {
    let quarter = total_memory_mb() / 4;
    (quarter.clamp(2048, 6144) / 512 * 512) as u32
}

pub fn load(paths: &Paths) -> Settings {
    std::fs::read(paths.settings()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save(paths: &Paths, settings: &Settings) -> Result<(), String> {
    std::fs::create_dir_all(&paths.root).map_err(|e| e.to_string())?;
    let tmp = paths.settings().with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, paths.settings()).map_err(|e| format!("Couldn't save settings: {e}"))
}
