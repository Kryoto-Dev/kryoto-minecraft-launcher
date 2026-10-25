//! Mods, resource packs and shader packs from Modrinth (api.modrinth.com,
//! public, no key). Only files that fit the instance are offered: its
//! Minecraft version and, for mods, its loader. Installing a mod also installs
//! the mods it requires.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::content::{self, Kind, Source};
use crate::download::{self, FileJob, Verify};
use crate::instances::{Instance, LoaderKind};
use crate::net;
use crate::paths::Paths;
use crate::tasks::TaskHandle;

const API: &str = "https://api.modrinth.com/v2";

fn project_type(kind: Kind) -> &'static str {
    match kind {
        Kind::Mod => "mod",
        Kind::ResourcePack => "resourcepack",
        Kind::ShaderPack => "shader",
        Kind::World => "mod",
    }
}

/// The loaders whose files run on this instance. Quilt runs Fabric mods.
fn loaders(kind: Kind, instance: &Instance) -> Vec<&'static str> {
    match kind {
        Kind::Mod => match instance.loader.kind {
            LoaderKind::Quilt => vec!["quilt", "fabric"],
            ref k => k.modrinth().into_iter().collect(),
        },
        Kind::ResourcePack => vec!["minecraft"],
        _ => Vec::new(),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    pub downloads: u64,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub date_modified: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub hits: Vec<Hit>,
    pub offset: u32,
    pub limit: u32,
    pub total_hits: u32,
}

pub async fn search(instance: &Instance, kind: Kind, query: &str, sort: &str, offset: u32) -> Result<SearchResult, String> {
    let mut facets: Vec<Vec<String>> = vec![vec![format!("project_type:{}", project_type(kind))], vec![format!("versions:{}", instance.game_version)]];
    let loaders = loaders(kind, instance);
    if kind == Kind::Mod {
        if loaders.is_empty() {
            return Err("Vanilla instances can't run mods. Pick a mod loader for this instance in its settings first.".into());
        }
        facets.push(loaders.iter().map(|l| format!("categories:{l}")).collect());
    }
    let sort = match sort {
        "downloads" | "follows" | "newest" | "updated" => sort,
        _ => "relevance",
    };
    let mut url = url::Url::parse(&format!("{API}/search")).expect("valid url");
    url.query_pairs_mut()
        .append_pair("query", query)
        .append_pair("facets", &serde_json::to_string(&facets).expect("serializable"))
        .append_pair("index", sort)
        .append_pair("offset", &offset.to_string())
        .append_pair("limit", "20");
    net::get_json(url.as_str()).await.map_err(|e| format!("Modrinth search failed. {e}"))
}

#[derive(Clone, Debug, Deserialize)]
struct Version {
    id: String,
    project_id: String,
    version_number: String,
    #[serde(default)]
    version_type: String,
    files: Vec<VersionFile>,
    #[serde(default)]
    dependencies: Vec<Dependency>,
}

#[derive(Clone, Debug, Deserialize)]
struct VersionFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    size: u64,
    hashes: Hashes,
}

