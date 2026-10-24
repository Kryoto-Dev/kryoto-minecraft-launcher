//! Kryoto Minecraft Launcher: the app around the `kryoto-mc` engine. The
//! window, the commands the UI calls, events back to it, file dialogs.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, State};

use kryoto_mc::content::{self, Kind};
use kryoto_mc::core::Core;
use kryoto_mc::instances::{self, Instance, LaunchOverrides, Loader};
use kryoto_mc::paths::Paths;
use kryoto_mc::{auth, java, launch, loaders, logging, meta, modrinth, settings, tasks};

struct App {
    core: Core,
    /// The Microsoft sign-in in progress: its code, and a way to stop waiting.
    sign_in: Mutex<Option<(auth::DeviceCode, Arc<AtomicBool>)>>,
}

type Res<T> = Result<T, String>;

/* ── App ────────────────────────────────────────────────── */

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: String,
    data_dir: String,
    microsoft_available: bool,
    platform: String,
    total_memory_mb: u64,
}

#[tauri::command]
fn app_info(app: State<App>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        data_dir: app.core.paths.root.display().to_string(),
        microsoft_available: auth::client_id().is_some(),
        platform: format!("{}-{}", meta::os_name(), meta::os_arch()),
        total_memory_mb: settings::total_memory_mb(),
    }
}

#[tauri::command]
fn settings_get(app: State<App>) -> settings::Settings {
    settings::load(&app.core.paths)
}

#[tauri::command]
fn settings_set(app: State<App>, settings: settings::Settings) -> Res<settings::Settings> {
    settings::save(&app.core.paths, &settings)?;
    Ok(settings)
}

#[tauri::command]
fn log_write(app: State<App>, level: String, scope: String, message: String) {
    logging::write(&app.core.paths, &level, &format!("ui/{scope}"), &message);
}

#[tauri::command(async)]
fn open_external(url: String) -> Res<()> {
    let parsed: url::Url = url.parse().map_err(|_| "Not a valid address.".to_string())?;
    if parsed.scheme() != "https" {
        return Err("Only https links open from the launcher.".into());
    }
    open_with_system(parsed.as_str())
}

fn open_with_system(target: &str) -> Res<()> {
    #[cfg(windows)]
    let result = {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", target]).creation_flags(0x0800_0000).spawn()
    };
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(target).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(target).spawn();
    result.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command(async)]
fn open_data_folder(app: State<App>, sub: Option<String>) -> Res<()> {
    let dir = match sub.as_deref() {
        Some("logs") => app.core.paths.logs(),
        _ => app.core.paths.root.clone(),
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    open_folder(&dir)
}

fn open_folder(dir: &std::path::Path) -> Res<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("explorer").arg(dir).spawn().map(|_| ()).map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        open_with_system(&dir.display().to_string())
    }
}

