//! Only used if --sample is active for
//! Test data for debug buttons
#![allow(dead_code)]

use crate::twitch_types::{
    CreatePredictionResponseData, PollChoiceState, PollPhase, PollStateData, PredictionOutcome,
    PredictionStatus, Predictor,
};

fn predictor(user_name: &str, channel_points_used: i32, channel_points_won: i32) -> Predictor {
    Predictor {
        user_name: user_name.to_string(),
        channel_points_used,
        channel_points_won,
    }
}

fn choice(id: &str, title: &str, votes: i32, channel_points_votes: i32) -> PollChoiceState {
    PollChoiceState {
        id: id.to_string(),
        title: title.to_string(),
        votes,
        channel_points_votes,
    }
}

fn poll(choices: Vec<PollChoiceState>) -> PollStateData {
    PollStateData {
        id: "test-poll".to_string(),
        choices,
        status: PollPhase::Completed,
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
        choices,
        status: PollPhase::Active,
    }
}

pub fn poll_popular_winner() -> PollStateData {
    poll(vec![
        choice("1", "Overall & Popular", 100, 10),
        choice("2", "Points Winner", 60, 55),
        choice("3", "Third", 30, 8),
    ])
}

pub fn poll_tie() -> PollStateData {
    poll(vec![
        choice("1", "Team Cats", 100, 30),
        choice("2", "Team Dogs", 100, 20),
        choice("3", "Team Rats", 40, 10),
        choice("4", "Team Spiders", 4, 2),
    ])
}

fn outcome(
    id: &str,
    title: &str,
    users: i32,
    channel_points: i32,
    color: &str,
    factor: i32,
) -> PredictionOutcome {
    // make sure to have one winner display and the rest losers.
    let won = if id == "1" { 1 } else { 0 };
    PredictionOutcome {
        id: id.to_string(),
        title: title.to_string(),
        users,
        channel_points,
        top_predictors: Some(vec![
            predictor("Alice", 12_000 * factor, 18_000 * factor * won),
            predictor("Bob", 8_500 * factor, 12_750 * factor * won),
            predictor("Carol", 3_000 * factor, 4_500 * factor * won),
        ]),
        color: color.to_string(),
    }
}

fn prediction(outcomes: Vec<PredictionOutcome>) -> CreatePredictionResponseData {
    CreatePredictionResponseData {
        id: "test-prediction".to_string(),
        winning_outcome_id: Some("1".to_string()),
        outcomes,
        status: PredictionStatus::Resolved,
    }
}

pub fn prediction_two() -> CreatePredictionResponseData {
    prediction(vec![
        outcome("1", "Yes", 120, 45_000, "BLUE", 2),
        outcome("2", "No", 80, 30_000, "PINK", 3),
    ])
}

pub fn prediction_ongoing() -> CreatePredictionResponseData {
    let outcomes = vec![
        outcome("1", "Yes", 120, 45_000, "BLUE", 2),
        outcome("2", "No", 80, 30_000, "PINK", 5),
    ];
    CreatePredictionResponseData {
        id: "test-prediction".to_string(),
        winning_outcome_id: Some("1".to_string()),
        outcomes,
        status: PredictionStatus::Active,
    }
}

pub fn prediction_five() -> CreatePredictionResponseData {
    prediction(vec![
        outcome("1", "Outcome 1", 90, 30_000, "BLUE", 1),
        outcome("2", "Outcome 2", 70, 22_000, "PINK", 2),
        outcome("3", "Outcome 3", 55, 18_500, "BLUE", 3),
        outcome("4", "Outcome 4", 40, 12_000, "PINK", 4),
        outcome("5", "Outcome 5", 25, 6_500, "BLUE", 5),
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
                i,
            )
        })
        .collect();
    prediction(outcomes)
}
