use crate::twitch_api::{
    CreatePredictionRequest, CreatePredictionResponseData, EndPredictionRequest, PollChoice,
    PredictionStatus, cancel_prediction, create_prediction, end_prediction, lock_prediction,
};
use crate::{App, AppPhase, Message, SPACING, prediction};
use iced::widget::{
    Button, Column, Container, PickList, Text, TextInput, button, column, pick_list, row, rule,
    text_input,
};
use iced::{Center, Element, Length, Renderer, Task, Theme};
use iced_aw::number_input;
use rand::prelude::SliceRandom;
use rand::rng;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct PredictionState {
    pub(crate) title: String,
    pub(crate) options: Vec<String>,
    pub(crate) duration: usize,
    pub(crate) name: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub(crate) phase: Option<PredictionStatus>,
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
    WinnerChosen(String),
    PredictionEnded,
    ConfigSelected(String),
    SaveConfig,
    NewConfig,
    NameChanged(String),
    SwitchOptions,
    LockPrediction,
    CancelPrediction,
    ResetPrediction,
}

fn get_state_view(state: &PredictionState) -> Element<'static, Message, Theme, Renderer> {
    if state.phase.is_none() {
        Text::new("No Prediction active.").into()
    } else if state.phase == Some(PredictionStatus::Active) {
        column![
            Text::new("Voting active, currently at:"),
            get_points_distribution(&state.current_state)
        ]
        .spacing(SPACING)
        .into()
    } else if state.phase == Some(PredictionStatus::Locked) {
        column![
            Text::new("Voting closed, prediction active."),
            get_points_distribution(&state.current_state)
        ]
        .spacing(SPACING)
        .into()
    } else if state.phase == Some(PredictionStatus::Canceled) {
        Text::new("Prediction cancelled.").into()
    } else if state.phase == Some(PredictionStatus::Resolved) {
        Text::new("Prediction resolved.").into()
    } else {
        Text::new("").into()
    }
}

