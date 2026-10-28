//! Mods, resource packs, shader packs and modpacks from CurseForge
//! (api.curseforge.com). Its API needs a key: release builds get one from the
//! `CURSEFORGE_API_KEY` secret, and `KRYOTO_CURSEFORGE_KEY` sets one while
//! developing. A build without a key offers Modrinth only.
//!
//! Like Modrinth, only files that fit the instance are offered, and adding a
//! mod also adds the mods it requires. Some authors let their files be
//! downloaded only on CurseForge's own site; when Modrinth has the very same
//! file (by SHA-1) it comes from there, and otherwise it is named, with its
//! page, so the player can fetch it there and drop it on the window.
//!
//! Modpacks are CurseForge's `.zip` export: `manifest.json` (the Minecraft
//! version, the loader and every file by project and file id) and
//! `overrides/`, imported the way `.mrpack` files are.
//!
//! API: <https://docs.curseforge.com/rest-api/>

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::content::{self, Kind, Source};
use crate::core::Core;
use crate::download::{self, FileJob, Verify};
use crate::instances::{self, Instance, LoaderKind};
use crate::modrinth::{Hit, SearchResult};
use crate::modrinth;
use crate::mrpack;
use crate::net;
use crate::paths::Paths;
use crate::settings;
use crate::tasks::TaskHandle;

const API: &str = "https://api.curseforge.com/v1";
const MINECRAFT: u32 = 432;
const SITE: &str = "curseforge";
/// The copy of the pack an imported instance keeps, for "Try again".
const KEPT: &str = "modpack.zip";
/// CurseForge answers no further than this many results into a search.
const SEARCH_WINDOW: u32 = 10_000;

fn key() -> Option<String> {
    std::env::var("KRYOTO_CURSEFORGE_KEY")
        .ok()
        .or_else(|| option_env!("CURSEFORGE_API_KEY").map(str::to_string))
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
}

/// Whether this build can reach CurseForge.
pub fn available() -> bool {
    key().is_some()
}

fn no_key() -> String {
    "This build of the launcher can't reach CurseForge. Use Modrinth, or download the file from CurseForge and drop it on the window.".into()
}

async fn send<T: DeserializeOwned>(request: reqwest::RequestBuilder) -> Result<T, String> {
    let key = key().ok_or_else(no_key)?;
    let response = request.header("x-api-key", key).header("accept", "application/json").send().await.map_err(|e| format!("CurseForge couldn't be reached. {e}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            403 => "CurseForge refused the launcher's request. Try again later, or use Modrinth.".into(),
            404 => "CurseForge doesn't have that any more.".into(),
            _ => format!("CurseForge answered {status}. Try again in a moment."),
        });
    }
    response.json::<T>().await.map_err(|e| format!("CurseForge's answer couldn't be read: {e}"))
}

async fn get<T: DeserializeOwned>(url: &url::Url) -> Result<T, String> {
    send(net::client().get(url.as_str())).await
}

async fn post<T: DeserializeOwned>(path: &str, body: serde_json::Value) -> Result<T, String> {
    send(net::client().post(format!("{API}{path}")).json(&body)).await
}

#[derive(Deserialize)]
struct Data<T> {
    data: T,
}

