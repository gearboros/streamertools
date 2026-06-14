use keyring::Entry;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use std::path::PathBuf;
use std::fs;
use serde_json::from_slice;
use crate::CLIENT_ID;

#[derive(Serialize, Deserialize)]
struct StoredTokens {
    access_token: String,
    refresh_token: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
}

#[derive(Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Deserialize)]
struct DeviceTokenError {
    message: String,
}

/// Request Device Code from twitch, this is where the user accepts the requested scopes from the application.
pub async fn request_device_code() -> Result<DeviceCodeResponse, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://id.twitch.tv/oauth2/device")
        .form(&[
            ("client_id", CLIENT_ID),
            ("scopes", "user:read:email channel:manage:polls channel:manage:predictions"),
        ])
        .send()
        .await
        .map_err(|e| format!("Request error: {:?}", e))?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        return Err(format!("Device code request failed: {}", err_text));
    }

    resp.json().await.map_err(|e| format!("Parse error: {}", e))
}

/// get access and refresh tokens for future requests
/// access token for auth
/// refresh token to refresh the auth token without the user having to re-auth
pub async fn poll_for_tokens(
    device_code: &str,
    interval: u64,
    expires_in: u64,
) -> Result<(String, String), String> {
    let client = reqwest::Client::new();
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(expires_in);

    loop {
        if start.elapsed() > timeout {
            return Err("Device code expired".to_string());
        }

        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;

        let resp = client
            .post("https://id.twitch.tv/oauth2/token")
            .form(&[
                ("client_id", CLIENT_ID),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .map_err(|e| format!("Request error: {:?}", e))?;

        if resp.status().is_success() {
            let tokens: TokenResponse = resp.json().await.map_err(|e| e.to_string())?;
            return Ok((tokens.access_token, tokens.refresh_token));
        }

        // Check if still pending or actual error
        let error: DeviceTokenError = resp.json().await.map_err(|e| e.to_string())?;

        if error.message == "authorization_pending" {
            // User hasn't authorized yet, continue polling
            continue;
        } else {
            return Err(format!("Authorization failed: {}", error.message));
        }
    }
}

/// Access tokens are short-lived, need refreshing regularly
pub async fn refresh_access_token(refresh_token: &str) -> Result<(String, String), String> {
    let client = reqwest::Client::new();
    let resp = client.post("https://id.twitch.tv/oauth2/token")
        .form(&[
            ("client_id", CLIENT_ID),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        return Err(format!("Token refresh failed: {}", err_text));
    }

    let tokens: TokenResponse = resp.json().await.map_err(|e| e.to_string())?;
    Ok((tokens.access_token, tokens.refresh_token))
}

/// Saves the tokens to a OS provided keyring, falls back to a storage file if that fails.
pub fn save_tokens(access: &str, refresh: &str, path: &PathBuf) -> Result<(), String> {
    info!("Saving tokens...");

    match save_tokens_to_keyring(access, refresh) {
        Ok(()) => {
            info!("Tokens saved to keyring");
            Ok(())
        }
        Err(e) => {
            warn!("Keyring unavailable ({}), falling back to file storage", e);
            save_tokens_to_file(access, refresh, path)
        }
    }
}

fn save_tokens_to_keyring(access: &str, refresh: &str) -> Result<(), String> {
    Entry::new("streamertools", "access_token")
        .map_err(|e| e.to_string())?
        .set_password(access)
        .map_err(|e| e.to_string())?;
    Entry::new("streamertools", "refresh_token")
        .map_err(|e| e.to_string())?
        .set_password(refresh)
        .map_err(|e| e.to_string())
}

fn save_tokens_to_file(access: &str, refresh: &str, path: &PathBuf) -> Result<(), String> {
    let tokens = StoredTokens {
        access_token: access.to_string(),
        refresh_token: refresh.to_string(),
    };
    let json = serde_json::to_string_pretty(&tokens).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| format!("Failed to write token file: {}", e))?;
    info!("Tokens saved to file: {:?}", path);
    Ok(())
}

/// loads token from OS provided keyring, tries to load from file if that fails.
pub fn load_tokens(path: &PathBuf) -> Option<(String, String)> {
    info!("Loading tokens...");

    // Try keyring first
    if let Some(tokens) = load_tokens_from_keyring() {
        info!("Tokens loaded from keyring");
        return Some(tokens);
    }

    // Fall back to file storage
    if let Some(tokens) = load_tokens_from_file(path) {
        return Some(tokens);
    }

    info!("No tokens found in keyring or file");
    None
}

fn load_tokens_from_keyring() -> Option<(String, String)> {
    let access = Entry::new("streamertools", "access_token").ok()?.get_password().ok()?;
    let refresh = Entry::new("streamertools", "refresh_token").ok()?.get_password().ok()?;
    Some((access, refresh))
}

fn load_tokens_from_file(path: &PathBuf) -> Option<(String, String)> {
    let json = fs::read_to_string(path).ok()?;
    let tokens: StoredTokens = serde_json::from_str(&json).ok()?;
    info!("Tokens loaded from file: {:?}", path);
    Some((tokens.access_token, tokens.refresh_token))
}

#[derive(Deserialize)]
struct ValidationResponse {
    user_id: String,
}

/// checks if current access token is valid
/// if invalid tries to refresh
/// if token can't be refreshed, tells the user to re-authenticate
pub async fn validate_token(token: &str) -> Option<String> {
    let resp = reqwest::Client::new()
        .get("https://id.twitch.tv/oauth2/validate")
        .header("Authorization", format!("OAuth {}", token))
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let body: ValidationResponse = resp.json().await.ok()?;
    Some(body.user_id)
}
