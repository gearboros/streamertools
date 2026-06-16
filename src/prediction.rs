use std::collections::HashMap;
use std::fs;
use iced::{Center, Element, Renderer, Task, Theme};
use iced::widget::{button, pick_list, row, text_input, column, Button, Column, Container, PickList, Text, TextInput, rule};
use iced_aw::number_input;
use rand::prelude::SliceRandom;
use rand::rng;
use serde::{Deserialize, Serialize};
use crate::{prediction, App, AppPhase, Message, SPACING};
use crate::prediction::PredictionMessage::{AddOption, CancelPrediction, ConfigLoaded, EndPrediction, LoadConfig, LockPrediction, NewConfig, PredictionEnded, SaveConfig, Submit, SwitchOptions};
use crate::twitch_api::{cancel_prediction, create_prediction, end_prediction, lock_prediction, CreatePredictionRequest, CreatePredictionResponseData, EndPredictionRequest, PollChoice, PredictionStatus};

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct PredictionState {
    pub(crate) title: String,
    pub(crate) options: Vec<String>,
    pub(crate) duration: usize,
    pub(crate) name: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub(crate) id: Option<String>,
    #[serde(skip_serializing, skip_deserializing)]
    pub(crate) phase: Option<PredictionStatus>,
    #[serde(skip_serializing, skip_deserializing)]
    pub(crate) outcomes: HashMap<String, String>,
    #[serde(skip_serializing, skip_deserializing)]
    pub(crate) current_state: Option<CreatePredictionResponseData>,
}

#[derive(Debug, Clone)]
pub enum PredictionMessage {
    TitleChanged(String),
    OptionChanged(usize, String),
    AddOption,
    RemoveOption(usize),
    Submit,
    PredictionCreated(Result<CreatePredictionResponseData, String>),
    DurationChange(usize),
    EndPrediction,
    WinnerChosen(usize),
    PredictionEnded,
    ConfigSelected(String),
    LoadConfig,
    ConfigLoaded,
    SaveConfig,
    NewConfig,
    NameChanged(String),
    SwitchOptions,
    LockPrediction,
    CancelPrediction,
}

fn get_state_text(state: &PredictionState) -> String {
    if state.phase.is_none() {
        String::from("No Prediction active.")
    } else if state.phase == Some(PredictionStatus::Active) {
        let distribution = get_points_distribution(&state.current_state);
        String::from(format!("Voting active, currently at: {}", distribution))
    } else if state.phase == Some(PredictionStatus::Locked) {
        let distribution = get_points_distribution(&state.current_state);
        String::from(format!("Voting closed, prediction active. Distribution: {}", distribution))
    } else if state.phase == Some(PredictionStatus::Canceled) {
        String::from("Prediction cancelled.")
    } else if state.phase == Some(PredictionStatus::Resolved) {
        String::from("Prediction resolved.")
    } else {
        String::new()
    }
}

fn get_points_distribution(state: &Option<CreatePredictionResponseData>) -> String {
    state.clone().map_or(String::from("No Prediction Active"), |state| {
        let total_points = state.outcomes.iter().map(|o| o.channel_points).sum::<i32>();
        let total_users = state.outcomes.iter().map(|o| o.users).sum::<i32>();
        let list = state.outcomes.iter().fold(String::new(), |acc, o| {
            let user_percent = if total_users == 0 { 0f64 } else { (o.users as f64) / (total_users as f64) * 100.0 };
            let point_percent = if total_points == 0 { 0f64 } else { (o.channel_points as f64) / (total_points as f64) * 100.0 };
            let current = format!("- {}: {:.2}% of points, {:.2}% of users\n", o.title, point_percent, user_percent);
            acc + current.as_str()
        });
        String::from(format!("Total: {} points & {} users\n{}", total_points, total_users, list))
    })
}

