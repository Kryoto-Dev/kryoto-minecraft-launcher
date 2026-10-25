//! The real thing: download versions from Mojang (and loaders from their
//! makers), start them with an offline profile, wait until the game is up,
//! and stop it. Opens Minecraft windows and downloads gigabytes, so it only
//! runs when asked:
//!
//! ```text
//! KRYOTO_TEST_ROOT=D:/kryo-test \
//! KRYOTO_TEST_GAMES="26.3 vanilla; 1.21.1 fabric; 1.12.2 forge; b1.7.3 vanilla" \
//!   cargo test --manifest-path crates/kryoto-mc/Cargo.toml --test real -- --ignored --nocapture
//! ```
//!
//! `KRYOTO_TEST_ROOT` is the data folder (it keeps what it downloaded, so a
//! second run is quick). Each entry of `KRYOTO_TEST_GAMES` is a Minecraft
//! version and a loader (vanilla, fabric, quilt, forge, neoforge), optionally
//! with the loader version after it.

use std::time::{Duration, Instant};

use kryoto_mc::core::Core;
use kryoto_mc::instances::{self, Loader, LoaderKind};
use kryoto_mc::launch::Extras;
use kryoto_mc::paths::Paths;
use kryoto_mc::{auth, meta, modrinth};

fn loader(name: &str) -> LoaderKind {
    match name {
        "fabric" => LoaderKind::Fabric,
        "quilt" => LoaderKind::Quilt,
        "forge" => LoaderKind::Forge,
        "neoforge" => LoaderKind::NeoForge,
        _ => LoaderKind::Vanilla,
    }
}

/// Lines that only appear once the game's window and renderer are up.
const UP: &[&str] = &["Sound engine started", "OpenAL initialized", "Created: ", "Reloading ResourceManager", "LWJGL Version"];

#[tokio::test(flavor = "multi_thread")]
#[ignore = "downloads Minecraft and opens its window"]
async fn install_and_play() {
    let root = std::env::var("KRYOTO_TEST_ROOT").unwrap_or_else(|_| std::env::temp_dir().join("kryoto-mc-test").display().to_string());
    let games = std::env::var("KRYOTO_TEST_GAMES").unwrap_or_else(|_| "latest vanilla".into());
    let paths = Paths::new(root.into());
    let core = Core::headless(paths.clone());
    auth::add(&paths, "KryotoTest").expect("profile");
    let manifest = meta::manifest(&paths, true).await.expect("version list");

    let mut failures = Vec::new();
    for entry in games.split(';').map(str::trim).filter(|e| !e.is_empty()) {
        let parts: Vec<&str> = entry.split_whitespace().collect();
        let game = if parts[0] == "latest" { manifest.latest.release.clone() } else { parts[0].to_string() };
        let kind = loader(parts.get(1).copied().unwrap_or("vanilla"));
        let version_type = manifest.versions.iter().find(|v| v.id == game).map(|v| v.kind.clone()).unwrap_or_default();
        let name = format!("test {entry}");
        let existing = instances::list(&paths).into_iter().find(|i| i.name == name);
        let id = match existing {
            Some(i) => i.id,
            None => {
                instances::create(&paths, &name, &game, &version_type, Loader { kind: kind.clone(), version: parts.get(2).map(|s| s.to_string()) }, None)
                    .expect("instance")
                    .id
            }
        };
        println!("\n== {entry}: installing");
        let started = Instant::now();
        if let Err(e) = core.install(&id, false).await {
            failures.push(format!("{entry}: install failed: {e}"));
            continue;
        }
        println!("   installed in {:.0?}", started.elapsed());

        // Mods from Modrinth, with their dependencies, for loader instances.
        if matches!(kind, LoaderKind::Fabric | LoaderKind::NeoForge) {
            let instance = instances::get(&paths, &id).unwrap();
            let task = core.tasks.start("mod", Some("test-mods"), None).unwrap();
            match modrinth::install(&paths, &instance, kryoto_mc::content::Kind::Mod, "sodium", &task).await {
                Ok(titles) => println!("   added from Modrinth: {}", titles.join(", ")),
                Err(e) => failures.push(format!("{entry}: Modrinth install failed: {e}")),
            }
            task.finish(&Ok(()));
        }

        println!("   starting");
        if let Err(e) = core.launch(&id, Extras::default()).await {
            failures.push(format!("{entry}: start failed: {e}"));
            continue;
        }
        let deadline = Instant::now() + Duration::from_secs(150);
        let mut up = false;
        while Instant::now() < deadline {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let log = core.games.log(&id);
            if log.iter().any(|l| UP.iter().any(|m| l.line.contains(m))) {
                up = true;
                break;
            }
            if !core.games.running(&id) {
                break;
            }
            // The oldest versions print nothing at all; alive after 25 s is up.
            if version_type.starts_with("old_") && started.elapsed() > Duration::from_secs(25) && core.games.running(&id) {
                up = true;
                break;
            }
        }
        let running = core.games.running(&id);
        let tail: Vec<String> = core.games.log(&id).iter().rev().take(25).map(|l| l.line.clone()).collect::<Vec<_>>().into_iter().rev().collect();
        if up && running {
            println!("   up and running; stopping it");
            // Give it a moment on screen, then stop.
            tokio::time::sleep(Duration::from_secs(5)).await;
            core.games.stop(&id);
            let stop_deadline = Instant::now() + Duration::from_secs(20);
            while core.games.running(&id) && Instant::now() < stop_deadline {
                tokio::time::sleep(Duration::from_millis(300)).await;
            }
        } else {
            let state = core.games.states().into_iter().find(|s| s.instance == id);
            failures.push(format!("{entry}: never came up (running: {running}, exit: {:?})\n{}", state.and_then(|s| s.exit), tail.join("\n")));
            core.games.stop(&id);
        }
    }
    assert!(failures.is_empty(), "\n\n{}", failures.join("\n\n"));
}