/* ── Versions ───────────────────────────────────────────── */

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VersionRow {
    id: String,
    kind: String,
    release_time: String,
    /// The vanilla files are already downloaded.
    installed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VersionList {
    latest_release: String,
    latest_snapshot: String,
    versions: Vec<VersionRow>,
}

#[tauri::command]
async fn versions_list(app: State<'_, App>, refresh: bool) -> Res<VersionList> {
    let manifest = meta::manifest(&app.core.paths, refresh).await?;
    let paths = &app.core.paths;
    Ok(VersionList {
        latest_release: manifest.latest.release,
        latest_snapshot: manifest.latest.snapshot,
        versions: manifest
            .versions
            .into_iter()
            .map(|v| VersionRow { installed: paths.version_jar(&v.id).exists(), id: v.id, kind: v.kind, release_time: v.release_time })
            .collect(),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VersionDetails {
    id: String,
    kind: String,
    release_time: Option<String>,
    java_major: u32,
    main_class: Option<String>,
    library_count: usize,
    client_size: Option<u64>,
    assets: Option<String>,
    assets_size: Option<u64>,
}

#[tauri::command]
async fn version_details(app: State<'_, App>, id: String) -> Res<VersionDetails> {
    let json = meta::version_json(&app.core.paths, &id).await?;
    Ok(VersionDetails {
        java_major: meta::java_major(&json),
        kind: json.kind.clone().unwrap_or_default(),
        release_time: json.release_time.clone(),
        main_class: json.main_class.clone(),
        library_count: json.libraries.len(),
        client_size: json.downloads.as_ref().and_then(|d| d.client.as_ref()).and_then(|c| c.size),
        assets: json.asset_index.as_ref().map(|a| a.id.clone()),
        assets_size: json.asset_index.as_ref().and_then(|a| a.total_size),
        id: json.id,
    })
}

#[tauri::command]
async fn loader_versions(app: State<'_, App>, kind: instances::LoaderKind, game: String) -> Res<Vec<loaders::LoaderVersion>> {
    loaders::versions(&app.core.paths, &kind, &game).await
}

/* ── Instances ──────────────────────────────────────────── */

#[tauri::command]
fn instances_list(app: State<App>) -> Vec<Instance> {
    instances::list(&app.core.paths)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewInstance {
    name: String,
    game_version: String,
    version_type: String,
    loader: Loader,
    icon: Option<String>,
}

#[tauri::command]
async fn instance_create(app: State<'_, App>, instance: NewInstance) -> Res<Instance> {
    let created = instances::create(&app.core.paths, &instance.name, &instance.game_version, &instance.version_type, instance.loader, instance.icon)?;
    let core = app.core.clone();
    let id = created.id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = core.install(&id, false).await {
            logging::write(&core.paths, "warn", "install", &format!("{id}: {e}"));
        }
    });
    Ok(created)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstancePatch {
    name: Option<String>,
    icon: Option<String>,
    overrides: Option<LaunchOverrides>,
    /// A new loader (or loader version): the instance is installed again.
    loader: Option<Loader>,
}

#[tauri::command]
fn instance_update(app: State<App>, id: String, patch: InstancePatch) -> Res<Instance> {
    let paths = &app.core.paths;
    let mut instance = instances::get(paths, &id)?;
    if let Some(name) = patch.name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) {
        instance.name = name.chars().take(64).collect();
    }
    if let Some(icon) = patch.icon {
        instance.icon = Some(icon);
    }
    if let Some(overrides) = patch.overrides {
        instance.overrides = overrides;
    }
    if let Some(loader) = patch.loader {
        if loader.kind != instance.loader.kind || loader.version != instance.loader.version {
            if app.core.games.running(&id) {
                return Err("Close the game before changing its mod loader.".into());
            }
            instance.loader = loader;
            instance.launch_version = None;
            instance.installed = false;
        }
    }
    instances::save(paths, &instance)?;
    Ok(instance)
}

#[tauri::command]
fn instance_delete(app: State<App>, id: String) -> Res<()> {
    if app.core.games.running(&id) {
        return Err("Close the game before deleting its instance.".into());
    }
    instances::delete(&app.core.paths, &id)
}

#[tauri::command]
async fn instance_install(app: State<'_, App>, id: String, repair: bool) -> Res<Instance> {
    app.core.install(&id, repair).await
}

#[tauri::command(async)]
fn instance_open_folder(app: State<App>, id: String, sub: Option<String>) -> Res<()> {
    let base = app.core.paths.game_dir(&id);
    let dir = match sub.as_deref() {
        Some(s) if !s.contains(['/', '\\', '.']) => base.join(s),
        _ => base,
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    open_folder(&dir)
}

#[tauri::command(async)]
fn open_crash_report(app: State<App>, id: String, path: String) -> Res<()> {
    let path = PathBuf::from(path);
    if !path.starts_with(app.core.paths.game_dir(&id)) || !path.is_file() {
        return Err("That crash report isn't there any more.".into());
    }
    open_with_system(&path.display().to_string())
}

/* ── Playing ────────────────────────────────────────────── */

#[tauri::command]
async fn launch(app: State<'_, App>, id: String, server: Option<String>, world: Option<String>) -> Res<u32> {
    app.core.launch(&id, launch::Extras { server, world }).await
}

#[tauri::command]
fn game_stop(app: State<App>, id: String) -> bool {
    app.core.games.stop(&id)
}

#[tauri::command]
fn game_states(app: State<App>) -> Vec<launch::GameState> {
    app.core.games.states()
}

#[tauri::command]
fn game_log(app: State<App>, id: String) -> Vec<launch::LogLine> {
    app.core.games.log(&id)
}

/* ── Tasks ──────────────────────────────────────────────── */

#[tauri::command]
fn tasks_list(app: State<App>) -> Vec<tasks::Task> {
    app.core.tasks.list()
}

#[tauri::command]
fn task_cancel(app: State<App>, id: String) {
    app.core.tasks.cancel(&id);
}

#[tauri::command]
fn tasks_clear(app: State<App>) {
    app.core.tasks.clear();
}

/* ── Accounts ───────────────────────────────────────────── */

#[tauri::command]
fn accounts_list(app: State<App>) -> auth::Accounts {
    auth::load(&app.core.paths)
}

#[tauri::command]
fn account_offline_add(app: State<App>, name: String) -> Res<auth::Account> {
    auth::add_offline(&app.core.paths, &name)
}

#[tauri::command]
fn account_set_active(app: State<App>, id: String) -> Res<()> {
    auth::set_active(&app.core.paths, &id)
}

#[tauri::command]
fn account_remove(app: State<App>, id: String) -> Res<()> {
    auth::remove(&app.core.paths, &id)
}

#[tauri::command]
async fn account_microsoft_start(app: State<'_, App>) -> Res<auth::DeviceCode> {
    let code = auth::start_device_code().await?;
    let stop = Arc::new(AtomicBool::new(false));
    if let Some((_, old)) = app.sign_in.lock().unwrap().replace((code.clone(), stop)) {
        old.store(true, Ordering::Relaxed);
    }
    Ok(code)
}

#[tauri::command]
async fn account_microsoft_finish(app: State<'_, App>) -> Res<auth::Account> {
    let (code, stop) = app.sign_in.lock().unwrap().clone().ok_or("Start the sign-in first.")?;
    let result = auth::finish_device_code(&app.core.paths, &code, || stop.load(Ordering::Relaxed)).await;
    let mut current = app.sign_in.lock().unwrap();
    if current.as_ref().is_some_and(|(c, _)| c.user_code == code.user_code) {
        *current = None;
    }
    result
}

#[tauri::command]
fn account_microsoft_cancel(app: State<App>) {
    if let Some((_, stop)) = app.sign_in.lock().unwrap().take() {
        stop.store(true, Ordering::Relaxed);
    }
}

/* ── Content ────────────────────────────────────────────── */

#[tauri::command(async)]
fn content_list(app: State<App>, id: String, kind: String) -> Res<Vec<content::Item>> {
    let instance = instances::get(&app.core.paths, &id)?;
    Ok(content::list(&app.core.paths, &id, Kind::parse(&kind)?, &instance.game_version, &instance.version_type))
}

#[tauri::command]
fn content_set_enabled(app: State<App>, id: String, kind: String, file: String, enabled: bool) -> Res<String> {
    let instance = instances::get(&app.core.paths, &id)?;
    content::set_enabled(&app.core.paths, &id, Kind::parse(&kind)?, &instance.game_version, &instance.version_type, &file, enabled)
}

#[tauri::command]
fn content_delete(app: State<App>, id: String, kind: String, file: String) -> Res<()> {
    let instance = instances::get(&app.core.paths, &id)?;
    content::delete(&app.core.paths, &id, Kind::parse(&kind)?, &instance.game_version, &instance.version_type, &file)
}

#[derive(Serialize)]
struct Imported {
    file: String,
    kind: Kind,
}

#[tauri::command(async)]
fn content_import(app: State<App>, id: String, files: Vec<String>, kind: Option<String>) -> Res<Vec<Imported>> {
    let instance = instances::get(&app.core.paths, &id)?;
    let only = kind.as_deref().map(Kind::parse).transpose()?;
    let files: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
    let done = content::import(&app.core.paths, &id, &instance.game_version, &instance.version_type, &files, only)?;
    Ok(done.into_iter().map(|(file, kind)| Imported { file, kind }).collect())
}

/// Pick files with the system's dialog and add them.
#[tauri::command(async)]
fn content_pick(app: State<App>, id: String, kind: String) -> Res<Vec<Imported>> {
    let k = Kind::parse(&kind)?;
    let (label, ext): (&str, &[&str]) = match k {
        Kind::Mod => ("Mods", &["jar"]),
        _ => ("Packs", &["zip"]),
    };
    let Some(files) = rfd::FileDialog::new().add_filter(label, ext).set_title("Add to this instance").pick_files() else {
        return Ok(Vec::new());
    };
    content_import(app, id, files.into_iter().map(|p| p.display().to_string()).collect(), Some(kind))
}

#[tauri::command]
async fn modrinth_search(app: State<'_, App>, id: String, kind: String, query: String, sort: String, offset: u32) -> Res<modrinth::SearchResult> {
    let instance = instances::get(&app.core.paths, &id)?;
    modrinth::search(&instance, Kind::parse(&kind)?, &query, &sort, offset).await
}

#[tauri::command]
async fn modrinth_install(app: State<'_, App>, id: String, kind: String, project: String, title: String) -> Res<Vec<String>> {
    let core = &app.core;
    let instance = instances::get(&core.paths, &id)?;
    let task = core.tasks.start(
        &format!("Adding {title}"),
        Some(&id),
        Some(serde_json::json!({ "command": "modrinth_install", "args": { "id": id, "kind": kind, "project": project, "title": title } })),
    )?;
    let result = modrinth::install(&core.paths, &instance, Kind::parse(&kind)?, &project, &task).await;
    task.finish(&result.as_ref().map(|_| ()).map_err(Clone::clone));
    result
}

/* ── Java ───────────────────────────────────────────────── */

#[tauri::command]
async fn java_probe(path: String) -> Res<java::JavaInfo> {
    java::probe(std::path::Path::new(&path)).await
}

#[tauri::command(async)]
fn java_pick() -> Option<String> {
    let mut dialog = rfd::FileDialog::new().set_title("Choose a Java program");
    if cfg!(windows) {
        dialog = dialog.add_filter("Java", &["exe"]);
    }
    dialog.pick_file().map(|p| p.display().to_string())
}

/* ── Start ──────────────────────────────────────────────── */

fn data_root(app: &tauri::AppHandle) -> PathBuf {
    let base = app.path().app_data_dir().expect("the system has an app data folder");
    // Debug builds keep their own data beside an installed copy's.
    if cfg!(debug_assertions) {
        base.with_file_name("to.kryo.minecraft.dev")
    } else {
        base
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let paths = Paths::new(data_root(&handle));
            std::fs::create_dir_all(&paths.root)?;
            if !logging::claim_single_instance(&paths) {
                rfd::MessageDialog::new()
                    .set_title("Kryoto Minecraft Launcher")
                    .set_description("The launcher is already open.")
                    .set_level(rfd::MessageLevel::Info)
                    .show();
                std::process::exit(0);
            }
            logging::write(&paths, "info", "start", &format!("version {} on {}", env!("CARGO_PKG_VERSION"), meta::os_name()));

            let tasks_handle = handle.clone();
            let log_handle = handle.clone();
            let state_handle = handle.clone();
            let instances_handle = handle.clone();
            let core = Core {
                paths,
                tasks: tasks::Tasks::new(move |list| {
                    let _ = tasks_handle.emit("tasks", list);
                }),
                games: launch::Games::default(),
                emit_log: Arc::new(move |instance, lines| {
                    let _ = log_handle.emit("game-log", serde_json::json!({ "instance": instance, "lines": lines }));
                }),
                emit_state: Arc::new(move |state| {
                    let _ = state_handle.emit("game-state", state);
                }),
                emit_instances: Arc::new(move || {
                    let _ = instances_handle.emit("instances-changed", ());
                }),
            };
            app.manage(App { core, sign_in: Mutex::new(None) });
            // The page shows the window once it has drawn (no white flash); if
            // it never gets that far, the window still appears.
            let window_handle = handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(5));
                if let Some(window) = window_handle.get_webview_window("main") {
                    if !window.is_visible().unwrap_or(true) {
                        let _ = window.show();
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            settings_get,
            settings_set,
            log_write,
            open_external,
            open_data_folder,
            versions_list,
            version_details,
            loader_versions,
            instances_list,
            instance_create,
            instance_update,
            instance_delete,
            instance_install,
            instance_open_folder,
            open_crash_report,
            launch,
            game_stop,
            game_states,
            game_log,
            tasks_list,
            task_cancel,
            tasks_clear,
            accounts_list,
            account_offline_add,
            account_set_active,
            account_remove,
            account_microsoft_start,
            account_microsoft_finish,
            account_microsoft_cancel,
            content_list,
            content_set_enabled,
            content_delete,
            content_import,
            content_pick,
            modrinth_search,
            modrinth_install,
            java_probe,
            java_pick,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Kryoto Minecraft Launcher");
}
