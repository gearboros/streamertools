//! Only used if --debug is active for
//! Test date for debug buttons
#![allow(dead_code)]

use crate::twitch_api::{
    CreatePredictionResponseData, PollChoiceState, PollPhase, PollStateData, PredictionOutcome,
    PredictionStatus,
};

fn choice(id: &str, title: &str, votes: i32, channel_point_votes: i32) -> PollChoiceState {
    PollChoiceState {
        id: id.to_string(),
        title: title.to_string(),
        votes,
        channel_point_votes,
    }
}

fn poll(choices: Vec<PollChoiceState>) -> PollStateData {
    PollStateData {
        id: "test-poll".to_string(),
        broadcaster_id: "test-broadcaster".to_string(),
        winning_outcome_id: Some("1".to_string()),
        choices,
        status: PollPhase::Completed,
        started_at: None,
    }
}

pub fn poll_total_winner() -> PollStateData {
    poll(vec![
        choice("1", "Total Winner", 100, 40),
        choice("2", "Runner Up", 50, 10),
        choice("3", "Third", 30, 5),
    ])
}

pub fn poll_points_winner() -> PollStateData {
    poll(vec![
        choice("1", "Overall & Points", 100, 90),
        choice("2", "Popular Vote", 80, 5),
        choice("3", "Third", 20, 10),
    ])
}

pub fn running_poll() -> PollStateData {
    let choices = vec![
        choice("1", "Overall & Points", 100, 90),
        choice("2", "Popular Vote", 80, 5),
        choice("3", "Third", 20, 10),
    ];
    PollStateData {
        id: "test-poll".to_string(),
        broadcaster_id: "test-broadcaster".to_string(),
        winning_outcome_id: None,
        choices,
        status: PollPhase::Active,
        started_at: None,
    }
}

pub fn poll_popular_winner() -> PollStateData {
    poll(vec![
        choice("1", "Overall & Popular", 100, 10),
        choice("2", "Points Winner", 60, 55),
        choice("3", "Third", 30, 8),
    ])
}

fn outcome(
    id: &str,
    title: &str,
    users: i32,
    channel_points: i32,
    color: &str,
) -> PredictionOutcome {
    PredictionOutcome {
        id: id.to_string(),
        title: title.to_string(),
        users,
        channel_points,
        top_predictors: None,
        color: color.to_string(),
    }
}

fn prediction(outcomes: Vec<PredictionOutcome>) -> CreatePredictionResponseData {
    CreatePredictionResponseData {
        id: "test-prediction".to_string(),
        broadcaster_id: "test-broadcaster".to_string(),
        winning_outcome_id: Some("1".to_string()),
        outcomes,
        status: PredictionStatus::Active,
        created_at: None,
        ended_at: None,
        locked_at: None,
    }
}

pub fn prediction_two() -> CreatePredictionResponseData {
    prediction(vec![
        outcome("1", "Yes", 120, 45_000, "BLUE"),
        outcome("2", "No", 80, 30_000, "PINK"),
    ])
}

pub fn prediction_five() -> CreatePredictionResponseData {
    prediction(vec![
        outcome("1", "Outcome 1", 90, 30_000, "BLUE"),
        outcome("2", "Outcome 2", 70, 22_000, "PINK"),
        outcome("3", "Outcome 3", 55, 18_500, "BLUE"),
        outcome("4", "Outcome 4", 40, 12_000, "PINK"),
        outcome("5", "Outcome 5", 25, 6_500, "BLUE"),
    ])
}

pub fn prediction_ten() -> CreatePredictionResponseData {
    let outcomes = (1..=10i32)
        .map(|i| {
            let color = if i % 2 == 1 { "BLUE" } else { "PINK" };
            outcome(
                &i.to_string(),
                &format!("Outcome {i}"),
                110 - i * 10,
                (11 - i) * 4000,
                color,
            )
        })
        .collect();
    prediction(outcomes)
}