impl App {
    pub fn handle_pred(&mut self, pred_message: PredictionMessage) -> Task<Message> {
        use prediction::PredictionMessage::*;
        match pred_message {
            TitleChanged(t) => {
                self.prediction_state.title = t;
                Task::none()
            }
            SaveConfig => {
                let json = serde_json::to_string(&self.prediction_state).unwrap();
                let pred = self.prediction_state.name.clone();
                let path = self.config_path.join("predictions").join(format!("{}.json", pred));
                fs::write(&path, json).unwrap();
                let preds = Self::load_files(self.config_path.join("predictions"));
                self.predictions = preds;
                self.selected_prediction = Some(pred);
                self.prediction_loaded = true;
                Task::none()
            }
            LoadConfig => {
                let selection = &self.selected_prediction;
                if let Some(pred) = selection {
                    let path = &self.config_path.join("predictions").join(format!("{}.json", pred));
                    let config: Option<PredictionState> = fs::read_to_string(path)
                        .ok()
                        .and_then(|t| {
                            let result = serde_json::from_str(&t);
                            result.ok()
                        });
                    if let Some(state) = config {
                        self.prediction_state = state;
                        self.prediction_loaded = true;
                    }
                }
                Task::none()
            }
            NewConfig => {
                self.prediction_loaded = false;
                self.selected_prediction = None;
                Task::none()
            }
            NameChanged(name) => {
                self.prediction_state.name = name;
                Task::none()
            }
            SwitchOptions => {
                if self.prediction_state.options.len() == 2 {
                    self.prediction_state.options.swap(0, 1);
                } else {
                    self.prediction_state.options.shuffle(&mut rng())
                }
                Task::none()
            }
            OptionChanged(idx, val) => {
                if let Some(o) = self.prediction_state.options.get_mut(idx) {
                    *o = val;
                }
                Task::none()
            }
            AddOption => {
                self.prediction_state.options.push(String::new());
                Task::none()
            }
            RemoveOption(idx) => {
                if self.prediction_state.options.len() > 2 {
                    self.prediction_state.options.remove(idx);
                }
                Task::none()
            }
            Submit => {
                let token = self.access_token.clone().unwrap_or_default();
                self.prediction_state.phase = Some(PredictionStatus::Active);
                let request = CreatePredictionRequest {
                    broadcaster_id: self.broadcaster_id.clone().unwrap_or_default(),
                    title: self.prediction_state.title.clone(),
                    outcomes: self.prediction_state.options.clone().iter().map(|o| {
                        PollChoice {
                            title: o.clone()
                        }
                    }).collect(),
                    prediction_window: self.poll_state.duration,
                };
                Task::perform(
                    async move { create_prediction(&token, request).await },
                    |r| Message::Prediction(PredictionCreated(r)),
                )
            }
            PredictionCreated(r) => {
                match r {
                    Ok(data) => {
                        self.phase = AppPhase::PredictionPolling;
                        self.prediction_state.id = Some(data.id);
                        self.prediction_state.outcomes = data.outcomes.iter().map(|o| (o.title.clone(), o.id.clone())).collect();
                        Task::none()
                    }
                    Err(e) => {
                        self.prediction_state.phase = None;
                        Task::done(Message::Error(e.to_string()))
                    }
                }
            }
            DurationChange(d) => {
                self.prediction_state.duration = d;
                Task::none()
            }
            EndPrediction => {
                Task::none()
            }
            WinnerChosen(idx) => {
                self.phase = AppPhase::NoPolling;
                let token = self.access_token.clone().unwrap_or_default();
                let request = EndPredictionRequest {
                    outcome_id: self.prediction_state.outcomes.get(&self.prediction_state.options[idx].clone()).unwrap().to_string(),
                    broadcaster_id: self.broadcaster_id.clone().unwrap_or_default(),
                    prediction_id: self.prediction_state.id.clone().unwrap(),
                };
                Task::perform(
                    async move { end_prediction(request, &token).await },
                    |r| Message::Prediction(PredictionEnded),
                )
            }
            PredictionEnded => {
                self.prediction_state.id = None;
                Task::none()
            }
            ConfigSelected(c) => {
                self.selected_prediction = Some(c.clone());
                Task::none()
            }
            ConfigLoaded => {
                Task::none()
            }
            LockPrediction => {
                let token = self.access_token.clone().unwrap_or_default();
                let request = EndPredictionRequest {
                    outcome_id: String::new(),
                    broadcaster_id: self.broadcaster_id.clone().unwrap_or_default(),
                    prediction_id: self.prediction_state.id.clone().unwrap(),
                };
                Task::future(
                    async move { lock_prediction(request, &token).await }).discard()
            }
            CancelPrediction => {
                let token = self.access_token.clone().unwrap_or_default();
                let request = EndPredictionRequest {
                    outcome_id: String::new(),
                    broadcaster_id: self.broadcaster_id.clone().unwrap_or_default(),
                    prediction_id: self.prediction_state.id.clone().unwrap(),
                };
                Task::future(
                    async move { cancel_prediction(request, &token).await }).discard()
            }
        }
    }

