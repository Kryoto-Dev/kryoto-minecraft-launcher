//! Accounts: Microsoft (verified ownership) and offline profiles.
//!
//! Microsoft sign-in uses the OAuth device-code flow: the launcher shows a
//! short code, the player enters it on microsoft.com/link in their own
//! browser, and Microsoft hands this app a token. The launcher never sees or
//! stores a password. The token is exchanged, as Mojang documents, for Xbox
//! Live, then XSTS, then a Minecraft token, and the account is only added when
//! Minecraft's services confirm the game is owned and a profile exists.
//! Refresh tokens live in the system's credential store (Windows Credential
//! Manager, Linux Secret Service), never in a file.
//!
//! Offline profiles are a name and nothing else: no ownership is claimed or
//! faked. They play singleplayer and servers that run in offline mode
//! (`online-mode=false`); servers that check accounts turn them away, and
//! skins and Realms need a Microsoft account.
//!
//! The Azure app ID comes from `KRYOTO_MSA_CLIENT_ID` at build time. Microsoft
//! only lets an app reach Minecraft's services once Mojang has approved its ID
//! (docs/microsoft-sign-in.md); a build without one says so instead of failing
//! halfway through.

use std::time::Duration;

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::net;
use crate::paths::Paths;
use crate::tasks::now;

pub fn client_id() -> Option<&'static str> {
    option_env!("KRYOTO_MSA_CLIENT_ID").filter(|s| !s.trim().is_empty())
}

const KEYRING_SERVICE: &str = "to.kryo.minecraft";
const SCOPE: &str = "XboxLive.signin offline_access";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    Microsoft,
    Offline,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// The profile UUID without dashes.
    pub id: String,
    pub kind: AccountKind,
    pub name: String,
    #[serde(default)]
    pub xuid: Option<String>,
    #[serde(default)]
    pub skin_url: Option<String>,
    pub added: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Accounts {
    pub accounts: Vec<Account>,
    pub active: Option<String>,
}

