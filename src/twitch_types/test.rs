use super::*;
use std::fs;

fn load_fixture(name: &str) -> String {
    let path = format!("tests/fixtures/{}", name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e))
}

mod prediction {
    use super::*;

    #[test]
    fn parses_prediction_resolved_pending() {
        let json = load_fixture("prediction_resolved_pending.json");
        let resp: CreatePredictionResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp.data.len(), 1);
        let data = &resp.data[0];
        assert_eq!(data.status, PredictionStatus::Resolved);
        assert!(data.winning_outcome_id.is_some());
        assert_eq!(
            data.winning_outcome_id.as_ref().unwrap(),
            "852378b7-ff5c-4b4e-b54d-615459b35574"
        );
        assert_eq!(data.outcomes.len(), 2);
        let winning = data
            .outcomes
            .iter()
            .find(|o| o.id == *data.winning_outcome_id.as_ref().unwrap())
            .unwrap();
        assert!(winning.top_predictors.is_some());
        let predictors = winning.top_predictors.as_ref().unwrap();
        assert!(predictors.iter().all(|p| p.channel_points_won == 0));
    }

    #[test]
    fn parses_prediction_resolved_settled() {
        let json = load_fixture("prediction_resolved_settled.json");
        let resp: CreatePredictionResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp.data.len(), 1);
        let data = &resp.data[0];
        assert_eq!(data.status, PredictionStatus::Resolved);
        let winning = data
            .outcomes
            .iter()
            .find(|o| o.id == *data.winning_outcome_id.as_ref().unwrap())
            .unwrap();
        let predictors = winning.top_predictors.as_ref().unwrap();
        assert!(predictors.iter().any(|p| p.channel_points_won > 0));
    }

    #[test]
    fn parses_prediction_active() {
        let json = load_fixture("prediction_active.json");
        let resp: CreatePredictionResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp.data.len(), 1);
        let data = &resp.data[0];
        assert_eq!(data.status, PredictionStatus::Active);
        assert!(data.winning_outcome_id.is_none());
        assert_eq!(data.outcomes.len(), 2);
        for outcome in &data.outcomes {
            assert!(outcome.top_predictors.is_some());
        }
    }

    #[test]
    fn enum_status_uppercase_rename() {
        let statuses = [
            ("RESOLVED", PredictionStatus::Resolved),
            ("ACTIVE", PredictionStatus::Active),
            ("LOCKED", PredictionStatus::Locked),
            ("CANCELED", PredictionStatus::Canceled),
        ];
        for (json_str, expected) in statuses {
            let json = format!("\"{}\"", json_str);
            let parsed: PredictionStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, expected);
        }
    }

    #[test]
    fn unrecognized_status_falls_back_to_unknown() {
        let parsed: PredictionStatus = serde_json::from_str("\"SOMETHING_NEW\"").unwrap();
        assert_eq!(parsed, PredictionStatus::Unknown);
    }
}

mod poll {
    use super::*;
    #[test]
    fn parses_poll_completed() {
        let json = load_fixture("poll_completed.json");
        let resp: PollStateResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp.data.len(), 1);
        let data = &resp.data[0];
        assert_eq!(data.status, PollPhase::Completed);
        assert_eq!(data.choices.len(), 4);
        let winning = data.choices.iter().max_by_key(|c| c.votes).unwrap();
        assert_eq!(winning.title, "7PM");
        assert_eq!(winning.votes, 12);
        assert_eq!(winning.channel_points_votes, 5);
        assert_eq!(winning.popular_votes(), 7);
    }

    #[test]
    fn enum_poll_phase_uppercase_rename() {
        let phases = [
            ("ACTIVE", PollPhase::Active),
            ("TERMINATED", PollPhase::Terminated),
            ("ARCHIVED", PollPhase::Archived),
            ("COMPLETED", PollPhase::Completed),
            ("MODERATED", PollPhase::Moderated),
            ("INVALID", PollPhase::Invalid),
        ];
        for (json_str, expected) in phases {
            let json = format!("\"{}\"", json_str);
            let parsed: PollPhase = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, expected);
        }
    }

    #[test]
    fn unrecognized_phase_falls_back_to_unknown() {
        let parsed: PollPhase = serde_json::from_str("\"SOMETHING_NEW\"").unwrap();
        assert_eq!(parsed, PollPhase::Unknown);
    }
}

mod payouts_pending {
    use super::*;
    use crate::prediction::payouts_pending;

    #[test]
    fn pending_fixture_is_pending() {
        let json = load_fixture("prediction_resolved_pending.json");
        let resp: CreatePredictionResponse = serde_json::from_str(&json).unwrap();
        assert!(payouts_pending(&resp.data[0]));
    }

    #[test]
    fn settled_fixture_is_not_pending() {
        let json = load_fixture("prediction_resolved_settled.json");
        let resp: CreatePredictionResponse = serde_json::from_str(&json).unwrap();
        assert!(!payouts_pending(&resp.data[0]));
    }

    #[test]
    fn active_fixture_is_not_pending() {
        let json = load_fixture("prediction_active.json");
        let resp: CreatePredictionResponse = serde_json::from_str(&json).unwrap();
        assert!(!payouts_pending(&resp.data[0]));
    }
}