fn get_points_distribution(
    state: &Option<CreatePredictionResponseData>,
) -> Element<'static, Message, Theme, Renderer> {
    let Some(state) = state.clone() else {
        return Text::new("No Prediction Active").into();
    };
    let total_points = state.outcomes.iter().map(|o| o.channel_points).sum::<i32>();
    let total_users = state.outcomes.iter().map(|o| o.users).sum::<i32>();
    let mut col: Column<_> = column![Text::new(format!(
        "Total: {} points & {} users",
        total_points, total_users
    ))]
    .spacing(SPACING);
    for o in &state.outcomes {
        let user_percent = if total_users == 0 {
            0f64
        } else {
            (o.users as f64) / (total_users as f64) * 100.0
        };
        let point_percent = if total_points == 0 {
            0f64
        } else {
            (o.channel_points as f64) / (total_points as f64) * 100.0
        };
        col = col.push(
            row![
                Text::new(o.title.clone()).width(Length::FillPortion(2)),
                Text::new(format!("{:.2}% of points", point_percent)).width(Length::FillPortion(2)),
                Text::new(format!("{:.2}% of users", user_percent)).width(Length::FillPortion(2)),
            ]
            .spacing(SPACING),
        );
    }
    col.into()
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
                let path = self
                    .config_path
                    .join("predictions")
                    .join(format!("{}.json", pred));
                fs::write(&path, json).unwrap();
                let preds = Self::load_files(self.config_path.join("predictions"));
                self.predictions = preds;
                self.selected_prediction = Some(pred);
                self.prediction_loaded = true;
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
                self.prediction_state.phase = Some(PredictionStatus::Active);
                let token = self.access_token.clone().unwrap_or_default();
                let request = CreatePredictionRequest {
                    broadcaster_id: self.broadcaster_id.clone().unwrap_or_default(),
                    title: self.prediction_state.title.clone(),
                    outcomes: self
                        .prediction_state
                        .options
                        .clone()
                        .iter()
                        .map(|o| PollChoice { title: o.clone() })
                        .collect(),
                    prediction_window: self.prediction_state.duration * 60,
                };
                let client = self.client.clone();
                Task::perform(
                    async move { create_prediction(&client, &token, request).await },
                    |r| Message::Prediction(PredictionCreated(r)),
                )
            }
            PredictionCreated(r) => match r {
                Ok(data) => {
                    self.prediction_state.current_state = Some(data);
                    self.phase = AppPhase::PredictionPolling;
                    Task::none()
                }
                Err(e) => {
                    self.prediction_state.phase = None;
                    Task::done(Message::Error(e.to_string()))
                }
            },
            DurationChange(d) => {
                self.prediction_state.duration = d;
                Task::none()
            }
            WinnerChosen(id) => {
                self.phase = AppPhase::NoPolling;
                let token = self.access_token.clone().unwrap_or_default();
                let request = EndPredictionRequest {
                    outcome_id: id,
                    broadcaster_id: self.broadcaster_id.clone().unwrap_or_default(),
                    prediction_id: self
                        .prediction_state
                        .current_state
                        .clone()
                        .unwrap()
                        .id
                        .clone(),
                };
                let client = self.client.clone();
                Task::perform(
                    async move { end_prediction(&client, request, &token).await },
                    |_r| Message::Prediction(PredictionEnded),
                )
            }
            PredictionEnded => {
                self.prediction_state.current_state = None;
                self.prediction_state.phase = Some(PredictionStatus::Resolved);
                Task::none()
            }
            ConfigSelected(c) => {
                self.selected_prediction = Some(c.clone());
                let selection = &self.selected_prediction;
                if let Some(pred) = selection {
                    let path = &self
                        .config_path
                        .join("predictions")
                        .join(format!("{}.json", pred));
                    let config: Option<PredictionState> =
                        fs::read_to_string(path).ok().and_then(|t| {
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
            LockPrediction => {
                let token = self.access_token.clone().unwrap_or_default();
                let request = self.create_end_prediction_request();
                let client = self.client.clone();
                Task::future(async move { lock_prediction(&client, request, &token).await })
                    .discard()
            }
            CancelPrediction => {
                let token = self.access_token.clone().unwrap_or_default();
                let request = self.create_end_prediction_request();
                let client = self.client.clone();
                Task::future(async move { cancel_prediction(&client, request, &token).await })
                    .discard()
            }
            ResetPrediction => {
                self.prediction_state.phase = None;
                Task::none()
            }
        }
    }

    fn create_end_prediction_request(&mut self) -> EndPredictionRequest {
        let request = EndPredictionRequest {
            outcome_id: String::new(),
            broadcaster_id: self.broadcaster_id.clone().unwrap_or_default(),
            prediction_id: self
                .prediction_state
                .current_state
                .clone()
                .unwrap()
                .id
                .clone(),
        };
        request
    }

    pub(crate) fn get_prediction_tab_content(&self) -> Element<'static, Message, Theme, Renderer> {
        let dropdown: PickList<'_, String, Vec<String>, String, Message> = pick_list(
            self.predictions.clone(),
            self.selected_prediction.clone(),
            |t| Message::Prediction(PredictionMessage::ConfigSelected(t)),
        );
        let state = self.prediction_state.clone();
        let mut name_input: TextInput<_> = text_input("Config Name", &state.name);
        if !self.prediction_loaded {
            name_input =
                name_input.on_input(|n| Message::Prediction(PredictionMessage::NameChanged(n)));
        }
        let new_btn: Button<_> =
            button("New").on_press(Message::Prediction(PredictionMessage::NewConfig));
        let save_btn: Button<_> =
            button("Save").on_press(Message::Prediction(PredictionMessage::SaveConfig));
        let save_row = row![dropdown, name_input, new_btn, save_btn].spacing(SPACING);

        let title_input = text_input("Prediction title", &state.title)
            .on_input(|r| Message::Prediction(PredictionMessage::TitleChanged(r)));
        let mut opt_col: Column<_> = iced::widget::column![].spacing(SPACING);

        if state.phase == None {
            for (idx, option) in state.options.iter().enumerate() {
                let input =
                    text_input(format!("Option {}", idx + 1).as_str(), option).on_input(move |s| {
                        Message::Prediction(PredictionMessage::OptionChanged(idx, s))
                    });
                let mut rem_btn = button("-");
                if state.options.len() > 2 && state.phase == None {
                    rem_btn =
                        rem_btn.on_press(Message::Prediction(PredictionMessage::RemoveOption(idx)));
                }
                opt_col = opt_col.push(row![rem_btn, input].spacing(SPACING));
            }
        } else {
            if state.current_state.is_some() {
                for option in state.current_state.clone().unwrap().outcomes.iter() {
                    let text = Text::new(option.title.clone()).width(Length::Fill);
                    let mut win_btn = button("Winner!");
                    if state.current_state.is_some()
                        && state.phase == Some(PredictionStatus::Locked)
                    {
                        win_btn = win_btn.on_press(Message::Prediction(
                            PredictionMessage::WinnerChosen(option.id.clone()),
                        ));
                    }
                    opt_col = opt_col.push(row![text, win_btn].align_y(Center).spacing(SPACING));
                }
            }
        }

        let mut add_btn = button("+");
        let mut switch_btn = button("Switch Options");
        let mut shuffle_btn = button("Shuffle Options");
        if state.phase == None {
            add_btn = add_btn.on_press(Message::Prediction(PredictionMessage::AddOption));
            switch_btn = switch_btn.on_press(Message::Prediction(PredictionMessage::SwitchOptions));
            shuffle_btn =
                shuffle_btn.on_press(Message::Prediction(PredictionMessage::SwitchOptions));
        }
        let mut option_btn_row = row![add_btn].spacing(SPACING);
        if state.options.len() == 2 {
            option_btn_row = option_btn_row.push(switch_btn)
        } else {
            option_btn_row = option_btn_row.push(shuffle_btn)
        }

        let duration_text = Text::new("Duration in mins: ");
        let mut duration_inp = number_input(&state.duration, 1..=30, |d| {
            Message::Prediction(PredictionMessage::DurationChange(d))
        });
        if state.current_state.is_some() {
            duration_inp = duration_inp.on_input_maybe(None::<fn(usize) -> Message>);
        }

        let duration_row = row![duration_text, duration_inp].align_y(Center);

        let mut submit_btn = button("Submit");
        if state.phase == None {
            submit_btn = submit_btn.on_press(Message::Prediction(PredictionMessage::Submit));
        }
        let mut lock_btn = button("Lock");
        if state.phase == Some(PredictionStatus::Active) {
            lock_btn = lock_btn.on_press(Message::Prediction(PredictionMessage::LockPrediction));
        }
        let mut cancel_btn = button("Cancel");
        if state.phase == Some(PredictionStatus::Active)
            || state.phase == Some(PredictionStatus::Locked)
        {
            cancel_btn =
                cancel_btn.on_press(Message::Prediction(PredictionMessage::CancelPrediction));
        }
        let mut reset_btn = button("Reset");
        if state.phase == Some(PredictionStatus::Resolved)
            || state.phase == Some(PredictionStatus::Canceled)
        {
            reset_btn = reset_btn.on_press(Message::Prediction(PredictionMessage::ResetPrediction));
        }
        let btn_row = row![submit_btn, lock_btn, cancel_btn, reset_btn].spacing(SPACING);

        let status_display = get_state_view(&state);

        Container::new(row![
            column![
                save_row,
                title_input,
                opt_col,
                option_btn_row,
                rule::horizontal(2),
                duration_row,
                rule::horizontal(2),
                btn_row,
                status_display
            ]
            .spacing(SPACING)
        ])
        .max_width(600)
        .into()
    }
}
