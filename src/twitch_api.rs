use crate::twitch_types::*;
use crate::CLIENT_ID;
use reqwest::{RequestBuilder, Response};
use tracing::error;

macro_rules! create_helix_url {
    ($path:literal) => {
        concat!("https://api.twitch.tv/helix/", $path)
    };
}

const POLL_URL: &str = create_helix_url!("polls");
const PREDICTION_URL: &str = create_helix_url!("predictions");

pub async fn create_poll(
    client: &reqwest::Client,
    access_token: &str,
    request: CreatePollRequest,
) -> Result<PollStateData, String> {
    let builder = client.post(POLL_URL).json(&request);
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
) -> Result<PollStateData, String> {
    let uri = format!("{POLL_URL}?broadcaster_id={broadcaster_id}&id={poll_id}&status=TERMINATED");
    let builder = client.patch(uri);
    let resp = add_headers_and_send(access_token, builder).await?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        error!("Error: {}", err_text);
        return Err(format!("Ending poll failed: {}", err_text));
    }

    extract_poll_response(resp).await
}

pub async fn create_prediction(
    client: &reqwest::Client,
    access_token: &str,
    request: CreatePredictionRequest,
) -> Result<CreatePredictionResponseData, String> {
    let builder = client.post(PREDICTION_URL).json(&request);
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

pub async fn end_prediction(
    client: &reqwest::Client,
    request: EndPredictionRequest,
    access_token: &str,
) -> Result<CreatePredictionResponseData, String> {
    set_prediction_state(client, request, access_token, PredictionStatus::Resolved).await
}

pub async fn lock_prediction(
    client: &reqwest::Client,
    request: EndPredictionRequest,
    access_token: &str,
) -> Result<CreatePredictionResponseData, String> {
    set_prediction_state(client, request, access_token, PredictionStatus::Locked).await
}

pub async fn cancel_prediction(
    client: &reqwest::Client,
    request: EndPredictionRequest,
    access_token: &str,
) -> Result<CreatePredictionResponseData, String> {
    set_prediction_state(client, request, access_token, PredictionStatus::Canceled).await
}

async fn set_prediction_state(
    client: &reqwest::Client,
    request: EndPredictionRequest,
    access_token: &str,
    status: PredictionStatus,
) -> Result<CreatePredictionResponseData, String> {
    let mut uri = format!(
        "{PREDICTION_URL}?broadcaster_id={}&id={}&status={}",
        request.broadcaster_id,
        request.prediction_id,
        status.as_str(),
    );
    // Only resolving a prediction supplies a winner; lock/cancel deliberately pass an
    // empty `outcome_id`, so we omit `winning_outcome_id` in those cases.
    if !request.outcome_id.is_empty() {
        uri = format!("{}&winning_outcome_id={}", uri, request.outcome_id);
    }
    let builder = client.patch(uri);
    let resp = add_headers_and_send(access_token, builder).await?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        error!("Request: {:?}, error: {}", request, err_text);
        return Err(format!("Ending prediction failed: {}", err_text));
    }

    extract_prediction_response(resp).await
}

pub async fn check_prediction(
    client: &reqwest::Client,
    broadcaster_id: &str,
    prediction_id: &str,
    access_token: &str,
) -> Result<CreatePredictionResponseData, String> {
    let uri = format!("{PREDICTION_URL}?broadcaster_id={broadcaster_id}&id={prediction_id}");
    let builder = client.get(uri);
    let resp = add_headers_and_send(access_token, builder).await?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        error!("Error: {}", err_text);
        return Err(format!("Checking prediction failed: {}", err_text));
    }

    extract_prediction_response(resp).await
}

pub async fn check_poll(
    client: &reqwest::Client,
    broadcaster_id: &str,
    poll_id: &str,
    access_token: &str,
) -> Result<PollStateData, String> {
    let uri = format!("{POLL_URL}?broadcaster_id={broadcaster_id}&id={poll_id}");
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