pub fn load(paths: &Paths) -> Accounts {
    std::fs::read(paths.accounts()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save(paths: &Paths, accounts: &Accounts) -> Result<(), String> {
    std::fs::create_dir_all(&paths.root).map_err(|e| e.to_string())?;
    let tmp = paths.accounts().with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(accounts).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, paths.accounts()).map_err(|e| format!("Couldn't save your accounts: {e}"))
}

/* ── Offline ────────────────────────────────────────────── */

pub fn valid_offline_name(name: &str) -> Result<(), String> {
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

pub fn add_offline(paths: &Paths, name: &str) -> Result<Account, String> {
    let name = name.trim();
    valid_offline_name(name)?;
    let mut all = load(paths);
    let id = offline_uuid(name);
    if let Some(existing) = all.accounts.iter().find(|a| a.id == id && a.kind == AccountKind::Offline) {
        let existing = existing.clone();
        all.active = Some(existing.id.clone());
        save(paths, &all)?;
        return Ok(existing);
    }
    let account = Account { id: id.clone(), kind: AccountKind::Offline, name: name.into(), xuid: None, skin_url: None, added: now() };
    all.accounts.push(account.clone());
    all.active = Some(id);
    save(paths, &all)?;
    Ok(account)
}

/* ── Microsoft ──────────────────────────────────────────── */

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    #[serde(skip_serializing)]
    pub device_code: String,
    #[serde(skip_serializing)]
    pub interval: u64,
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    #[serde(default = "five")]
    interval: u64,
}
fn five() -> u64 {
    5
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct TokenError {
    error: String,
    #[serde(default)]
    error_description: String,
}

/// What we keep in the credential store for one Microsoft account.
#[derive(Serialize, Deserialize)]
struct Secret {
    refresh_token: String,
    #[serde(default)]
    minecraft_token: Option<String>,
    #[serde(default)]
    minecraft_expires: u64,
}

fn no_client() -> String {
    "Microsoft sign-in isn't set up in this build of the launcher. Use an offline profile, or get the launcher from kryo.to.".into()
}

pub async fn start_device_code() -> Result<DeviceCode, String> {
    let client = client_id().ok_or_else(no_client)?;
    let url = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
    let response = net::client()
        .post(url)
        .form(&[("client_id", client), ("scope", SCOPE)])
        .send()
        .await
        .map_err(|e| net::explain(url, &e))?;
    if !response.status().is_success() {
        let body: TokenError = response.json().await.unwrap_or(TokenError { error: "unknown".into(), error_description: String::new() });
        return Err(format!("Microsoft refused to start the sign-in ({}). {}", body.error, first_line(&body.error_description)));
    }
    let d: DeviceCodeResponse = response.json().await.map_err(|e| net::explain(url, &e))?;
    Ok(DeviceCode { user_code: d.user_code, verification_uri: d.verification_uri, expires_in: d.expires_in, device_code: d.device_code, interval: d.interval })
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("")
}

/// Wait for the player to finish in their browser, then sign in to Minecraft.
/// `cancelled` is polled so closing the dialog stops the wait.
pub async fn finish_device_code(paths: &Paths, code: &DeviceCode, cancelled: impl Fn() -> bool) -> Result<Account, String> {
    let client = client_id().ok_or_else(no_client)?;
    let url = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
    let deadline = std::time::Instant::now() + Duration::from_secs(code.expires_in);
    let mut interval = code.interval.max(1);
    let token = loop {
        if cancelled() {
            return Err(crate::tasks::CANCELLED.into());
        }
        if std::time::Instant::now() > deadline {
            return Err("The code expired before sign-in finished. Start again.".into());
        }
        tokio::time::sleep(Duration::from_secs(interval)).await;
        let response = net::client()
            .post(url)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", client),
                ("device_code", code.device_code.as_str()),
            ])
            .send()
            .await;
        let Ok(response) = response else { continue };
        if response.status().is_success() {
            break response.json::<TokenResponse>().await.map_err(|e| net::explain(url, &e))?;
        }
        let err: TokenError = response.json().await.unwrap_or(TokenError { error: "unknown".into(), error_description: String::new() });
        match err.error.as_str() {
            "authorization_pending" => {}
            "slow_down" => interval += 5,
            "authorization_declined" => return Err("Sign-in was declined in the browser.".into()),
            "expired_token" => return Err("The code expired before sign-in finished. Start again.".into()),
            other => return Err(format!("Microsoft sign-in failed ({other}). {}", first_line(&err.error_description))),
        }
    };
    let refresh = token.refresh_token.ok_or("Microsoft didn't allow staying signed in. Try again.")?;
    let session = minecraft_login(&token.access_token).await?;
    store_account(paths, session, refresh)
}

/// The Minecraft side of a sign-in.
struct Session {
    token: String,
    expires_in: u64,
    uuid: String,
    name: String,
    xuid: Option<String>,
    skin: Option<String>,
}