/* ── What CurseForge sends ──────────────────────────────── */

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Mod {
    id: u64,
    name: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    download_count: f64,
    #[serde(default)]
    logo: Option<Logo>,
    #[serde(default)]
    authors: Vec<Named>,
    #[serde(default)]
    categories: Vec<Named>,
    #[serde(default)]
    date_modified: String,
    #[serde(default)]
    links: Option<Links>,
    #[serde(default)]
    class_id: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Logo {
    #[serde(default)]
    thumbnail_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct Named {
    name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Links {
    #[serde(default)]
    website_url: Option<String>,
}

impl Mod {
    fn page(&self) -> String {
        self.links
            .as_ref()
            .and_then(|l| l.website_url.clone())
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| format!("https://www.curseforge.com/minecraft/mc-mods/{}", self.slug))
    }

    fn icon(&self) -> Option<String> {
        self.logo.as_ref().and_then(|l| l.thumbnail_url.clone()).filter(|u| !u.is_empty())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct File {
    id: u64,
    mod_id: u64,
    #[serde(default)]
    display_name: String,
    file_name: String,
    /// 1 release, 2 beta, 3 alpha.
    #[serde(default)]
    release_type: u8,
    #[serde(default)]
    file_date: String,
    #[serde(default)]
    file_length: u64,
    /// `None` when the author allows downloads on CurseForge's site only.
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    hashes: Vec<FileHash>,
    /// Minecraft versions and loader names ("1.20.1", "Forge").
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    dependencies: Vec<FileDependency>,
    #[serde(default = "yes")]
    is_available: bool,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize)]
struct FileHash {
    value: String,
    /// 1 SHA-1, 2 MD5.
    algo: u8,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileDependency {
    mod_id: u64,
    /// 3 is "required".
    relation_type: u8,
}

impl File {
    fn sha1(&self) -> Option<String> {
        self.hashes.iter().find(|h| h.algo == 1).map(|h| h.value.to_lowercase())
    }

    fn version(&self) -> String {
        if self.display_name.is_empty() { self.file_name.clone() } else { self.display_name.clone() }
    }
}

/* ── Searching ──────────────────────────────────────────── */

fn class_id(kind: Kind) -> u32 {
    match kind {
        Kind::Mod => 6,
        Kind::ResourcePack => 12,
        Kind::ShaderPack => 6552,
        Kind::World => 17,
    }
}

/// The folder a pack's file goes in, by its project's class.
fn kind_of_class(class: Option<u32>) -> Kind {
    match class {
        Some(12) => Kind::ResourcePack,
        Some(6552) => Kind::ShaderPack,
        _ => Kind::Mod,
    }
}

/// CurseForge's number for a loader. Quilt runs Fabric mods, and most
/// Quilt-ready mods are filed under Fabric.
fn loader_type(kind: &LoaderKind) -> Option<u8> {
    match kind {
        LoaderKind::Vanilla => None,
        LoaderKind::Forge => Some(1),
        LoaderKind::Fabric | LoaderKind::Quilt => Some(4),
        LoaderKind::NeoForge => Some(6),
    }
}

/// Whether a file's tags say it runs on this loader.
fn fits_loader(file: &File, kind: &LoaderKind) -> bool {
    let tagged = |name: &str| file.game_versions.iter().any(|v| v.eq_ignore_ascii_case(name));
    match kind {
        LoaderKind::Vanilla => true,
        LoaderKind::Fabric => tagged("Fabric"),
        LoaderKind::Quilt => tagged("Quilt") || tagged("Fabric"),
        LoaderKind::Forge => tagged("Forge"),
        LoaderKind::NeoForge => tagged("NeoForge"),
    }
}

pub async fn search(instance: &Instance, kind: Kind, query: &str, sort: &str, offset: u32) -> Result<SearchResult, String> {
    if kind == Kind::Mod && instance.loader.kind == LoaderKind::Vanilla {
        return Err("Vanilla instances can't run mods. Pick a mod loader for this instance in its settings first.".into());
    }
    // Popularity, total downloads, last updated, newest, name.
    let (field, order) = match sort {
        "downloads" => (6, "desc"),
        "updated" => (3, "desc"),
        "newest" => (11, "desc"),
        "name" => (4, "asc"),
        _ => (2, "desc"),
    };
    let offset = offset.min(SEARCH_WINDOW - 20);
    let mut url = url::Url::parse(&format!("{API}/mods/search")).expect("valid url");
    url.query_pairs_mut()
        .append_pair("gameId", &MINECRAFT.to_string())
        .append_pair("classId", &class_id(kind).to_string())
        .append_pair("gameVersion", &instance.game_version)
        .append_pair("searchFilter", query.trim())
        .append_pair("sortField", &field.to_string())
        .append_pair("sortOrder", order)
        .append_pair("index", &offset.to_string())
        .append_pair("pageSize", "20");
    if kind == Kind::Mod {
        if let Some(t) = loader_type(&instance.loader.kind) {
            url.query_pairs_mut().append_pair("modLoaderType", &t.to_string());
        }
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Page {
        data: Vec<Mod>,
        pagination: Pagination,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Pagination {
        index: u32,
        total_count: u32,
    }
    let page: Page = get(&url).await?;
    Ok(SearchResult {
        offset: page.pagination.index,
        limit: 20,
        total_hits: page.pagination.total_count.min(SEARCH_WINDOW),
        hits: page.data.into_iter().map(hit).collect(),
    })
}

fn hit(m: Mod) -> Hit {
    Hit {
        project_id: m.id.to_string(),
        url: Some(m.page()),
        icon_url: m.icon(),
        author: m.authors.first().map(|a| a.name.clone()).unwrap_or_default(),
        categories: m.categories.iter().map(|c| c.name.clone()).collect(),
        downloads: m.download_count.max(0.0) as u64,
        slug: m.slug,
        title: m.name,
        description: m.summary,
        date_modified: m.date_modified,
    }
}

/* ── Installing ─────────────────────────────────────────── */

/// The file to install: the newest release or beta for this Minecraft and
/// loader, an alpha only when there is nothing else (as for Modrinth).
fn pick(mut files: Vec<File>, game: &str, loader: Option<&LoaderKind>) -> Option<File> {
    files.retain(|f| f.is_available && f.game_versions.iter().any(|v| v == game) && loader.is_none_or(|l| fits_loader(f, l)));
    files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
    let settled = files.iter().position(|f| f.release_type != 3);
    match settled {
        Some(i) => Some(files.swap_remove(i)),
        None => files.into_iter().next(),
    }
}

/// A project's newest files for this Minecraft (and, for mods, loader; for
/// Quilt every file, as Fabric's run there too).
async fn files_of(project: u64, game: &str, loader: Option<&LoaderKind>) -> Result<Vec<File>, String> {
    let mut url = url::Url::parse(&format!("{API}/mods/{project}/files")).map_err(|e| e.to_string())?;
    url.query_pairs_mut().append_pair("gameVersion", game).append_pair("pageSize", "50");
    if let Some(t) = loader.filter(|l| **l != LoaderKind::Quilt).and_then(loader_type) {
        url.query_pairs_mut().append_pair("modLoaderType", &t.to_string());
    }
    Ok(get::<Data<Vec<File>>>(&url).await?.data)
}

async fn project(id: u64) -> Result<Mod, String> {
    let url = url::Url::parse(&format!("{API}/mods/{id}")).map_err(|e| e.to_string())?;
    Ok(get::<Data<Mod>>(&url).await?.data)
}

fn source(m: &Mod, file: &File) -> Source {
    Source {
        project_id: m.id.to_string(),
        version_id: file.id.to_string(),
        title: m.name.clone(),
        icon_url: m.icon(),
        version_number: Some(file.version()),
        site: Some(SITE.into()),
    }
}

/// Install a project (and, for mods, what it requires) into the instance.
/// Returns the titles installed.
pub async fn install(paths: &Paths, instance: &Instance, kind: Kind, project_id: &str, task: &TaskHandle) -> Result<Vec<String>, String> {
    let first: u64 = project_id.parse().map_err(|_| format!("\"{project_id}\" isn't a CurseForge project."))?;
    let already: HashSet<String> = content::read_index(paths, &instance.id).values().filter(|s| s.site.as_deref() == Some(SITE)).map(|s| s.project_id.clone()).collect();
    let loader = (kind == Kind::Mod).then_some(&instance.loader.kind);
    let dir = paths.game_dir(&instance.id).join(content::folder(kind, &instance.game_version, &instance.version_type));
    let mut installed = Vec::new();
    // Files the author only lets CurseForge's site hand out: (title, page).
    let mut by_hand: Vec<(String, String)> = Vec::new();
    let mut seen = HashSet::new();
    let mut queue: Vec<(u64, bool)> = vec![(first, true)];
    while let Some((id, wanted)) = queue.pop() {
        task.check()?;
        if !seen.insert(id) || (!wanted && already.contains(&id.to_string())) {
            continue;
        }
        let info = project(id).await?;
        let Some(file) = pick(files_of(id, &instance.game_version, loader).await?, &instance.game_version, loader) else {
            if wanted {
                return Err(format!(
                    "{} has no file for {} {} on CurseForge.",
                    info.name,
                    if kind == Kind::Mod { instance.loader.kind.label() } else { "Minecraft" },
                    instance.game_version
                ));
            }
            return Err(format!("{} needs {}, which has no file for this instance on CurseForge.", installed.first().cloned().unwrap_or_default(), info.name));
        };
        if kind == Kind::Mod {
            for dep in file.dependencies.iter().filter(|d| d.relation_type == 3) {
                queue.push((dep.mod_id, false));
            }
        }
        let Some(download) = download_link(&file).await else {
            by_hand.push((info.name.clone(), info.page()));
            continue;
        };
        task.stage(&info.name);
        let name = crate::paths::safe_file_name(&file.file_name);
        download::download_all(vec![FileJob { url: download, path: dir.join(&name), sha1: file.sha1(), size: Some(file.file_length) }], Verify::Hash, task, 2).await?;
        replace_older(paths, &instance.id, &dir, &info.id.to_string(), &name)?;
        content::remember(paths, &instance.id, kind, &name, source(&info, &file))?;
        installed.push(info.name);
    }
    if !by_hand.is_empty() {
        return Err(by_hand_message(&installed, &by_hand));
    }
    Ok(installed)
}

/// Where to download a file: CurseForge's link, or for a file its author
/// keeps off other launchers, the same file (by SHA-1) on Modrinth.
async fn download_link(file: &File) -> Option<String> {
    if let Some(url) = file.download_url.clone().filter(|u| !u.is_empty()) {
        return Some(url);
    }
    let sha1 = file.sha1()?;
    modrinth::same_files(std::slice::from_ref(&sha1)).await.remove(&sha1)
}

/// Another file of the same project (an update) goes away.
fn replace_older(paths: &Paths, instance: &str, dir: &Path, project_id: &str, keep: &str) -> Result<(), String> {
    let mut index = content::read_index(paths, instance);
    let old: Vec<String> = index
        .iter()
        .filter(|(_, s)| s.site.as_deref() == Some(SITE) && s.project_id == project_id)
        .filter_map(|(key, _)| key.split_once('/').map(|(_, f)| f.to_string()))
        .filter(|f| !f.eq_ignore_ascii_case(keep))
        .collect();
    if !old.is_empty() {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let file = entry.file_name().to_string_lossy().to_lowercase();
            if old.iter().any(|o| file == *o || file == format!("{o}.disabled")) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    index.retain(|_, s| !(s.site.as_deref() == Some(SITE) && s.project_id == project_id));
    content::write_index(paths, instance, &index)
}

fn by_hand_message(installed: &[String], by_hand: &[(String, String)]) -> String {
    let added = if installed.is_empty() { String::new() } else { format!("Added {}. ", installed.join(", ")) };
    let list = by_hand.iter().map(|(name, page)| format!("{name} ({page})")).collect::<Vec<_>>().join("\n");
    let (whose, it) = if by_hand.len() == 1 { ("This file's author only allows", "it") } else { ("These files' authors only allow", "them") };
    format!("{added}{whose} downloads from CurseForge's own site, so the launcher can't fetch {it}:\n{list}\nDownload {it} there and drop {it} on this window.")
}

/* ── Modpacks ───────────────────────────────────────────── */

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    #[serde(default)]
    manifest_type: String,
    #[serde(default)]
    pub name: String,
    minecraft: ManifestGame,
    #[serde(default)]
    files: Vec<ManifestFile>,
    #[serde(default)]
    overrides: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestGame {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<ManifestLoader>,
}

#[derive(Debug, Deserialize)]
struct ManifestLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestFile {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
    #[serde(default = "yes")]
    required: bool,
}

/// Whether a file is a CurseForge modpack (a zip with its `manifest.json`).
pub fn is_pack(file: &Path) -> bool {
    std::fs::File::open(file).ok().and_then(|f| zip::ZipArchive::new(f).ok()).is_some_and(|mut z| z.by_name("manifest.json").is_ok())
}

pub fn read(file: &Path) -> Result<Manifest, String> {
    let zip_file = std::fs::File::open(file).map_err(|e| format!("Couldn't open the modpack: {e}"))?;
    let mut zip = zip::ZipArchive::new(zip_file).map_err(|_| "That file isn't a modpack.".to_string())?;
    let mut entry = zip.by_name("manifest.json").map_err(|_| "That file isn't a CurseForge modpack: it has no manifest.json.".to_string())?;
    let mut text = String::new();
    entry.read_to_string(&mut text).map_err(|e| format!("The modpack's manifest couldn't be read: {e}"))?;
    let manifest: Manifest = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("The modpack's manifest is damaged: {e}"))?;
    if manifest.manifest_type != "minecraftModpack" {
        return Err("That file isn't a Minecraft modpack.".into());
    }
    if let Some(overrides) = &manifest.overrides {
        if mrpack::inside(overrides).is_none() {
            return Err("The modpack's overrides folder is outside the pack, so it wasn't imported.".into());
        }
    }
    Ok(manifest)
}

/// The loader a pack names (`forge-47.2.0`, `neoforge-21.1.77`,
/// `fabric-0.16.10`, `quilt-0.26.0`), and its version.
fn loader_of(loaders: &[ManifestLoader]) -> (LoaderKind, Option<String>) {
    let Some(main) = loaders.iter().find(|l| l.primary).or_else(|| loaders.first()) else { return (LoaderKind::Vanilla, None) };
    let (name, version) = main.id.split_once('-').unwrap_or((main.id.as_str(), ""));
    let kind = match name.to_ascii_lowercase().as_str() {
        "forge" => LoaderKind::Forge,
        "neoforge" => LoaderKind::NeoForge,
        "fabric" => LoaderKind::Fabric,
        "quilt" => LoaderKind::Quilt,
        _ => return (LoaderKind::Vanilla, None),
    };
    (kind, Some(version.to_string()).filter(|v| !v.is_empty()))
}

/// Make an instance from a pack. Nothing is downloaded yet.
pub async fn create(paths: &Paths, file: &Path) -> Result<Instance, String> {
    let manifest = read(file)?;
    if !available() {
        return Err(no_key());
    }
    let (kind, wanted) = loader_of(&manifest.minecraft.mod_loaders);
    let name = if manifest.name.trim().is_empty() { "CurseForge modpack" } else { manifest.name.trim() };
    mrpack::make_instance(paths, name, &manifest.minecraft.version, kind, wanted, file, KEPT).await
}

/// Whether an instance came from a CurseForge pack.
pub fn has_pack(paths: &Paths, id: &str) -> bool {
    paths.instance(id).join(KEPT).exists()
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

async fn add_files_in(core: &Core, instance: &Instance, task: &TaskHandle) -> Result<(), String> {
    let pack = core.paths.instance(&instance.id).join(KEPT);
    let manifest = read(&pack)?;
    let wanted: Vec<&ManifestFile> = manifest.files.iter().filter(|f| f.required).collect();

    task.stage("Looking up the mods");
    let mut files: Vec<File> = Vec::new();
    let mut projects: HashMap<u64, Mod> = HashMap::new();
    for chunk in wanted.chunks(500) {
        let ids: Vec<u64> = chunk.iter().map(|f| f.file_id).collect();
        files.extend(post::<Data<Vec<File>>>("/mods/files", serde_json::json!({ "fileIds": ids })).await?.data);
        let ids: Vec<u64> = chunk.iter().map(|f| f.project_id).collect();
        projects.extend(post::<Data<Vec<Mod>>>("/mods", serde_json::json!({ "modIds": ids })).await?.data.into_iter().map(|m| (m.id, m)));
    }
    let missing: Vec<u64> = wanted.iter().filter(|w| !files.iter().any(|f| f.id == w.file_id)).map(|w| w.project_id).collect();
    if !missing.is_empty() {
        let names: Vec<String> = missing.iter().map(|p| projects.get(p).map_or_else(|| format!("project {p}"), |m| m.name.clone())).collect();
        return Err(format!("CurseForge no longer has some of the pack's files: {}.", names.join(", ")));
    }

    let blocked: Vec<String> = files.iter().filter(|f| f.download_url.as_deref().is_none_or(str::is_empty)).filter_map(File::sha1).collect();
    let on_modrinth = modrinth::same_files(&blocked).await;

    let game_dir = core.paths.game_dir(&instance.id);
    let mut jobs = Vec::new();
    let mut remembered = Vec::new();
    let mut by_hand = Vec::new();
    for file in &files {
        let project = projects.get(&file.mod_id);
        let kind = kind_of_class(project.and_then(|p| p.class_id));
        let name = crate::paths::safe_file_name(&file.file_name);
        let path = game_dir.join(content::folder(kind, &instance.game_version, &instance.version_type)).join(&name);
        let link = file.download_url.clone().filter(|u| !u.is_empty()).or_else(|| file.sha1().and_then(|h| on_modrinth.get(&h).cloned()));
        match link {
            Some(url) => jobs.push(FileJob { url, path, sha1: file.sha1(), size: Some(file.file_length) }),
            // Already dropped in by hand on an earlier try.
            None if path.exists() => {}
            None => by_hand.push((project.map_or_else(|| file.file_name.clone(), |p| p.name.clone()), project.map_or_else(String::new, Mod::page))),
        }
        if let Some(p) = project {
            remembered.push((kind, name, source(p, file)));
        }
    }
    task.stage("Mods");
    let parallel = settings::load(&core.paths).parallel_downloads.clamp(1, 16) as usize;
    download::download_all(jobs, Verify::Hash, task, parallel).await?;

    task.stage("Settings and configs");
    let overrides = manifest.overrides.clone().unwrap_or_else(|| "overrides".into());
    let dir = game_dir.clone();
    tokio::task::spawn_blocking(move || mrpack::copy_overrides(&pack, &dir, &[overrides.as_str()])).await.map_err(|e| e.to_string())??;

    let mut index = content::read_index(&core.paths, &instance.id);
    for (kind, name, source) in remembered {
        index.insert(content::index_key(kind, &name), source);
    }
    content::write_index(&core.paths, &instance.id, &index)?;

    if !by_hand.is_empty() {
        return Err(by_hand_message(&[], &by_hand));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(id: u64, date: &str, release: u8, versions: &[&str]) -> File {
        File {
            id,
            mod_id: 1,
            display_name: String::new(),
            file_name: format!("{id}.jar"),
            release_type: release,
            file_date: date.into(),
            file_length: 1,
            download_url: None,
            hashes: Vec::new(),
            game_versions: versions.iter().map(|s| s.to_string()).collect(),
            dependencies: Vec::new(),
            is_available: true,
        }
    }

    #[test]
    fn newest_file_for_the_loader_wins() {
        let id = |f: Option<File>| f.map(|f| f.id);
        let files = || {
            vec![
                file(1, "2026-01-01T00:00:00Z", 1, &["1.21.1", "Fabric"]),
                file(2, "2026-03-01T00:00:00Z", 2, &["1.21.1", "Fabric"]),
                file(3, "2026-04-01T00:00:00Z", 3, &["1.21.1", "Fabric"]),
                file(4, "2026-05-01T00:00:00Z", 1, &["1.21.1", "NeoForge"]),
                file(5, "2026-06-01T00:00:00Z", 1, &["1.21", "Fabric"]),
            ]
        };
        assert_eq!(id(pick(files(), "1.21.1", Some(&LoaderKind::Fabric))), Some(2));
        assert_eq!(id(pick(files(), "1.21.1", Some(&LoaderKind::Quilt))), Some(2));
        assert_eq!(id(pick(files(), "1.21.1", Some(&LoaderKind::NeoForge))), Some(4));
        assert_eq!(id(pick(files(), "1.21.1", Some(&LoaderKind::Forge))), None);
        assert_eq!(id(pick(vec![file(9, "2026", 3, &["1.21.1", "Forge"])], "1.21.1", Some(&LoaderKind::Forge))), Some(9));
        // Resource packs: any loader.
        assert_eq!(id(pick(files(), "1.21.1", None)), Some(4));
    }

    #[test]
    fn reads_api_answers() {
        let page: Data<Vec<File>> = serde_json::from_value(serde_json::json!({ "data": [{
            "id": 5101366, "gameId": 432, "modId": 238222, "isAvailable": true, "displayName": "jei-1.20.1-forge-15.3.0.4.jar",
            "fileName": "jei-1.20.1-forge-15.3.0.4.jar", "releaseType": 1, "fileStatus": 4,
            "hashes": [{ "value": "ABCDEF", "algo": 1 }, { "value": "00", "algo": 2 }],
            "fileDate": "2024-02-01T00:00:00Z", "fileLength": 1234, "downloadCount": 1, "downloadUrl": null,
            "gameVersions": ["1.20.1", "Forge", "Client"], "dependencies": [{ "modId": 1, "relationType": 3 }, { "modId": 2, "relationType": 2 }]
        }]}))
        .unwrap();
        let f = &page.data[0];
        assert_eq!(f.sha1().as_deref(), Some("abcdef"));
        assert!(f.download_url.is_none());
        assert_eq!(f.dependencies.iter().filter(|d| d.relation_type == 3).count(), 1);
        let m: Mod = serde_json::from_value(serde_json::json!({
            "id": 238222, "name": "Just Enough Items (JEI)", "slug": "jei", "summary": "View items", "downloadCount": 3.5e8,
            "logo": { "thumbnailUrl": "https://media.forgecdn.net/x.png" }, "authors": [{ "name": "mezz" }], "classId": 6,
            "links": { "websiteUrl": "https://www.curseforge.com/minecraft/mc-mods/jei" }
        }))
        .unwrap();
        let h = hit(m);
        assert_eq!(h.project_id, "238222");
        assert_eq!(h.downloads, 350_000_000);
        assert_eq!(h.url.as_deref(), Some("https://www.curseforge.com/minecraft/mc-mods/jei"));
    }

    #[test]
    fn pack_loaders() {
        let l = |id: &str| vec![ManifestLoader { id: id.into(), primary: true }];
        assert_eq!(loader_of(&l("forge-47.2.0")), (LoaderKind::Forge, Some("47.2.0".into())));
        assert_eq!(loader_of(&l("neoforge-21.1.77")), (LoaderKind::NeoForge, Some("21.1.77".into())));
        assert_eq!(loader_of(&l("fabric-0.16.10")), (LoaderKind::Fabric, Some("0.16.10".into())));
        assert_eq!(loader_of(&l("quilt-0.26.0")), (LoaderKind::Quilt, Some("0.26.0".into())));
        assert_eq!(loader_of(&[]), (LoaderKind::Vanilla, None));
    }

    #[test]
    fn reads_a_pack() {
        use std::io::Write;
        let path = std::env::temp_dir().join(format!("kryoto-cfpack-{}.zip", std::process::id()));
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("manifest.json", options).unwrap();
        let manifest = serde_json::json!({
            "minecraft": { "version": "1.20.1", "modLoaders": [{ "id": "forge-47.2.0", "primary": true }] },
            "manifestType": "minecraftModpack", "manifestVersion": 1, "name": "Friends", "version": "1.0",
            "files": [{ "projectID": 238222, "fileID": 5101366, "required": true }], "overrides": "overrides"
        });
        zip.write_all(manifest.to_string().as_bytes()).unwrap();
        zip.start_file("overrides/config/jei.toml", options).unwrap();
        zip.write_all(b"x").unwrap();
        zip.finish().unwrap();
        assert!(is_pack(&path));
        let m = read(&path).unwrap();
        assert_eq!(m.name, "Friends");
        assert_eq!(m.files.len(), 1);
        let out = std::env::temp_dir().join(format!("kryoto-cfpack-out-{}", std::process::id()));
        mrpack::copy_overrides(&path, &out, &["overrides"]).unwrap();
        assert_eq!(std::fs::read_to_string(out.join("config").join("jei.toml")).unwrap(), "x");
        let _ = std::fs::remove_dir_all(&out);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn names_files_to_fetch_by_hand() {
        let one = by_hand_message(&["Create".into()], &[("Optifine".into(), "https://www.curseforge.com/x".into())]);
        assert!(one.starts_with("Added Create. This file's author"));
        assert!(one.contains("Optifine (https://www.curseforge.com/x)"));
        let two = by_hand_message(&[], &[("A".into(), "a".into()), ("B".into(), "b".into())]);
        assert!(two.contains("These files' authors") && two.contains("drop them"));
    }
}
