use streamertools::CLIENT_ID;
use keyring::Entry;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use std::path::PathBuf;
use std::fs;
use serde_json::from_slice;

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

#[derive(Serialize, Debug)]
pub struct CreatePollRequest {
    pub broadcaster_id: String,
    pub title: String,
    pub choices: Vec<PollChoice>,
    pub duration: usize,
    pub channel_points_voting_enabled: bool,
    pub channel_points_per_vote: usize,
}

#[derive(Serialize, Debug)]
pub struct PollChoice {
    pub title: String,
}

#[derive(Debug, Deserialize)]
struct PollResponse {
    data: Vec<PollId>,
}

#[derive(Debug, Deserialize)]
struct PollId {
    id: String,
}

pub async fn create_poll(
    access_token: &str,
    request: CreatePollRequest,
) -> Result<String, String> {
    let resp = reqwest::Client::new()
        .post("https://api.twitch.tv/helix/polls")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Client-Id", CLIENT_ID)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        return Err(format!("Create poll failed: {}", err_text));
    }

    let parsed: PollResponse = resp.json::<PollResponse>().await.map_err(|e| e.to_string())?;
    Ok(parsed.data.first().unwrap().id.clone())
}

pub async fn end_poll(broadcaster_id: &str, poll_id: &str, access_token: &str)-> Result<(), String> {
    let uri = format!("https://api.twitch.tv/helix/polls?broadcaster_id={}&id={}&status=TERMINATED", broadcaster_id, poll_id);
    let resp = reqwest::Client::new()
        .patch(uri)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Client-Id", CLIENT_ID)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        return Err(format!("Ending poll failed: {}", err_text));
    }

    Ok(())
}

#[derive(Serialize, Debug)]
pub struct CreatePredictionRequest {
    pub broadcaster_id: String,
    pub title: String,
    pub outcomes: Vec<PollChoice>,
    pub prediction_window: usize,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Predictor {
    user_id: String,
    user_login: String,
    user_name: String,
    channel_points_used: i32,
    channel_points_won: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PredictionOutcome {
    pub id: String,
    pub title: String,
    pub users: i32,
    pub channel_points: i32,
    pub top_predictors: Option<Vec<Predictor>>,
    pub color: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CreatePredictionResponseData {
    pub id: String,
    pub broadcaster_id: String,
    pub winning_outcome_id: Option<String>,
    pub outcomes: Vec<PredictionOutcome>,
    pub status: PredictionStatus,
    pub created_at: Option<String>,
    pub ended_at: Option<String>,
    pub locked_at: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct CreatePredictionResponse {
    data: Vec<CreatePredictionResponseData>,
}

pub async fn create_prediction(
    access_token: &str,
    request: CreatePredictionRequest,
) -> Result<CreatePredictionResponseData, String> {
    dbg!(&request);
    let resp = reqwest::Client::new()
        .post("https://api.twitch.tv/helix/predictions")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Client-Id", CLIENT_ID)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        return Err(format!("Create prediction failed: {}", err_text));
    }
    // log before returning
    // let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    // println!("{}", std::str::from_utf8(&bytes).map_err(|e| e.to_string())?);
    // let parsed: CreatePredictionResponse =
    //     from_slice(&bytes).map_err(|e| e.to_string())?;
    // Ok(parsed.data.first().unwrap().clone())

    let parsed: CreatePredictionResponse = resp.json::<CreatePredictionResponse>().await.map_err(|e| e.to_string())?;
    Ok(parsed.data.first().unwrap().clone())
}

#[derive(Serialize, Debug)]
pub struct EndPredictionRequest {
    pub broadcaster_id: String,
    pub outcome_id: String,
    pub prediction_id: String,
}

pub async fn end_prediction(request: EndPredictionRequest, access_token: &str) -> Result<(), String> {
    set_prediction_state(request, access_token, PredictionStatus::Resolved).await.map_err(|e| e.to_string())
}

pub async fn lock_prediction(request: EndPredictionRequest, access_token: &str) -> Result<(), String> {
    set_prediction_state(request, access_token, PredictionStatus::Locked).await.map_err(|e| e.to_string())
}

pub async fn cancel_prediction(request: EndPredictionRequest, access_token: &str) -> Result<(), String> {
    set_prediction_state(request, access_token, PredictionStatus::Canceled).await.map_err(|e| e.to_string())
}

async fn set_prediction_state(request: EndPredictionRequest, access_token: &str, status: PredictionStatus) -> Result<(), String> {
    let uri = format!("https://api.twitch.tv/helix/predictions?broadcaster_id={}&id={}&status={}&winning_outcome_id={}",
                      request.broadcaster_id,
                      request.prediction_id,
                        status.as_str(),
                      request.outcome_id);
    let resp = reqwest::Client::new()
        .patch(uri)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Client-Id", CLIENT_ID)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        return Err(format!("Ending prediction failed: {}", err_text));
    }

    Ok(())
}

#[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PredictionStatus {
    Resolved,
    Active,
    Locked,
    Canceled
}

impl PredictionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PredictionStatus::Resolved => "RESOLVED",
            PredictionStatus::Active   => "ACTIVE",
            PredictionStatus::Locked   => "LOCKED",
            PredictionStatus::Canceled => "CANCELED",
        }
    }
}

pub async fn check_prediction(broadcaster_id: &str, prediction_id: &str, access_token: &String) -> Result<CreatePredictionResponseData, String> {
    let uri = format!("https://api.twitch.tv/helix/predictions?broadcaster_id={}&id={}", broadcaster_id, prediction_id);
    let resp = reqwest::Client::new()
        .get(uri)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Client-Id", CLIENT_ID)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        return Err(format!("Checking prediction failed: {}", err_text));
    }

    let parsed: CreatePredictionResponse = resp.json::<CreatePredictionResponse>().await.map_err(|e| e.to_string())?;
    Ok(parsed.data.first().unwrap().clone())
}