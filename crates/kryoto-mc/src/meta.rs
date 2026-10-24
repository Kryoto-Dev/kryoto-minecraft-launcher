//! Mojang's version metadata: the version list, version files, and the rules
//! in them (which libraries and arguments apply on this computer).
//!
//! Sources are Mojang's own: `piston-meta.mojang.com` for the list and the
//! version files, `piston-data`/`libraries.minecraft.net`/
//! `resources.download.minecraft.net` for the files they name. Loader
//! versions (Fabric, Quilt, Forge, NeoForge) are version files of the same
//! shape that name a vanilla version in `inheritsFrom`; `resolve` merges the
//! chain into one.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::net;
use crate::paths::Paths;

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
pub const LIBRARIES_URL: &str = "https://libraries.minecraft.net/";
pub const RESOURCES_URL: &str = "https://resources.download.minecraft.net/";

/* ── The version list ───────────────────────────────────── */

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub time: String,
    pub release_time: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub compliance_level: Option<u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub latest: Latest,
    pub versions: Vec<ManifestVersion>,
}

/// The version list: fresh from Mojang when online, the last copy when not
/// (so installed versions still start with no connection).
pub async fn manifest(paths: &Paths, refresh: bool) -> Result<Manifest, String> {
    let cached = paths.cache().join("version_manifest_v2.json");
    if !refresh {
        if let Some(m) = read_fresh(&cached, 60 * 60) {
            return Ok(m);
        }
    }
    match net::get_json::<Manifest>(MANIFEST_URL).await {
        Ok(m) => {
            let _ = std::fs::create_dir_all(paths.cache());
            let _ = std::fs::write(&cached, serde_json::to_vec(&m).unwrap_or_default());
            Ok(m)
        }
        Err(e) => match std::fs::read(&cached).ok().and_then(|b| serde_json::from_slice(&b).ok()) {
            Some(m) => Ok(m),
            None => Err(format!("Couldn't load the version list. {e}")),
        },
    }
}