async fn minecraft_login(msa_token: &str) -> Result<Session, String> {
    // Xbox Live.
    let url = "https://user.auth.xboxlive.com/user/authenticate";
    let xbl: serde_json::Value = post_json(
        url,
        &json!({
            "Properties": { "AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={msa_token}") },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        }),
        None,
    )
    .await
    .map_err(|e| format!("Xbox Live sign-in failed. {e}"))?;
    let xbl_token = xbl["Token"].as_str().ok_or("Xbox Live sent no token.")?.to_string();
    let uhs = xbl["DisplayClaims"]["xui"][0]["uhs"].as_str().ok_or("Xbox Live sent no user hash.")?.to_string();

    // XSTS, for Minecraft's services.
    let url = "https://xsts.auth.xboxlive.com/xsts/authorize";
    let response = net::client()
        .post(url)
        .json(&json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
        }))
        .send()
        .await
        .map_err(|e| net::explain(url, &e))?;
    if response.status().as_u16() == 401 {
        let body: serde_json::Value = response.json().await.unwrap_or_default();
        return Err(xsts_error(body["XErr"].as_u64().unwrap_or(0)));
    }
    let xsts: serde_json::Value = response.error_for_status().map_err(|e| net::explain(url, &e))?.json().await.map_err(|e| net::explain(url, &e))?;
    let xsts_token = xsts["Token"].as_str().ok_or("Xbox Live sent no XSTS token.")?.to_string();
    let xuid = xsts["DisplayClaims"]["xui"][0]["xid"].as_str().map(str::to_string);

    // Minecraft.
    let url = "https://api.minecraftservices.com/authentication/login_with_xbox";
    let response = net::client()
        .post(url)
        .json(&json!({ "identityToken": format!("XBL3.0 x={uhs};{xsts_token}") }))
        .send()
        .await
        .map_err(|e| net::explain(url, &e))?;
    if response.status().as_u16() == 403 {
        return Err("Minecraft's services haven't approved this launcher's sign-in yet. Play with an offline profile until they do.".into());
    }
    let mc: serde_json::Value = response.error_for_status().map_err(|e| net::explain(url, &e))?.json().await.map_err(|e| net::explain(url, &e))?;
    let token = mc["access_token"].as_str().ok_or("Minecraft sent no token.")?.to_string();
    let expires_in = mc["expires_in"].as_u64().unwrap_or(86_400);

    // Ownership, then the profile (name, UUID, skin).
    let url = "https://api.minecraftservices.com/entitlements/mcstore";
    let owned: serde_json::Value = get_bearer(url, &token).await?;
    let items = owned["items"].as_array().cloned().unwrap_or_default();
    let profile_url = "https://api.minecraftservices.com/minecraft/profile";
    let profile = net::client().get(profile_url).bearer_auth(&token).send().await.map_err(|e| net::explain(profile_url, &e))?;
    if profile.status().as_u16() == 404 {
        return Err(if items.is_empty() {
            "This Microsoft account doesn't own Minecraft: Java Edition. Buy it at minecraft.net, or play with an offline profile.".into()
        } else {
            "This account owns Minecraft but has no profile yet. Choose a name at minecraft.net, then sign in again.".into()
        });
    }
    let profile: serde_json::Value = profile.error_for_status().map_err(|e| net::explain(profile_url, &e))?.json().await.map_err(|e| net::explain(profile_url, &e))?;
    let uuid = profile["id"].as_str().ok_or("Minecraft sent a profile with no ID.")?.to_string();
    let name = profile["name"].as_str().ok_or("Minecraft sent a profile with no name.")?.to_string();
    let skin = profile["skins"].as_array().and_then(|s| s.iter().find(|s| s["state"] == "ACTIVE")).and_then(|s| s["url"].as_str()).map(str::to_string);
    Ok(Session { token, expires_in, uuid, name, xuid, skin })
}

fn xsts_error(code: u64) -> String {
    match code {
        2148916233 => "This Microsoft account has no Xbox profile yet. Sign in once at xbox.com to create one, then try again.".into(),
        2148916235 => "Xbox Live isn't available in this account's country.".into(),
        2148916236 | 2148916237 => "This account needs adult verification on xbox.com before it can play.".into(),
        2148916238 => "This is a child account. An adult needs to add it to a Microsoft family first (account.microsoft.com/family).".into(),
        _ => format!("Xbox Live refused the sign-in (code {code})."),
    }
}

async fn post_json(url: &str, body: &serde_json::Value, bearer: Option<&str>) -> Result<serde_json::Value, String> {
    let mut request = net::client().post(url).json(body).header("Accept", "application/json");
    if let Some(token) = bearer {
        request = request.bearer_auth(token);
    }
    request
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| net::explain(url, &e))?
        .json()
        .await
        .map_err(|e| net::explain(url, &e))
}

async fn get_bearer(url: &str, token: &str) -> Result<serde_json::Value, String> {
    net::client()
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| net::explain(url, &e))?
        .json()
        .await
        .map_err(|e| net::explain(url, &e))
}

fn keyring_entry(uuid: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, &format!("microsoft:{uuid}")).map_err(|e| format!("The system's credential store isn't available: {e}"))
}

