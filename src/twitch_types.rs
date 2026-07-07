use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Default, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PredictionStatus {
    Resolved,
    #[default]
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

#[derive(Serialize, Debug)]
pub struct CreatePredictionRequest {
    pub broadcaster_id: String,
    pub title: String,
    pub outcomes: Vec<PollChoice>,
    pub prediction_window: usize,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default, Eq, PartialEq)]
pub struct Predictor {
    pub user_name: String,
    pub channel_points_used: i64,
    pub channel_points_won: i64,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default, Eq, PartialEq)]
pub struct PredictionOutcome {
    pub id: String,
    pub title: String,
    pub users: i64,
    pub channel_points: i64,
    pub top_predictors: Option<Vec<Predictor>>,
    pub color: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct CreatePredictionResponseData {
    pub id: String,
    pub winning_outcome_id: Option<String>,
    pub outcomes: Vec<PredictionOutcome>,
    pub status: PredictionStatus,
}

#[derive(Deserialize, Debug)]
pub struct CreatePredictionResponse {
    pub(crate) data: Vec<CreatePredictionResponseData>,
}

#[derive(Serialize, Debug)]
pub struct EndPredictionRequest {
    pub broadcaster_id: String,
    pub outcome_id: String,
    pub prediction_id: String,
}

#[derive(Deserialize, Default, Serialize, Debug, Clone, Eq, PartialEq)]
pub struct PollChoiceState {
    pub id: String,
    pub title: String,
    pub votes: i64,
    pub channel_points_votes: i64,
}

impl PollChoiceState {
    /// Twitch's `votes` is the combined total (free + channel-point votes), so subtracting
    /// the channel-point votes yields the free "popular" votes.
    pub fn popular_votes(&self) -> i64 {
        self.votes - self.channel_points_votes
    }
}

#[derive(Deserialize, Serialize, Default, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PollPhase {
    #[default]
    Active,
    Terminated,
    Archived,
    Completed,
}

#[derive(Deserialize, Serialize, Default, Debug, Clone, Eq, PartialEq)]
pub struct PollStateData {
    pub id: String,
    pub choices: Vec<PollChoiceState>,
    pub status: PollPhase,
}

#[derive(Deserialize, Debug)]
pub struct PollStateResponse {
    pub(crate) data: Vec<PollStateData>,
}