    pub(crate) fn get_prediction_tab_content(&self) -> Element<'static, Message, Theme, Renderer> {
        let dropdown: PickList<'_, String, Vec<String>, String, Message> = pick_list(self.predictions.clone(), self.selected_prediction.clone(), |t| Message::Prediction(PredictionMessage::ConfigSelected(t)));
        let load_btn: Button<_> = button("Load")
            .on_press(Message::Prediction(PredictionMessage::LoadConfig));
        let state = self.prediction_state.clone();
        let mut name_input: TextInput<_> = text_input("Config Name", &state.name);
        if !self.prediction_loaded {
            name_input = name_input.on_input(|n| Message::Prediction(PredictionMessage::NameChanged(n)));
        }
        let new_btn: Button<_> = button("New")
            .on_press(Message::Prediction(PredictionMessage::NewConfig));
        let save_btn: Button<_> = button("Save")
            .on_press(Message::Prediction(PredictionMessage::SaveConfig));
        let save_row = row![dropdown, name_input, new_btn, load_btn, save_btn].spacing(SPACING);

        let title_input = text_input("Prediction title", &state.title)
            .on_input(|r| Message::Prediction(PredictionMessage::TitleChanged(r)));
        let mut opt_col: Column<_> = iced::widget::column![].spacing(SPACING);

        for (idx, option) in state.options.iter().enumerate() {
            let input = text_input(format!("Option {}", idx + 1).as_str(), option)
                .on_input(move |s| Message::Prediction(PredictionMessage::OptionChanged(idx, s)));
            let mut rem_btn = button("-");
            if state.options.len() > 2 && state.phase == None {
                rem_btn = rem_btn.on_press(Message::Prediction(PredictionMessage::RemoveOption(idx)));
            }
            let mut win_btn = button("Winner!");
            if state.id.is_some() && state.phase == Some(PredictionStatus::Locked) {
                win_btn = win_btn.on_press(Message::Prediction(PredictionMessage::WinnerChosen(idx)));
            }
            opt_col = opt_col.push(row![rem_btn, input, win_btn].spacing(SPACING));
        }

        let mut add_btn = button("+");
        let mut switch_btn = button("Switch Options");
        let mut shuffle_btn = button("Shuffle Options");
        if state.phase == None {
            add_btn = add_btn.on_press(Message::Prediction(PredictionMessage::AddOption));
            switch_btn = switch_btn.on_press(Message::Prediction(PredictionMessage::SwitchOptions));
            shuffle_btn = shuffle_btn.on_press(Message::Prediction(PredictionMessage::SwitchOptions));
        }
        let mut option_btn_row = row![add_btn].spacing(SPACING);
        if state.options.len() == 2 {
            option_btn_row = option_btn_row.push(switch_btn)
        } else {
            option_btn_row = option_btn_row.push(shuffle_btn)
        }

        let duration_text = Text::new("Duration in s: ");
        let duration_inp = number_input(&state.duration, 30..=1800, |d| Message::Prediction(PredictionMessage::DurationChange(d)));

        let duration_row = row![duration_text, duration_inp].align_y(Center);

        let mut submit_btn = button("Submit");
        if state.phase == None {
            submit_btn = submit_btn.on_press(Message::Prediction(PredictionMessage::Submit));
        }
        let mut lock_btn = button("Lock");
        if state.phase == Some(PredictionStatus::Active) {
            lock_btn = lock_btn.on_press(Message::Prediction(LockPrediction));
        }
        let mut cancel_btn = button("Cancel");
        if state.phase == Some(PredictionStatus::Active) || state.phase == Some(PredictionStatus::Locked) {
            cancel_btn = cancel_btn.on_press(Message::Prediction(CancelPrediction));
        }
        let btn_row = row![submit_btn, lock_btn, cancel_btn].spacing(SPACING);

        let status_text = get_state_text(&state);
        let status_display = Text::new(status_text);

        Container::new(row![column![save_row, title_input, opt_col, option_btn_row, rule::horizontal(2), duration_row, rule::horizontal(2), btn_row, status_display].spacing(SPACING)])
            .max_width(600).into()
    }
}