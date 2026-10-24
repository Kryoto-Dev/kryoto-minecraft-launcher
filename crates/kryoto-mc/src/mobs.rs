//! Mob textures for the launcher's decoration (the chick that walks along the
//! bottom of the window), read from the player's own installed game.
//!
//! The launcher ships no Mojang files: the texture comes out of a client jar
//! this player downloaded from Mojang, at the moment it is shown. Until a
//! version that has it is installed, there is nothing to show.

use std::io::Read;

use crate::paths::Paths;

/// The textures the UI may ask for, and where they are in the jar.
const ALLOWED: &[&str] = &["chicken_temperate_baby", "chicken_warm_baby", "chicken_cold_baby"];

pub fn texture(paths: &Paths, name: &str) -> Result<Vec<u8>, String> {
    if !ALLOWED.contains(&name) {
        return Err(format!("No texture called {name}."));
    }
    let inner = format!("assets/minecraft/textures/entity/chicken/{name}.png");
    // Newest jar first: the newest game has the newest look.
    let mut jars: Vec<(std::time::SystemTime, std::path::PathBuf)> = std::fs::read_dir(paths.versions())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().into_owned();
            let jar = paths.version_jar(&id);
            let modified = jar.metadata().ok()?.modified().ok()?;
            Some((modified, jar))
        })
        .collect();
    jars.sort_by_key(|j| std::cmp::Reverse(j.0));
    for (_, jar) in jars {
        let Ok(file) = std::fs::File::open(&jar) else { continue };
        let Ok(mut zip) = zip::ZipArchive::new(file) else { continue };
        let Ok(mut entry) = zip.by_name(&inner) else { continue };
        if entry.size() > 64 * 1024 {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        return Ok(bytes);
    }
    Err("No installed version has this texture yet.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_known_textures() {
        let paths = Paths::new(std::env::temp_dir().join("kryoto-mobs-none"));
        assert!(texture(&paths, "../../secrets").unwrap_err().contains("No texture"));
        assert!(texture(&paths, "chicken_temperate_baby").unwrap_err().contains("No installed version"));
    }
}