/// A real Modrinth modpack: imported as an instance, its mods downloaded and
/// named, its configs copied in. `KRYOTO_TEST_MRPACK` is a `.mrpack` on disk;
/// without it, Fabulously Optimized's newest pack is downloaded.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "downloads a modpack"]
async fn import_a_modpack() {
    let root = std::env::var("KRYOTO_TEST_ROOT").unwrap_or_else(|_| std::env::temp_dir().join("kryoto-mc-test").display().to_string());
    let paths = Paths::new(root.into());
    let core = Core::headless(paths.clone());
    let file = match std::env::var("KRYOTO_TEST_MRPACK") {
        Ok(f) => std::path::PathBuf::from(f),
        Err(_) => {
            let versions: serde_json::Value = kryoto_mc::net::get_json("https://api.modrinth.com/v2/project/fabulously-optimized/version").await.expect("pack versions");
            let url = versions[0]["files"][0]["url"].as_str().expect("pack file").to_string();
            let bytes = kryoto_mc::net::client().get(&url).send().await.expect("pack").bytes().await.expect("pack bytes");
            let file = paths.cache().join("test-pack.mrpack");
            std::fs::create_dir_all(paths.cache()).unwrap();
            std::fs::write(&file, &bytes).unwrap();
            file
        }
    };
    let instance = kryoto_mc::mrpack::create(&paths, &file).await.expect("instance from the pack");
    eprintln!("{} - {}", instance.name, instance.label());
    kryoto_mc::mrpack::add_files(&core, &instance.id).await.expect("the pack's files");
    let mods = kryoto_mc::content::list(&paths, &instance.id, kryoto_mc::content::Kind::Mod, &instance.game_version, &instance.version_type);
    assert!(!mods.is_empty(), "the pack's mods are in the instance");
    let named = mods.iter().filter(|m| m.source.is_some()).count();
    eprintln!("{} mods, {named} known to Modrinth", mods.len());
    assert!(named > 0, "Modrinth names the pack's mods");
    assert!(paths.game_dir(&instance.id).join("config").exists(), "the pack's configs were copied");
    instances::delete(&paths, &instance.id).expect("clean up");
}
