//! Profiles: the name a player plays under.
//!
//! A profile is a name and nothing else: no account, no sign-in, and no
//! ownership is claimed or faked. Profiles play singleplayer and servers that
//! run in offline mode (`online-mode=false`, as play.kryo.to does); servers
//! that check accounts turn them away, and skins and Realms are unavailable.
//!
//! A name a paid account owns is the exception on offline servers too:
//! play.kryo.to (FastLogin) and many others ask the game to prove that name,
//! and the game, with no account to prove it with, is turned away with
//! Minecraft's own "Invalid session (Try restarting your game and the
//! launcher)". Restarting never helps, so the launcher says so before it
//! happens (`name_owned_by_account`, KB-0314).

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

/// Whether a paid Minecraft account has this name, from Mojang's public name
/// lookup: `Some(true)` taken, `Some(false)` free, `None` when Mojang could
/// not be asked (offline, rate limited). Names are compared without case,
/// as Mojang and servers do.
pub async fn name_owned_by_account(name: &str) -> Option<bool> {
    let name = name.trim();
    valid_name(name).ok()?;
    let url = format!("https://api.mojang.com/users/profiles/minecraft/{name}");
    let response = crate::net::client().get(&url).timeout(std::time::Duration::from_secs(8)).send().await.ok()?;
    owned_from_status(response.status().as_u16())
}

/// Mojang answers 200 with the account for a taken name, and 404 (204 before
/// 2023) for a free one. Anything else says nothing either way.
fn owned_from_status(status: u16) -> Option<bool> {
    match status {
        200 => Some(true),
        204 | 404 => Some(false),
        _ => None,
    }
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
    fn mojang_answers_read_as_taken_free_or_unknown() {
        assert_eq!(owned_from_status(200), Some(true));
        assert_eq!(owned_from_status(404), Some(false));
        assert_eq!(owned_from_status(204), Some(false));
        assert_eq!(owned_from_status(429), None);
        assert_eq!(owned_from_status(500), None);
    }

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
