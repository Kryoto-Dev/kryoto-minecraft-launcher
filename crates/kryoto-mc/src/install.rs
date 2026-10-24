//! Everything a version needs on disk before it can start: its version files,
//! the client jar, libraries, natives, assets, the logging config and Java.
//!
//! Installing and starting use the same code: starting runs it with quick
//! checks (sizes), so a missing or half-downloaded file is fetched on the way
//! to Play, and Repair runs it with full SHA-1 checks.

use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::download::{self, FileJob, Verify};
use crate::java;
use crate::meta::{self, AssetIndex, Features, Resolved};
use crate::paths::Paths;
use crate::tasks::TaskHandle;

/// A version ready to start.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub resolved: Resolved,
    pub java: PathBuf,
    /// The required Java major version, for the "wrong Java" hint.
    pub java_major: u32,
    pub classpath: Vec<PathBuf>,
    pub natives: Vec<(PathBuf, Vec<String>)>,
    pub client_jar: PathBuf,
    pub assets_index: String,
    /// What `${game_assets}` means: the virtual folder or the game's
    /// `resources` for versions from before 1.6, the assets folder otherwise.
    pub game_assets: PathBuf,
    /// The log4j argument, pointing at Mojang's (patched) config.
    pub logging_arg: Option<String>,
}

pub struct Options<'a> {
    pub verify: Verify,
    pub parallel: usize,
    /// A Java the player chose; `None` uses Mojang's runtime for the version.
    pub java: Option<PathBuf>,
    pub game_dir: Option<&'a Path>,
}

