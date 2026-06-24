use crate::CLIENT_ID;
use reqwest::{RequestBuilder, Response};
use serde::{Deserialize, Serialize};
use tracing::error;

#[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PredictionStatus {
    Resolved,
    Active,
    Locked,
    Canceled,
}

impl PredictionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PredictionStatus::Resolved => "RESOLVED",
            PredictionStatus::Active => "ACTIVE",
            PredictionStatus::Locked => "LOCKED",
            PredictionStatus::Canceled => "CANCELED",
        }
    }
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

pub async fn create_poll(
    client: &reqwest::Client,
    access_token: &str,
    request: CreatePollRequest,
) -> Result<PollStateData, String> {
    let builder = client
        .post("https://api.twitch.tv/helix/polls")
        .json(&request);
    let resp = add_headers_and_send(access_token, builder).await?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        error!("Request: {:?}, error: {}", request, err_text);
        return Err(format!("Create poll failed: {}", err_text));
    }

    extract_poll_response(resp).await
}

async fn extract_poll_response(resp: Response) -> Result<PollStateData, String> {
    resp.json::<PollStateResponse>()
        .await
        .map_err(|e| {
            error!("Parse error: {}", e);
            e.to_string()
        })?
        .data
        .into_iter()
        .next()
        .ok_or_else(|| "Empty response from Twitch".to_string())
}

pub async fn end_poll(
    client: &reqwest::Client,
    broadcaster_id: &str,
    poll_id: &str,
    access_token: &str,
) -> Result<(), String> {
    let uri = format!(
        "https://api.twitch.tv/helix/polls?broadcaster_id={}&id={}&status=TERMINATED",
        broadcaster_id, poll_id
    );
    let builder = client.patch(uri);
    let resp = add_headers_and_send(access_token, builder).await?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        error!("Error: {}", err_text);
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
    client: &reqwest::Client,
    access_token: &str,
    request: CreatePredictionRequest,
) -> Result<CreatePredictionResponseData, String> {
    let builder = client
        .post("https://api.twitch.tv/helix/predictions")
        .json(&request);
    let resp = add_headers_and_send(access_token, builder).await?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        error!("Request: {:?}, error: {}", request, err_text);
        return Err(format!("Create prediction failed: {}", err_text));
    }

    extract_prediction_response(resp).await
}

async fn extract_prediction_response(
    resp: Response,
) -> Result<CreatePredictionResponseData, String> {
    resp.json::<CreatePredictionResponse>()
        .await
        .map_err(|e| {
            error!("Parse error: {}", e);
            e.to_string()
        })?
        .data
        .into_iter()
        .next()
        .ok_or_else(|| "Empty response from Twitch".to_string())
}

#[derive(Serialize, Debug)]
pub struct EndPredictionRequest {
    pub broadcaster_id: String,
    pub outcome_id: String,
    pub prediction_id: String,
}

pub async fn end_prediction(
    client: &reqwest::Client,
    request: EndPredictionRequest,
    access_token: &str,
) -> Result<(), String> {
    set_prediction_state(client, request, access_token, PredictionStatus::Resolved)
        .await
        .map_err(|e| e.to_string())
}

pub async fn lock_prediction(
    client: &reqwest::Client,
    request: EndPredictionRequest,
    access_token: &str,
) -> Result<(), String> {
    set_prediction_state(client, request, access_token, PredictionStatus::Locked)
        .await
        .map_err(|e| e.to_string())
}

pub async fn cancel_prediction(
    client: &reqwest::Client,
    request: EndPredictionRequest,
    access_token: &str,
) -> Result<(), String> {
    set_prediction_state(client, request, access_token, PredictionStatus::Canceled)
        .await
        .map_err(|e| e.to_string())
}

async fn set_prediction_state(
    client: &reqwest::Client,
    request: EndPredictionRequest,
    access_token: &str,
    status: PredictionStatus,
) -> Result<(), String> {
    let uri = format!(
        "https://api.twitch.tv/helix/predictions?broadcaster_id={}&id={}&status={}&winning_outcome_id={}",
        request.broadcaster_id,
        request.prediction_id,
        status.as_str(),
        request.outcome_id
    );
    let builder = client.patch(uri);
    let resp = add_headers_and_send(access_token, builder).await?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        error!("Request: {:?}, error: {}", request, err_text);
        return Err(format!("Ending prediction failed: {}", err_text));
    }

    Ok(())
}

pub async fn check_prediction(
    client: &reqwest::Client,
    broadcaster_id: &str,
    prediction_id: &str,
    access_token: &str,
) -> Result<CreatePredictionResponseData, String> {
    let uri = format!(
        "https://api.twitch.tv/helix/predictions?broadcaster_id={}&id={}",
        broadcaster_id, prediction_id
    );
    let builder = client.get(uri);
    let resp = add_headers_and_send(access_token, builder).await?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        error!("Error: {}", err_text);
        return Err(format!("Checking prediction failed: {}", err_text));
    }

    extract_prediction_response(resp).await
}

#[derive(Deserialize, Debug, Clone)]
pub struct PollChoiceState {
    pub id: String,
    pub title: String,
    pub votes: i32,
    pub channel_point_votes: i32,
}

impl PollChoiceState {
    pub fn popular_votes(&self) -> i32 {
        self.votes - self.channel_point_votes
    }
}

#[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PollPhase {
    Active,
    Terminated,
    Archived,
    Completed,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PollStateData {
    pub id: String,
    pub broadcaster_id: String,
    pub winning_outcome_id: Option<String>,
    pub choices: Vec<PollChoiceState>,
    pub status: PollPhase,
    pub started_at: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct PollStateResponse {
    data: Vec<PollStateData>,
}

pub async fn check_poll(
    client: &reqwest::Client,
    broadcaster_id: &str,
    poll_id: &str,
    access_token: &str,
) -> Result<PollStateData, String> {
    let uri = format!(
        "https://api.twitch.tv/helix/polls?broadcaster_id={}&id={}",
        broadcaster_id, poll_id
    );
    let builder = client.get(uri);
    let resp = add_headers_and_send(access_token, builder).await?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        error!("Error: {}", err_text);
        return Err(format!("Checking poll failed: {}", err_text));
    }

    extract_poll_response(resp).await
}

async fn add_headers_and_send(
    access_token: &str,
    builder: RequestBuilder,
) -> Result<Response, String> {
    let resp = builder
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Client-Id", CLIENT_ID)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?;
    Ok(resp)
}
