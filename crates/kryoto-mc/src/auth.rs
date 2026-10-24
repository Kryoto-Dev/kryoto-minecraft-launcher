//! Profiles: the name a player plays under.
//!
//! A profile is a name and nothing else: no account, no sign-in, and no
//! ownership is claimed or faked. Profiles play singleplayer and servers that
//! run in offline mode (`online-mode=false`, as play.kryo.to does); servers
//! that check accounts turn them away, and skins and Realms are unavailable.

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};

use crate::paths::Paths;
use crate::tasks::now;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// The UUID an offline-mode server gives this name, without dashes.
    pub id: String,
    pub name: String,
    pub added: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profiles {
    #[serde(alias = "accounts")]
    pub profiles: Vec<Profile>,
    pub active: Option<String>,
}

pub fn load(paths: &Paths) -> Profiles {
    std::fs::read(paths.accounts()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save(paths: &Paths, profiles: &Profiles) -> Result<(), String> {
    std::fs::create_dir_all(&paths.root).map_err(|e| e.to_string())?;
    let tmp = paths.accounts().with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(profiles).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, paths.accounts()).map_err(|e| format!("Couldn't save your profiles: {e}"))
}

pub fn valid_name(name: &str) -> Result<(), String> {
    if name.len() < 3 || name.len() > 16 {
        return Err("A name is 3 to 16 characters long.".into());
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("A name can only use letters, numbers and _.".into());
    }
    Ok(())
}

/// The UUID an offline-mode server gives this name (Java's
/// `UUID.nameUUIDFromBytes("OfflinePlayer:" + name)`), so worlds and
/// server inventories follow the name.
pub fn offline_uuid(name: &str) -> String {
    let mut hash: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    hash[6] = (hash[6] & 0x0f) | 0x30;
    hash[8] = (hash[8] & 0x3f) | 0x80;
    hex::encode(hash)
}

pub fn add(paths: &Paths, name: &str) -> Result<Profile, String> {
    let name = name.trim();
    valid_name(name)?;
    let mut all = load(paths);
    let id = offline_uuid(name);
    let profile = match all.profiles.iter().find(|p| p.id == id) {
        Some(existing) => existing.clone(),
        None => {
            let p = Profile { id: id.clone(), name: name.into(), added: now() };
            all.profiles.push(p.clone());
            p
        }
    };
    all.active = Some(id);
    save(paths, &all)?;
    Ok(profile)
}

pub fn remove(paths: &Paths, id: &str) -> Result<(), String> {
    let mut all = load(paths);
    all.profiles.retain(|p| p.id != id);
    if all.active.as_deref() == Some(id) {
        all.active = all.profiles.first().map(|p| p.id.clone());
    }
    save(paths, &all)
}

pub fn set_active(paths: &Paths, id: &str) -> Result<(), String> {
    let mut all = load(paths);
    if !all.profiles.iter().any(|p| p.id == id) {
        return Err("That profile isn't there any more.".into());
    }
    all.active = Some(id.into());
    save(paths, &all)
}

/// The active profile, which the game starts as.
pub fn active(paths: &Paths) -> Result<Profile, String> {
    let all = load(paths);
    all.active
        .as_ref()
        .and_then(|id| all.profiles.iter().find(|p| &p.id == id))
        .cloned()
        .ok_or_else(|| "Make a profile first: pick the name you play under.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuids_match_the_server() {
        // What an offline-mode server computes for "Notch".
        assert_eq!(offline_uuid("Notch"), "b50ad385829d3141a2167e7d7539ba7f");
    }

    #[test]
    fn names() {
        assert!(valid_name("Steve_01").is_ok());
        assert!(valid_name("ab").is_err());
        assert!(valid_name("has space").is_err());
        assert!(valid_name("waytoolongforaname").is_err());
    }

    #[test]
    fn old_accounts_files_still_read() {
        let old = r#"{"accounts":[{"id":"b50ad385829d3141a2167e7d7539ba7f","kind":"offline","name":"Notch","xuid":null,"skinUrl":null,"added":1}],"active":"b50ad385829d3141a2167e7d7539ba7f"}"#;
        let p: Profiles = serde_json::from_str(old).unwrap();
        assert_eq!(p.profiles[0].name, "Notch");
    }
}
