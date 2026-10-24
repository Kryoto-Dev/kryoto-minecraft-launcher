//! One HTTP client for the whole app, and errors a player can read.

use std::sync::OnceLock;
use std::time::Duration;

use serde::de::DeserializeOwned;

pub const USER_AGENT: &str = concat!(
    "Kryoto-Dev/kryoto-minecraft-launcher/",
    env!("CARGO_PKG_VERSION"),
    " (+https://kryo.to)"
);

pub fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(60))
            .build()
            .expect("the HTTP client builds")
    })
}

/// The host of a URL, for messages ("Couldn't reach piston-meta.mojang.com").
pub fn host(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| url.to_string())
}

/// What went wrong with a request, in words.
pub fn explain(url: &str, e: &reqwest::Error) -> String {
    let host = host(url);
    if e.is_timeout() {
        format!("{host} took too long to answer.")
    } else if e.is_connect() {
        format!("Couldn't reach {host}. Check your internet connection.")
    } else if let Some(status) = e.status() {
        format!("{host} answered {status}.")
    } else if e.is_decode() {
        format!("{host} sent something this launcher couldn't read.")
    } else {
        format!("Downloading from {host} failed: {e}")
    }
}

/// GET a JSON document, with a few retries for flaky connections. A 4xx answer
/// is final; timeouts, connection failures and 5xx are retried.
pub async fn get_json<T: DeserializeOwned>(url: &str) -> Result<T, String> {
    let mut last = String::new();
    for attempt in 0..3u64 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(800 * attempt * attempt)).await;
        }
        match client().get(url).send().await.and_then(|r| r.error_for_status()) {
            Ok(response) => match response.json::<T>().await {
                Ok(value) => return Ok(value),
                Err(e) => last = explain(url, &e),
            },
            Err(e) => {
                last = explain(url, &e);
                if e.status().is_some_and(|s| s.is_client_error()) {
                    break;
                }
            }
        }
    }
    Err(last)
}

/// GET a text document (maven metadata), with the same retries.
pub async fn get_text(url: &str) -> Result<String, String> {
    let mut last = String::new();
    for attempt in 0..3u64 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(800 * attempt * attempt)).await;
        }
        match client().get(url).send().await.and_then(|r| r.error_for_status()) {
            Ok(response) => match response.text().await {
                Ok(text) => return Ok(text),
                Err(e) => last = explain(url, &e),
            },
            Err(e) => {
                last = explain(url, &e);
                if e.status().is_some_and(|s| s.is_client_error()) {
                    break;
                }
            }
        }
    }
    Err(last)
}