pub async fn prepare(paths: &Paths, version_id: &str, opts: Options<'_>, task: &TaskHandle) -> Result<Prepared, String> {
    task.stage("Version files");
    let resolved = meta::resolve(paths, version_id).await?;
    task.check()?;
    let json = &resolved.json;
    let features = Features::new();

    // The client jar: the version's own when an installer made one (Forge
    // copies it in), else the vanilla one at the root of the chain.
    let own_jar = paths.version_jar(&resolved.json.id);
    let base = resolved.base().to_string();
    let jar_id = json.jar.clone().unwrap_or_else(|| base.clone());
    let base_jar = paths.version_jar(&jar_id);
    let mut jobs: Vec<FileJob> = Vec::new();
    match meta::client_download(&resolved) {
        Some(client) => jobs.push(FileJob { url: client.url.clone(), path: base_jar.clone(), sha1: client.sha1.clone(), size: client.size }),
        None if !base_jar.exists() => return Err(format!("Minecraft {jar_id} has no game file to download.")),
        None => {}
    }
    let client_jar = if resolved.chain.len() > 1 && own_jar.exists() { own_jar } else { base_jar };

    // Libraries and natives.
    let mut classpath = Vec::new();
    let mut natives = Vec::new();
    let mut made_by_installer = Vec::new();
    let mut seen = HashSet::new();
    for lib in &json.libraries {
        let Some(files) = meta::library_files(paths, lib, &features) else { continue };
        if let Some(job) = files.classpath {
            if seen.insert(job.path.clone()) {
                classpath.push(job.path.clone());
                if job.url.is_empty() {
                    made_by_installer.push(job.path.clone());
                } else {
                    jobs.push(job);
                }
            }
        }
        if let Some((job, exclude)) = files.natives {
            natives.push((job.path.clone(), exclude));
            jobs.push(job);
        }
    }
    if let Some(missing) = made_by_installer.iter().find(|p| !p.exists()) {
        return Err(format!(
            "A file the mod loader's installer makes is missing ({}). Reinstall the loader from the instance's settings.",
            missing.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
        ));
    }

    // Mojang's logging config (the log4j fix for 1.7 to 1.18).
    let mut logging_arg = None;
    if let Some(client) = json.logging.as_ref().and_then(|l| l.client.as_ref()) {
        let path = paths.assets().join("log_configs").join(crate::paths::safe_file_name(&client.file.id));
        jobs.push(FileJob { url: client.file.url.clone(), path: path.clone(), sha1: client.file.sha1.clone(), size: client.file.size });
        logging_arg = Some(client.argument.replace("${path}", &path.display().to_string()));
    }

    task.stage("Libraries");
    download::download_all(jobs, opts.verify, task, opts.parallel).await?;
    task.check()?;

    // Assets.
    task.stage("Assets");
    let index_ref = json
        .asset_index
        .clone()
        .ok_or_else(|| format!("The version file for {} names no assets.", json.id))?;
    let index_path = paths.asset_index(&index_ref.id);
    download::download_all(
        vec![FileJob { url: index_ref.url.clone(), path: index_path.clone(), sha1: index_ref.sha1.clone(), size: index_ref.size }],
        Verify::Hash,
        task,
        1,
    )
    .await?;
    let index: AssetIndex = serde_json::from_slice(&std::fs::read(&index_path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("The asset list for {} couldn't be read: {e}", index_ref.id))?;
    let objects: Vec<FileJob> = index
        .objects
        .values()
        .map(|o| FileJob {
            url: format!("{}{}/{}", meta::RESOURCES_URL, &o.hash[..2], o.hash),
            path: paths.asset_object(&o.hash),
            sha1: Some(o.hash.clone()),
            size: Some(o.size),
        })
        .collect();
    task.stage("Assets");
    // Asset objects are named by their hash; a size check is enough even for
    // a repair, unless the player asked for the full check.
    download::download_all(objects, opts.verify, task, opts.parallel).await?;
    task.check()?;

    let game_assets = if index.map_to_resources {
        let dir = opts.game_dir.map(|g| g.join("resources")).unwrap_or_else(|| paths.assets().join("virtual").join(&index_ref.id));
        copy_assets(paths, &index, &dir)?;
        dir
    } else if index.is_virtual {
        let dir = paths.assets().join("virtual").join(&index_ref.id);
        copy_assets(paths, &index, &dir)?;
        dir
    } else {
        paths.assets()
    };

    // Java.
    let java_major = meta::java_major(json);
    let java = match opts.java {
        Some(path) => path,
        None => java::ensure_runtime(paths, &meta::java_component(json), java_major, task, opts.parallel).await?,
    };

    Ok(Prepared { resolved, java, java_major, classpath, natives, client_jar, assets_index: index_ref.id, game_assets, logging_arg })
}

/// Old versions read assets by name from a folder, not by hash.
fn copy_assets(paths: &Paths, index: &AssetIndex, dir: &Path) -> Result<(), String> {
    for (name, object) in &index.objects {
        let target = crate::paths::join_relative(dir, name);
        if target.metadata().map(|m| m.len() == object.size).unwrap_or(false) {
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(paths.asset_object(&object.hash), &target).map_err(|e| format!("Couldn't copy the asset {name}: {e}"))?;
    }
    Ok(())
}

/// Unpack the natives jars into `dir` (skipped when the same jars were
/// unpacked there last time).
pub fn extract_natives(natives: &[(PathBuf, Vec<String>)], dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let marker = dir.join(".kryoto-natives");
    let wanted: String = natives.iter().map(|(p, _)| p.display().to_string()).collect::<Vec<_>>().join("\n");
    if std::fs::read_to_string(&marker).ok().as_deref() == Some(wanted.as_str()) {
        return Ok(());
    }
    for (jar, exclude) in natives {
        let file = std::fs::File::open(jar).map_err(|e| format!("Couldn't open {}: {e}", jar.display()))?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("{} is damaged: {e}", jar.display()))?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            let name = entry.name().to_string();
            if entry.is_dir() || exclude.iter().any(|x| name.starts_with(x.as_str())) || name.starts_with("META-INF/") {
                continue;
            }
            let target = crate::paths::join_relative(dir, &name);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            // A native still loaded by a running game can't be replaced; the
            // copy already there is the same file.
            if let Err(e) = std::fs::write(&target, &bytes) {
                if !target.exists() {
                    return Err(format!("Couldn't unpack {name}: {e}"));
                }
            }
        }
    }
    let _ = std::fs::write(&marker, wanted);
    Ok(())
}