#[derive(Clone, Debug, Deserialize)]
struct Hashes {
    sha1: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct Dependency {
    #[serde(default)]
    version_id: Option<String>,
    #[serde(default)]
    project_id: Option<String>,
    dependency_type: String,
}

#[derive(Clone, Debug, Deserialize)]
struct Project {
    title: String,
    #[serde(default)]
    icon_url: Option<String>,
}

async fn compatible_version(instance: &Instance, kind: Kind, project: &str) -> Result<Option<Version>, String> {
    let mut url = url::Url::parse(&format!("{API}/project/{project}/version")).map_err(|e| e.to_string())?;
    url.query_pairs_mut().append_pair("game_versions", &serde_json::to_string(&[&instance.game_version]).expect("serializable"));
    let loaders = loaders(kind, instance);
    if !loaders.is_empty() {
        url.query_pairs_mut().append_pair("loaders", &serde_json::to_string(&loaders).expect("serializable"));
    }
    let versions: Vec<Version> = net::get_json(url.as_str()).await?;
    Ok(pick(versions))
}

/// The file to install from a project's versions (newest first): the newest
/// release or beta, an alpha only when there is nothing else. A release is
/// not preferred over a newer beta: loaders change between their own builds
/// (NeoForge 26.3.0.99 broke Sodium 0.9.2, the release, and not 0.9.3-beta.1),
/// and the newer file is the one made for the loader players get today.
fn pick(versions: Vec<Version>) -> Option<Version> {
    let settled = versions.iter().find(|v| v.version_type != "alpha").cloned();
    settled.or_else(|| versions.into_iter().next())
}

#[derive(Deserialize)]
struct ProjectRow {
    id: String,
    title: String,
    #[serde(default)]
    icon_url: Option<String>,
}

/// Remember where files that came some other way (a modpack) are from on
/// Modrinth, found by their SHA-1, so they show their names, icons and pages.
/// Files Modrinth doesn't know are left as they are.
pub async fn remember_by_hash(paths: &Paths, instance: &str, files: &[(Kind, String, String)]) {
    if files.is_empty() {
        return;
    }
    let hashes: Vec<&str> = files.iter().map(|(_, _, sha1)| sha1.as_str()).collect();
    let body = serde_json::json!({ "hashes": hashes, "algorithm": "sha1" });
    let url = format!("{API}/version_files");
    let Ok(response) = net::client().post(&url).json(&body).send().await.and_then(|r| r.error_for_status()) else { return };
    let Ok(versions) = response.json::<std::collections::HashMap<String, Version>>().await else { return };
    let ids: Vec<&str> = versions.values().map(|v| v.project_id.as_str()).collect::<HashSet<_>>().into_iter().collect();
    let mut projects_url = url::Url::parse(&format!("{API}/projects")).expect("valid url");
    projects_url.query_pairs_mut().append_pair("ids", &serde_json::to_string(&ids).expect("serializable"));
    let projects: Vec<ProjectRow> = net::get_json(projects_url.as_str()).await.unwrap_or_default();
    let mut index = content::read_index(paths, instance);
    for (kind, file, sha1) in files {
        let Some(version) = versions.get(sha1) else { continue };
        let Some(project) = projects.iter().find(|p| p.id == version.project_id) else { continue };
        index.insert(
            content::index_key(*kind, file),
            Source {
                project_id: version.project_id.clone(),
                version_id: version.id.clone(),
                title: project.title.clone(),
                icon_url: project.icon_url.clone(),
                version_number: Some(version.version_number.clone()),
            },
        );
    }
    let _ = content::write_index(paths, instance, &index);
}

/// Install a project (and, for mods, what it requires) into the instance.
/// Returns the titles installed.
pub async fn install(paths: &Paths, instance: &Instance, kind: Kind, project: &str, task: &TaskHandle) -> Result<Vec<String>, String> {
    let mut installed = Vec::new();
    let mut seen = HashSet::new();
    let already: HashSet<String> = content::read_index(paths, &instance.id).values().map(|s| s.project_id.clone()).collect();
    // (project, asked for by the player rather than needed by another mod)
    let mut queue: Vec<(String, bool)> = vec![(project.to_string(), true)];
    while let Some((project_id, wanted)) = queue.pop() {
        task.check()?;
        if !seen.insert(project_id.clone()) || (!wanted && already.contains(&project_id)) {
            continue;
        }
        // A dependency pinned to one version is often pinned to another
        // Minecraft's; the newest file that fits this instance is used instead.
        let version = compatible_version(instance, kind, &project_id).await?;
        let info: Project = net::get_json(&format!("{API}/project/{project_id}")).await?;
        let Some(version) = version else {
            if wanted {
                return Err(format!(
                    "{} has no file for {} {}.",
                    info.title,
                    if kind == Kind::Mod { instance.loader.kind.label() } else { "Minecraft" },
                    instance.game_version
                ));
            }
            return Err(format!("{} needs {}, which has no file for this instance.", installed.first().cloned().unwrap_or_default(), info.title));
        };
        let file = version.files.iter().find(|f| f.primary).or_else(|| version.files.first()).ok_or_else(|| format!("{} has no file to download.", info.title))?;
        task.stage(&info.title);
        let dir = paths.game_dir(&instance.id).join(content::folder(kind, &instance.game_version, &instance.version_type));
        let name = crate::paths::safe_file_name(&file.filename);
        download::download_all(
            vec![FileJob { url: file.url.clone(), path: dir.join(&name), sha1: file.hashes.sha1.clone(), size: Some(file.size) }],
            Verify::Hash,
            task,
            2,
        )
        .await?;
        // Another version of the same project (an update) goes away.
        let old: Vec<String> = content::read_index(paths, &instance.id)
            .iter()
            .filter(|(_, source)| source.project_id == version.project_id)
            .filter_map(|(key, _)| key.split_once('/').map(|(_, f)| f.to_string()))
            .filter(|f| !f.eq_ignore_ascii_case(&name))
            .collect();
        if !old.is_empty() {
            for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                let file = entry.file_name().to_string_lossy().to_lowercase();
                if old.iter().any(|o| file == *o || file == format!("{o}.disabled")) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        let mut index = content::read_index(paths, &instance.id);
        index.retain(|_, s| s.project_id != version.project_id);
        content::write_index(paths, &instance.id, &index)?;
        content::remember(
            paths,
            &instance.id,
            kind,
            &name,
            Source {
                project_id: version.project_id.clone(),
                version_id: version.id.clone(),
                title: info.title.clone(),
                icon_url: info.icon_url.clone(),
                version_number: Some(version.version_number.clone()),
            },
        )?;
        installed.push(info.title);
        if kind == Kind::Mod {
            for dep in version.dependencies.iter().filter(|d| d.dependency_type == "required") {
                let dep_project = match (&dep.project_id, &dep.version_id) {
                    (Some(p), _) => Some(p.clone()),
                    (None, Some(v)) => net::get_json::<Version>(&format!("{API}/version/{v}")).await.ok().map(|ver| ver.project_id),
                    _ => None,
                };
                if let Some(p) = dep_project {
                    queue.push((p, false));
                }
            }
        }
    }
    Ok(installed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(id: &str, kind: &str) -> Version {
        Version { id: id.into(), project_id: "p".into(), version_number: id.into(), version_type: kind.into(), files: Vec::new(), dependencies: Vec::new() }
    }

    #[test]
    fn newest_settled_file_wins() {
        let ids = |v: Option<Version>| v.map(|v| v.id);
        // Sodium for NeoForge 26.3: the newer beta, not the older release.
        assert_eq!(ids(pick(vec![version("0.9.3-beta.1", "beta"), version("0.9.3-alpha.1", "alpha"), version("0.9.2", "release")])).as_deref(), Some("0.9.3-beta.1"));
        assert_eq!(ids(pick(vec![version("2-alpha", "alpha"), version("1", "release")])).as_deref(), Some("1"));
        assert_eq!(ids(pick(vec![version("only-alpha", "alpha")])).as_deref(), Some("only-alpha"));
        assert!(pick(Vec::new()).is_none());
    }
}