fn read_fresh<T: for<'de> Deserialize<'de>>(path: &Path, max_age_secs: u64) -> Option<T> {
    let age = std::fs::metadata(path).ok()?.modified().ok()?.elapsed().ok()?.as_secs();
    if age > max_age_secs {
        return None;
    }
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/* ── Version files ──────────────────────────────────────── */

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherits_from: Option<String>,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_class: Option<String>,
    /// Before 1.13: one string of game arguments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minecraft_arguments: Option<String>,
    /// 1.13 and later: lists with rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Arguments>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloads: Option<Downloads>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java_version: Option<JavaVersion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logging: Option<Logging>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_time: Option<String>,
    /// The jar to run, when it is another version's (some Forge files).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jar: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Ruled { rules: Vec<Rule>, value: StringOrList },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StringOrList {
    One(String),
    Many(Vec<String>),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Rule {
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<OsRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OsRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Library {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloads: Option<LibraryDownloads>,
    /// A maven repository to fetch `name` from (Fabric, Quilt, old Forge).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<Rule>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extract: Option<Extract>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// Old Forge: whether the client needs it (absent means yes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clientreq: Option<bool>,
    /// Old Forge: SHA-1s the file may have.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksums: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct LibraryDownloads {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<Artifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Artifact {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Extract {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub total_size: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Downloads {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<Artifact>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub component: String,
    pub major_version: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Logging {
    #[serde(default)]
    pub client: Option<LoggingClient>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LoggingClient {
    pub argument: String,
    pub file: LoggingFile,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LoggingFile {
    pub id: String,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AssetIndex {
    #[serde(default)]
    pub objects: HashMap<String, AssetObject>,
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
    #[serde(default)]
    pub map_to_resources: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

/// The version file for `id`: from disk when it is there and current, from
/// Mojang otherwise. Loader versions are only ever on disk (their installers
/// write them).
pub async fn version_json(paths: &Paths, id: &str) -> Result<VersionJson, String> {
    let file = paths.version_json(id);
    let manifest_entry = match manifest(paths, false).await {
        Ok(m) => m.versions.into_iter().find(|v| v.id == id),
        Err(_) => None,
    };
    if let Ok(bytes) = std::fs::read(&file) {
        let current = match manifest_entry.as_ref().and_then(|v| v.sha1.as_ref()) {
            Some(expected) => crate::download::sha1_file(&file).map(|h| &h == expected).unwrap_or(false),
            None => true,
        };
        if current {
            return serde_json::from_slice(&bytes).map_err(|e| format!("The version file for {id} is damaged ({e}). Reinstall the version."));
        }
    }
    let Some(entry) = manifest_entry else {
        return Err(format!("Version {id} isn't installed and Mojang doesn't list it."));
    };
    let bytes = net::client()
        .get(&entry.url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Couldn't download the version file for {id}. {}", net::explain(&entry.url, &e)))?
        .bytes()
        .await
        .map_err(|e| net::explain(&entry.url, &e))?;
    let parsed: VersionJson = serde_json::from_slice(&bytes).map_err(|e| format!("Mojang's file for {id} couldn't be read: {e}"))?;
    std::fs::create_dir_all(paths.version_dir(id)).map_err(|e| e.to_string())?;
    std::fs::write(&file, &bytes).map_err(|e| format!("Couldn't save the version file: {e}"))?;
    Ok(parsed)
}

/// A version with everything it inherits merged in. `chain` is the ids from
/// the version itself down to the vanilla one at the root.
#[derive(Clone, Debug)]
pub struct Resolved {
    pub json: VersionJson,
    pub chain: Vec<String>,
}

impl Resolved {
    /// The vanilla version at the root of the chain.
    pub fn base(&self) -> &str {
        self.chain.last().map(String::as_str).unwrap_or(&self.json.id)
    }
}

pub async fn resolve(paths: &Paths, id: &str) -> Result<Resolved, String> {
    let mut chain = vec![id.to_string()];
    let mut stack = vec![version_json(paths, id).await?];
    while let Some(parent) = stack.last().and_then(|v| v.inherits_from.clone()) {
        if chain.contains(&parent) || chain.len() > 8 {
            return Err(format!("The version file for {id} inherits from itself."));
        }
        chain.push(parent.clone());
        stack.push(version_json(paths, &parent).await?);
    }
    let mut merged = stack.pop().expect("at least one version");
    while let Some(child) = stack.pop() {
        merged = merge(merged, child);
    }
    Ok(Resolved { json: merged, chain })
}

/// `child` over `parent`, the way the official launcher combines them:
/// the child's values win, argument lists are appended, and the child's
/// libraries come first and replace the parent's of the same name.
pub fn merge(parent: VersionJson, child: VersionJson) -> VersionJson {
    // Only across files: one file can list a library twice on purpose, once
    // per operating system with different versions.
    let theirs: HashSet<String> = child.libraries.iter().map(|l| library_key(&l.name)).collect();
    let mut libraries = child.libraries;
    libraries.extend(parent.libraries.into_iter().filter(|l| !theirs.contains(&library_key(&l.name))));
    let arguments = match (parent.arguments, child.arguments) {
        (Some(mut p), Some(c)) => {
            p.game.extend(c.game);
            p.jvm.extend(c.jvm);
            Some(p)
        }
        (p, c) => c.or(p),
    };
    VersionJson {
        id: child.id,
        inherits_from: None,
        kind: child.kind.or(parent.kind),
        main_class: child.main_class.or(parent.main_class),
        minecraft_arguments: child.minecraft_arguments.or(parent.minecraft_arguments),
        arguments,
        libraries,
        asset_index: child.asset_index.or(parent.asset_index),
        assets: child.assets.or(parent.assets),
        downloads: child.downloads.or(parent.downloads),
        java_version: child.java_version.or(parent.java_version),
        logging: child.logging.or(parent.logging),
        release_time: child.release_time.or(parent.release_time),
        jar: child.jar.or(parent.jar),
    }
}

/// `group:artifact[:classifier]`, without the version: two versions of one
/// library must not both end up on the classpath.
fn library_key(name: &str) -> String {
    let parts: Vec<&str> = name.split('@').next().unwrap_or(name).split(':').collect();
    match parts.as_slice() {
        [g, a, _v, c, ..] => format!("{g}:{a}:{c}"),
        [g, a, ..] => format!("{g}:{a}"),
        _ => name.to_string(),
    }
}

/* ── Rules ──────────────────────────────────────────────── */

pub fn os_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

/// Mojang's arch names: `x86` is 32-bit only.
pub fn os_arch() -> &'static str {
    if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86_64"
    }
}

/// The bitness `${arch}` stands for in native classifiers.
fn arch_bits() -> &'static str {
    if cfg!(target_pointer_width = "32") {
        "32"
    } else {
        "64"
    }
}

fn os_version() -> String {
    sysinfo::System::os_version().unwrap_or_default()
}

pub type Features = HashMap<String, bool>;

/// Whether a rule list allows this computer with these features. No rules
/// means allowed; with rules, nothing is allowed until a rule allows it.
pub fn allowed(rules: Option<&Vec<Rule>>, features: &Features) -> bool {
    let Some(rules) = rules else { return true };
    if rules.is_empty() {
        return true;
    }
    let mut allow = false;
    for rule in rules {
        if rule_matches(rule, features) {
            allow = rule.action == "allow";
        }
    }
    allow
}

fn rule_matches(rule: &Rule, features: &Features) -> bool {
    if let Some(os) = &rule.os {
        if os.name.as_deref().is_some_and(|n| n != os_name()) {
            return false;
        }
        if os.arch.as_deref().is_some_and(|a| a != os_arch()) {
            return false;
        }
        if let Some(pattern) = &os.version {
            let Ok(re) = regex::Regex::new(pattern) else { return false };
            if !re.is_match(&os_version()) {
                return false;
            }
        }
    }
    if let Some(wanted) = &rule.features {
        for (name, value) in wanted {
            if features.get(name).copied().unwrap_or(false) != *value {
                return false;
            }
        }
    }
    true
}

/* ── Libraries ──────────────────────────────────────────── */

/// `group:artifact:version[:classifier][@ext]` as a path in a maven repository.
pub fn maven_path(name: &str) -> Option<String> {
    let (coords, ext) = match name.split_once('@') {
        Some((c, e)) => (c, e),
        None => (name, "jar"),
    };
    let parts: Vec<&str> = coords.split(':').collect();
    let (group, artifact, version, classifier) = match parts.as_slice() {
        [g, a, v] => (*g, *a, *v, None),
        [g, a, v, c] => (*g, *a, *v, Some(*c)),
        _ => return None,
    };
    let file = match classifier {
        Some(c) => format!("{artifact}-{version}-{c}.{ext}"),
        None => format!("{artifact}-{version}.{ext}"),
    };
    Some(format!("{}/{artifact}/{version}/{file}", group.replace('.', "/")))
}

/// What one library needs on this computer: a jar for the classpath, and/or
/// a natives jar to unpack.
#[derive(Clone, Debug)]
pub struct LibraryFiles {
    pub classpath: Option<crate::download::FileJob>,
    pub natives: Option<(crate::download::FileJob, Vec<String>)>,
}

fn artifact_job(paths: &Paths, artifact: &Artifact, fallback_path: Option<String>) -> Option<crate::download::FileJob> {
    let path = artifact.path.clone().or(fallback_path)?;
    let url = if artifact.url.is_empty() { format!("{LIBRARIES_URL}{path}") } else { artifact.url.clone() };
    Some(crate::download::FileJob { url, path: paths.library(&path), sha1: artifact.sha1.clone(), size: artifact.size.filter(|s| *s > 0) })
}

pub fn library_files(paths: &Paths, lib: &Library, features: &Features) -> Option<LibraryFiles> {
    if !allowed(lib.rules.as_ref(), features) || lib.clientreq == Some(false) {
        return None;
    }
    let mut files = LibraryFiles { classpath: None, natives: None };

    if let Some(classifier) = lib.natives.as_ref().and_then(|n| n.get(os_name())) {
        let classifier = classifier.replace("${arch}", arch_bits());
        let artifact = lib.downloads.as_ref().and_then(|d| d.classifiers.as_ref()).and_then(|c| c.get(&classifier));
        let fallback = maven_path(&format!("{}:{classifier}", lib.name));
        let job = match artifact {
            Some(a) => artifact_job(paths, a, fallback),
            None => fallback.map(|p| crate::download::FileJob {
                url: format!("{}{p}", repo(lib)),
                path: paths.library(&p),
                sha1: None,
                size: None,
            }),
        };
        if let Some(job) = job {
            files.natives = Some((job, lib.extract.as_ref().map(|e| e.exclude.clone()).unwrap_or_default()));
        }
        // Old-style natives entries carry no classpath jar of their own.
        if lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()).is_none() {
            return Some(files);
        }
    }

    let path = maven_path(&lib.name);
    files.classpath = match lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
        Some(artifact) => {
            // An empty URL means "the installer made this file" (Forge's
            // patched client): nothing to fetch, but it belongs on the classpath.
            if artifact.url.is_empty() && lib.url.is_none() {
                artifact.path.clone().or(path).map(|p| crate::download::FileJob {
                    url: String::new(),
                    path: paths.library(&p),
                    sha1: artifact.sha1.clone(),
                    size: artifact.size.filter(|s| *s > 0),
                })
            } else {
                artifact_job(paths, artifact, path)
            }
        }
        None => path.map(|p| crate::download::FileJob {
            url: format!("{}{p}", repo(lib)),
            path: paths.library(&p),
            sha1: lib.sha1.clone().or_else(|| lib.checksums.as_ref().and_then(|c| (c.len() == 1).then(|| c[0].clone()))),
            size: lib.size.filter(|s| *s > 0),
        }),
    };
    Some(files)
}

fn repo(lib: &Library) -> String {
    let base = lib.url.clone().unwrap_or_else(|| LIBRARIES_URL.into());
    // Forge's old maven address now redirects; go straight to the new one.
    let base = base
        .replace("http://files.minecraftforge.net/maven/", "https://maven.minecraftforge.net/")
        .replace("https://files.minecraftforge.net/maven/", "https://maven.minecraftforge.net/")
        .replace("http://", "https://");
    if base.ends_with('/') {
        base
    } else {
        format!("{base}/")
    }
}

/// The client jar the version runs: its own, or (for loader versions that
/// have none) the one of the version it is built on.
pub fn client_download(resolved: &Resolved) -> Option<&Artifact> {
    resolved.json.downloads.as_ref().and_then(|d| d.client.as_ref())
}

/// The major Java version a version wants. Versions from before Mojang added
/// the field ran on Java 8.
pub fn java_major(json: &VersionJson) -> u32 {
    json.java_version.as_ref().map(|j| j.major_version).unwrap_or(8)
}

pub fn java_component(json: &VersionJson) -> String {
    json.java_version.as_ref().map(|j| j.component.clone()).unwrap_or_else(|| "jre-legacy".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_paths() {
        assert_eq!(maven_path("org.lwjgl:lwjgl:3.3.3").unwrap(), "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar");
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3:natives-windows").unwrap(),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar"
        );
        assert_eq!(maven_path("de.oceanlabs.mcp:mcp_config:1.20.1@zip").unwrap(), "de/oceanlabs/mcp/mcp_config/1.20.1/mcp_config-1.20.1.zip");
        assert!(maven_path("broken").is_none());
    }

    #[test]
    fn rules() {
        let features = Features::new();
        let only = |name: &str| vec![Rule { action: "allow".into(), os: Some(OsRule { name: Some(name.into()), arch: None, version: None }), features: None }];
        assert!(allowed(Some(&only(os_name())), &features));
        assert!(!allowed(Some(&only("nothing-like-this")), &features));
        let all_but = vec![
            Rule { action: "allow".into(), os: None, features: None },
            Rule { action: "disallow".into(), os: Some(OsRule { name: Some(os_name().into()), arch: None, version: None }), features: None },
        ];
        assert!(!allowed(Some(&all_but), &features));
        let demo = vec![Rule { action: "allow".into(), os: None, features: Some(HashMap::from([("is_demo_user".into(), true)])) }];
        assert!(!allowed(Some(&demo), &features));
        assert!(allowed(Some(&demo), &HashMap::from([("is_demo_user".into(), true)])));
    }

    #[test]
    fn child_libraries_replace_parent_ones() {
        let lib = |name: &str| Library { name: name.into(), ..Default::default() };
        let parent = VersionJson { id: "1.20.1".into(), libraries: vec![lib("org.ow2.asm:asm:9.3"), lib("a:b:1")], main_class: Some("vanilla".into()), ..Default::default() };
        let child = VersionJson { id: "fabric".into(), libraries: vec![lib("org.ow2.asm:asm:9.6")], main_class: Some("fabric".into()), ..Default::default() };
        let merged = merge(parent, child);
        let names: Vec<_> = merged.libraries.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["org.ow2.asm:asm:9.6", "a:b:1"]);
        assert_eq!(merged.main_class.as_deref(), Some("fabric"));
    }
}
