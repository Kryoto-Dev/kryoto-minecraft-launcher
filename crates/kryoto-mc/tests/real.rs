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
//! with the loader version after it. `KRYOTO_TEST_JVM_ARGS` gives every
//! instance those Java arguments, as a player would type them.

use std::time::{Duration, Instant};

use kryoto_mc::core::Core;
use kryoto_mc::instances::{self, Loader, LoaderKind};
use kryoto_mc::launch::Extras;
use kryoto_mc::paths::Paths;
use kryoto_mc::{auth, curseforge, meta, modrinth};

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
        if let Ok(args) = std::env::var("KRYOTO_TEST_JVM_ARGS") {
            let mut i = instances::get(&paths, &id).unwrap();
            i.overrides.jvm_args = Some(args).filter(|a| !a.is_empty());
            instances::save(&paths, &i).unwrap();
        }
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

/// CurseForge, for real: a search, a mod with the mod it requires, and a
/// modpack imported as an instance with its files and configs. Needs a key in
/// `KRYOTO_CURSEFORGE_KEY`; `KRYOTO_TEST_CFPACK` is a pack `.zip` on disk,
/// without it Fabulously Optimized's newest CurseForge pack is downloaded.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "talks to CurseForge"]
async fn curseforge_for_real() {
    assert!(curseforge::available(), "KRYOTO_CURSEFORGE_KEY is set");
    let root = std::env::var("KRYOTO_TEST_ROOT").unwrap_or_else(|_| std::env::temp_dir().join("kryoto-mc-test").display().to_string());
    let paths = Paths::new(root.into());
    let core = Core::headless(paths.clone());
    use kryoto_mc::content::{self, Kind};

    let instance = instances::create(&paths, "test curseforge", "1.21.1", "release", Loader { kind: LoaderKind::Fabric, version: Some("0.16.10".into()) }, None).expect("instance");
    let found = curseforge::search(&instance, Kind::Mod, "mod menu", "relevance", 0).await.expect("search");
    eprintln!("search: {} results, first {:?}", found.total_hits, found.hits.first().map(|h| &h.title));
    let mod_menu = found.hits.iter().find(|h| h.title == "Mod Menu").expect("Mod Menu is found");
    let task = core.tasks.start("cf mod", Some("test-cf"), None).unwrap();
    let titles = curseforge::install(&paths, &instance, Kind::Mod, &mod_menu.project_id, &task).await.expect("Mod Menu installs");
    task.finish(&Ok(()));
    eprintln!("added: {}", titles.join(", "));
    assert!(titles.iter().any(|t| t.contains("Fabric API")), "Fabric API came with it");
    let mods = content::list(&paths, &instance.id, Kind::Mod, &instance.game_version, &instance.version_type);
    assert_eq!(mods.iter().filter(|m| m.source.as_ref().is_some_and(|s| s.site.as_deref() == Some("curseforge"))).count(), titles.len());
    let packs = curseforge::search(&instance, Kind::ResourcePack, "", "downloads", 0).await.expect("pack search");
    assert!(!packs.hits.is_empty(), "resource packs are found");
    instances::delete(&paths, &instance.id).expect("clean up");

    let file = match std::env::var("KRYOTO_TEST_CFPACK") {
        Ok(f) => std::path::PathBuf::from(f),
        Err(_) => {
            let key = std::env::var("KRYOTO_CURSEFORGE_KEY").unwrap();
            let files: serde_json::Value = kryoto_mc::net::client()
                .get("https://api.curseforge.com/v1/mods/396246/files?pageSize=1")
                .header("x-api-key", key)
                .send()
                .await
                .expect("pack files")
                .json()
                .await
                .expect("pack files json");
            let url = files["data"][0]["downloadUrl"].as_str().expect("pack download").to_string();
            let bytes = kryoto_mc::net::client().get(&url).send().await.expect("pack").bytes().await.expect("pack bytes");
            let file = paths.cache().join("test-pack.zip");
            std::fs::create_dir_all(paths.cache()).unwrap();
            std::fs::write(&file, &bytes).unwrap();
            file
        }
    };
    assert!(curseforge::is_pack(&file));
    let instance = curseforge::create(&paths, &file).await.expect("instance from the pack");
    eprintln!("pack: {} - {}", instance.name, instance.label());
    curseforge::add_files(&core, &instance.id).await.expect("the pack's files");
    let mods = content::list(&paths, &instance.id, Kind::Mod, &instance.game_version, &instance.version_type);
    eprintln!("{} mods, {} named", mods.len(), mods.iter().filter(|m| m.source.is_some()).count());
    assert!(mods.len() > 5, "the pack's mods are in the instance");
    assert!(mods.iter().all(|m| m.source.is_some()), "every mod is named from CurseForge");
    assert!(paths.game_dir(&instance.id).join("config").exists(), "the pack's configs were copied");
    instances::delete(&paths, &instance.id).expect("clean up");
}

/// The loader lists players pick from, as the makers serve them today.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "asks the loaders' servers"]
async fn loader_lists() {
    let root = std::env::var("KRYOTO_TEST_ROOT").unwrap_or_else(|_| std::env::temp_dir().join("kryoto-mc-test").display().to_string());
    let paths = Paths::new(root.into());
    let list = |kind: LoaderKind, game: &'static str| {
        let paths = paths.clone();
        async move { kryoto_mc::loaders::versions(&paths, &kind, game).await.expect("loader list") }
    };
    let quilt = list(LoaderKind::Quilt, "26.3").await;
    let recommended = quilt.iter().find(|v| v.recommended).expect("a recommended Quilt");
    eprintln!("Quilt 26.3: first {}, recommended {}", quilt[0].label, recommended.label);
    assert!(quilt.iter().filter(|v| v.stable).all(|v| !recommended.label.is_empty() && kryoto_mc::loaders::newer_or_same(&recommended.label, &v.label)), "the recommended Quilt is the newest stable one");
    assert!(list(LoaderKind::Forge, "1.1").await.is_empty(), "no Forge without an installer");
    let forge_161 = list(LoaderKind::Forge, "1.6.1").await;
    eprintln!("Forge 1.6.1: {} builds with installers", forge_161.len());
    assert!(!forge_161.iter().any(|v| v.version == "1.6.1-8.9.0.749"), "a 1.6.1 build without an installer is left out");
    assert!(forge_161.iter().any(|v| v.version == "1.6.1-8.9.0.775"));
    assert!(!list(LoaderKind::Forge, "1.20.1").await.is_empty());
    assert!(list(LoaderKind::NeoForge, "26.3").await.iter().any(|v| v.recommended));
}