fn store_account(paths: &Paths, session: Session, refresh: String) -> Result<Account, String> {
    let secret = Secret { refresh_token: refresh, minecraft_token: Some(session.token), minecraft_expires: now() + session.expires_in };
    keyring_entry(&session.uuid)?
        .set_password(&serde_json::to_string(&secret).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Couldn't save the sign-in to the system's credential store: {e}"))?;
    let mut all = load(paths);
    all.accounts.retain(|a| !(a.id == session.uuid && a.kind == AccountKind::Microsoft));
    let account = Account {
        id: session.uuid.clone(),
        kind: AccountKind::Microsoft,
        name: session.name,
        xuid: session.xuid,
        skin_url: session.skin,
        added: now(),
    };
    all.accounts.push(account.clone());
    all.active = Some(session.uuid);
    save(paths, &all)?;
    Ok(account)
}

pub fn remove(paths: &Paths, id: &str) -> Result<(), String> {
    let mut all = load(paths);
    if let Some(account) = all.accounts.iter().find(|a| a.id == id) {
        if account.kind == AccountKind::Microsoft {
            if let Ok(entry) = keyring_entry(id) {
                let _ = entry.delete_credential();
            }
        }
    }
    all.accounts.retain(|a| a.id != id);
    if all.active.as_deref() == Some(id) {
        all.active = all.accounts.first().map(|a| a.id.clone());
    }
    save(paths, &all)
}

pub fn set_active(paths: &Paths, id: &str) -> Result<(), String> {
    let mut all = load(paths);
    if !all.accounts.iter().any(|a| a.id == id) {
        return Err("That account isn't signed in any more.".into());
    }
    all.active = Some(id.into());
    save(paths, &all)
}

/// What the game is started with.
#[derive(Clone, Debug)]
pub struct Launch {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    pub xuid: String,
    pub user_type: &'static str,
    pub online: bool,
}

/// The active account, ready to start the game with: a Microsoft account's
/// Minecraft token is renewed when it is close to expiring.
pub async fn for_launch(paths: &Paths) -> Result<Launch, String> {
    let all = load(paths);
    let account = all
        .active
        .as_ref()
        .and_then(|id| all.accounts.iter().find(|a| &a.id == id))
        .cloned()
        .ok_or("Add an account first: sign in with Microsoft, or make an offline profile.")?;
    match account.kind {
        AccountKind::Offline => Ok(Launch {
            name: account.name,
            uuid: account.id,
            access_token: "0".into(),
            xuid: "0".into(),
            user_type: "legacy",
            online: false,
        }),
        AccountKind::Microsoft => {
            let entry = keyring_entry(&account.id)?;
            let secret: Secret = entry
                .get_password()
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .ok_or_else(|| format!("{} needs to sign in again.", account.name))?;
            if let (Some(token), true) = (&secret.minecraft_token, secret.minecraft_expires > now() + 600) {
                return Ok(Launch {
                    name: account.name,
                    uuid: account.id,
                    access_token: token.clone(),
                    xuid: account.xuid.unwrap_or_else(|| "0".into()),
                    user_type: "msa",
                    online: true,
                });
            }
            let refreshed = refresh(&secret.refresh_token).await.map_err(|e| format!("{} needs to sign in again. {e}", account.name))?;
            let session = minecraft_login(&refreshed.access_token).await?;
            let launch = Launch {
                name: session.name.clone(),
                uuid: session.uuid.clone(),
                access_token: session.token.clone(),
                xuid: session.xuid.clone().unwrap_or_else(|| "0".into()),
                user_type: "msa",
                online: true,
            };
            store_account(paths, session, refreshed.refresh_token.unwrap_or(secret.refresh_token))?;
            Ok(launch)
        }
    }
}

async fn refresh(refresh_token: &str) -> Result<TokenResponse, String> {
    let client = client_id().ok_or_else(no_client)?;
    let url = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
    let response = net::client()
        .post(url)
        .form(&[("grant_type", "refresh_token"), ("client_id", client), ("refresh_token", refresh_token), ("scope", SCOPE)])
        .send()
        .await
        .map_err(|e| net::explain(url, &e))?;
    if !response.status().is_success() {
        let err: TokenError = response.json().await.unwrap_or(TokenError { error: "unknown".into(), error_description: String::new() });
        return Err(format!("Microsoft said {}.", err.error));
    }
    response.json().await.map_err(|e| net::explain(url, &e))
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
    fn offline_names() {
        assert!(valid_offline_name("Steve_01").is_ok());
        assert!(valid_offline_name("ab").is_err());
        assert!(valid_offline_name("has space").is_err());
        assert!(valid_offline_name("waytoolongforaname").is_err());
    }
}
