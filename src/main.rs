mod twitch;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use iced::widget::{button, column, text, row, container, Text, text_input, Column, checkbox, Checkbox, rule, Container, stack, opaque, mouse_area, center, Button, pick_list, PickList, TextInput};
use iced::{time, Center, Color, Element, Length, Renderer, Subscription, Task, Theme};
use iced::alignment::Vertical;
use crate::twitch::*;
use tracing::info;
use iced_aw::{number_input, TabBar, TabLabel};
use serde::{Deserialize, Serialize};
use directories::ProjectDirs;
use rand::prelude::*;
use rand::rng;
use crate::PredictionMessage::{CancelPrediction, LockPrediction, PredictionEnded};
use crate::twitch::PredictionStatus::{Active, Canceled, Locked, Resolved};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum TabId {
    Misc,
    Poll,
    #[default]
    Prediction,
}

impl TabId {
    pub fn idx(self) -> usize {
        match self {
            TabId::Misc => { 2}
            TabId::Poll => { 1 }
            TabId::Prediction => { 0 }
        }
    }

    pub fn from_idx(idx: usize) -> Self {
        match idx {
            0 => TabId::Prediction,
            1 => TabId::Poll,
            2 => TabId::Misc,
            _ => TabId::Prediction,
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
struct PredictionState {
    title: String,
    options: Vec<String>,
    duration: usize,
    name: String,
    #[serde(skip_serializing, skip_deserializing)]
    id: Option<String>,
    #[serde(skip_serializing, skip_deserializing)]
    phase: Option<PredictionStatus>,
    #[serde(skip_serializing, skip_deserializing)]
    outcomes: HashMap<String, String>,
    #[serde(skip_serializing, skip_deserializing)]
    current_state: Option<CreatePredictionResponseData>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(default)]
struct PollState {
    title: String,
    options: Vec<String>,
    duration: usize,
    uses_channel_points: bool,
    channel_point_cost: usize,
    #[serde(skip_serializing, skip_deserializing)]
    id: String,
    name: String
}

#[derive(Default, Debug, Eq, PartialEq)]
enum AppPhase {
    #[default]
    NoPolling,
    PredictionPolling,
    PollPolling,
}

#[derive(Default, Debug)]
struct App {
    broadcaster_id: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
    result: String,
    loading: bool,
    auth_status: String,
    // Device code flow UI state
    device_code_info: Option<DeviceCodeInfo>,
    auth_in_progress: bool,
    phase: AppPhase,
    active_tab: TabId,
    tabs: Vec<(String, String)>,
    err: String,
    poll_state: PollState,
    polls: Vec<String>,
    selected_poll: Option<String>,
    poll_loaded: bool,
    prediction_state: PredictionState,
    predictions: Vec<String>,
    selected_prediction: Option<String>,
    prediction_loaded: bool,
    config_path: PathBuf,
}

#[derive(Debug, Clone)]
struct DeviceCodeInfo {
    verification_uri: String,
    user_code: String,
    device_code: String,
    interval: u64,
    expires_in: u64,
}

#[derive(Debug, Clone)]
enum AuthMessage {
    StartAuth,
    DeviceCodeReceived(Result<DeviceCodeInfo, String>),
    PollForTokens { device_code: String, interval: u64, expires_in: u64 },
    AuthCompleted(Result<(String, String), String>),
    ValidateToken,
    TokenValidated(Option<String>),
    RefreshToken,
}

#[derive(Debug, Clone)]
enum PollMessage {
    TitleChanged(String),
    OptionChanged(usize, String),
    AddOption,
    RemoveOption(usize),
    Submit,
    PollCreated(Result<String, String>),
    DurationChange(usize),
    ChannelPointsToggled(bool),
    PointCostChange(usize),
    EndPoll,
    PollEnded,
    ConfigSelected(String),
    LoadConfig,
    ConfigLoaded,
    SaveConfig,
    NewConfig,
    NameChanged(String),
    SwitchOptions,
}

#[derive(Debug, Clone)]
enum PredictionMessage {
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

#[derive(Debug, Clone)]
enum Message {
    Auth(AuthMessage),
    TabSelected(usize),
    Poll(PollMessage),
    Prediction(PredictionMessage),
    TabClosed(usize),
    Error(String),
    ClearError,
    PredictionTick,
    PredictionPolled(Result<CreatePredictionResponseData, String>),
}

const SPACING: u32 = 10;

impl App {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Auth(auth_message) => {
                self.handle_auth(auth_message)
            }
            Message::TabSelected(idx) => {
                self.active_tab = TabId::from_idx(idx);
                Task::none()
            }
            Message::TabClosed(_idx) => {
                Task::none()
            }
            Message::Prediction(pred_message) => {
                self.handle_pred(pred_message)
            }
            Message::Poll(poll_message) => {
                self.handle_poll(poll_message)
            }
            Message::Error(err) => {
                self.err = err;
                Task::none()
            }
            Message::ClearError => {
                self.err = String::new();
                Task::none()
            }
            Message::PredictionTick => {
                if self.phase == AppPhase::PredictionPolling {
                    let broadcaster_id = self.broadcaster_id.clone().unwrap();
                    let pred_id = self.prediction_state.id.clone().unwrap();
                    let token = self.access_token.clone().unwrap();
                    Task::perform(async move {
                        check_prediction(&broadcaster_id, &pred_id, &token).await },
                                  |r| Message::PredictionPolled(r))
                } else {
                    Task::none()
                }
            }
            Message::PredictionPolled(resp) => {
                match resp {
                    Ok(r) => {
                        self.prediction_state.phase = Some(r.status.clone());
                        if r.status == PredictionStatus::Canceled || r.status == PredictionStatus::Resolved {
                            self.phase = AppPhase::NoPolling;
                        }
                        self.prediction_state.current_state = Some(r);
                    }
                    Err(err) => {
                        eprintln!("{:?}", err);
                        self.phase = AppPhase::NoPolling;
                    }
                }
                Task::none()
            }
        }
    }

    fn handle_pred(&mut self, pred_message: PredictionMessage) -> Task<Message> {
        use PredictionMessage::*;
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
                let preds = Self::load_predictions(self.config_path.join("predictions"));
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

    fn handle_poll(&mut self, poll_message: PollMessage) -> Task<Message> {
        use PollMessage::*;
        match poll_message {
            TitleChanged(t) => {
                self.poll_state.title = t;
                Task::none()
            }
            Submit => {
                let token = self.access_token.clone().unwrap_or_default();
                let request = CreatePollRequest {
                    broadcaster_id: self.broadcaster_id.clone().unwrap_or_default(),
                    title: self.poll_state.title.clone(),
                    choices: self.poll_state.options.clone().iter().map(|o| {
                        PollChoice {
                            title: o.clone()
                        }
                    }).collect(),
                    duration: self.poll_state.duration,
                    channel_points_voting_enabled: self.poll_state.uses_channel_points,
                    channel_points_per_vote: self.poll_state.channel_point_cost,
                };
                Task::perform(
                    async move { create_poll(&token, request).await },
                    |r| Message::Poll(PollCreated(r)),
                )

            }
            OptionChanged(idx, val) => {
                if let Some(o) = self.poll_state.options.get_mut(idx) {
                    *o = val;
                }
                Task::none()
            }
            AddOption => {
                self.poll_state.options.push(String::new());
                Task::none()
            }
            RemoveOption(idx) => {
                if self.poll_state.options.len() > 2 {
                    self.poll_state.options.remove(idx);
                }
                Task::none()
            }
            PollCreated(r) => {
                match r {
                    Ok(id) => {
                        self.poll_state.id = id;
                        println!("Poll created successfully");
                        Task::none()
                    }
                    Err(e) => {
                        eprintln!("Poll creation failed: {}", e);
                        Task::done(Message::Error(e.to_string()))
                    }
                }
            }
            DurationChange(d) => {
                self.poll_state.duration = d;
                Task::none()
            }
            ChannelPointsToggled(t) => {
                self.poll_state.uses_channel_points = t;
                Task::none()
            }
            PointCostChange(c) => {
                self.poll_state.channel_point_cost = c;
                Task::none()
            }
            EndPoll => {
                let token = self.access_token.clone().unwrap_or_default();
                let broadcaster = self.broadcaster_id.clone().unwrap_or_default();
                let poll_id = self.poll_state.id.clone();
                Task::perform(
                    async move { end_poll(&broadcaster, &poll_id, &token).await },
                    |_| Message::Poll(PollEnded)
                )
            }
            PollEnded => {
                self.poll_state.id = String::new();
                Task::none()
            }
            SaveConfig => {
                let json = serde_json::to_string(&self.poll_state).unwrap();
                let poll = self.poll_state.name.clone();
                let path = self.config_path.join("polls").join(format!("{}.json", poll));
                fs::write(&path, json).unwrap();
                let polls = Self::load_polls(self.config_path.join("polls"));
                self.polls = polls;
                self.selected_poll = Some(poll);
                self.poll_loaded = true;
                Task::none()
            }
            ConfigSelected(c) => {
                self.selected_poll = Some(c.clone());
                Task::none()
            }
            LoadConfig => {
                let selection = &self.selected_poll;
                if let Some(poll) = selection {
                    let path = &self.config_path.join("polls").join(format!("{}.json", poll));
                    let config: Option<PollState> = fs::read_to_string(path)
                        .ok()
                        .and_then(|t| {
                            let result = serde_json::from_str(&t);
                            result.ok()
                        });
                    if let Some(state) = config {
                        self.poll_state = state;
                        self.poll_loaded = true;
                    }
                }
                Task::none()
            }
            ConfigLoaded => {
                Task::none()
            }
            NewConfig => {
                self.poll_loaded = false;
                self.selected_poll = None;
                Task::none()
            }
            NameChanged(name) => {
                self.poll_state.name = name;
                Task::none()
            }
            SwitchOptions => {
                if self.poll_state.options.len() == 2 {
                    self.poll_state.options.swap(0, 1);
                } else {
                    self.poll_state.options.shuffle(&mut rng())
                }
                Task::none()
            }
        }
    }

    fn handle_auth(&mut self, auth_message: AuthMessage) -> Task<Message> {
        use AuthMessage::*;
        match auth_message {
            StartAuth => {
                self.auth_status = "Requesting device code...".to_string();
                self.auth_in_progress = true;
                self.device_code_info = None;

                Task::perform(
                    async {
                        match request_device_code().await {
                            Ok(resp) => {
                                // Open browser to verification URL
                                let _ = open::that(&resp.verification_uri);
                                Ok(DeviceCodeInfo {
                                    verification_uri: resp.verification_uri,
                                    user_code: resp.user_code,
                                    device_code: resp.device_code,
                                    interval: resp.interval,
                                    expires_in: resp.expires_in,
                                })
                            }
                            Err(e) => Err(e),
                        }
                    },
                    |result| Message::Auth(DeviceCodeReceived(result)),
                )
            }
            DeviceCodeReceived(result) => {
                match result {
                    Ok(info) => {
                        self.auth_status = format!(
                            "Go to {} and enter code: {}",
                            info.verification_uri, info.user_code
                        );
                        let device_code = info.device_code.clone();
                        let interval = info.interval;
                        let expires_in = info.expires_in;
                        self.device_code_info = Some(info);

                        // Start polling for tokens
                        Task::done(Message::Auth(PollForTokens { device_code, interval, expires_in }))
                    }
                    Err(e) => {
                        self.auth_status = format!("Error: {}", e);
                        self.auth_in_progress = false;
                        Task::none()
                    }
                }
            }
            PollForTokens { device_code, interval, expires_in } => {
                Task::perform(
                    async move {
                        poll_for_tokens(&device_code, interval, expires_in).await
                    },
                    |result| Message::Auth(AuthCompleted(result)),
                )
            }
            AuthCompleted(res) => {
                self.auth_in_progress = false;
                self.device_code_info = None;
                match res {
                    Ok((access_token, refresh_token)) => {
                        let _ = save_tokens(&access_token, &refresh_token, &self.config_path);
                        self.access_token = Some(access_token);
                        self.refresh_token = Some(refresh_token);
                        self.auth_status = "Authenticated".to_string();
                    }
                    Err(e) => {
                        self.auth_status = format!("Error: {}", e);
                    }
                }
                Task::none()
            }
            ValidateToken => {
                if let Some(token) = &self.access_token {
                    let t = token.clone();
                    Task::perform(async move { validate_token(&t).await }, |result| Message::Auth(TokenValidated(result)))
                } else {
                    Task::none()
                }
            }
            TokenValidated(valid) => {
                info!("Token validation result: {:?}", valid);
                if valid.is_some() {
                    self.auth_status = "Authenticated".to_string();
                    self.broadcaster_id = valid;
                    Task::none()
                } else {
                    self.auth_status = "Token Expired, refreshing...".to_string();
                    if self.refresh_token.is_some() {
                        info!("Refreshing token...");
                        Task::done(Message::Auth(RefreshToken))
                    } else {
                        info!("No refresh token, starting auth...");
                        Task::done(Message::Auth(StartAuth))
                    }
                }
            }
            RefreshToken => {
                if let Some(refresh) = &self.refresh_token {
                    let t = refresh.clone();
                    Task::perform(async move { refresh_access_token(&t).await }, |result| Message::Auth(AuthCompleted(result)))
                } else {
                    Task::none()
                }
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let tab_bar = TabBar::new(Message::TabSelected)
            .push(TabId::Prediction.idx(), TabLabel::Text("Prediction".into()))
            .push(TabId::Poll.idx(), TabLabel::Text("Poll".into()))
            .push(TabId::Misc.idx(), TabLabel::Text("Misc".into()))
            .set_active_tab(&self.active_tab.idx());

        let auth_btn = if self.auth_in_progress {
            button("Authenticating...")
        } else if self.access_token.is_some() {
            button("Re-authenticate").on_press(Message::Auth(AuthMessage::StartAuth))
        } else {
            button("Login with Twitch").on_press(Message::Auth(AuthMessage::StartAuth))
        };

        let mut content = column![].spacing(SPACING);
        let auth = row![auth_btn, text(&self.auth_status)].align_y(Vertical::Center).spacing(SPACING);
        content = content.push(auth);

        // Show device code info if available
        if let Some(info) = &self.device_code_info {
            content = content.push(
                column![
                    text(format!("Visit: {}", info.verification_uri)),
                    row![
                        text(format!("Enter code: {}", info.user_code)).size(20),
                    ].spacing(SPACING),
                ].spacing(SPACING)
            );
        }

        content = content.push(tab_bar);
        content = content.push(self.get_tab_content().into());

        if !self.err.is_empty() {
            let error = container(
                column![
                    text("Error").size(24),
                    column![
                        text(self.err.clone()),
                        button(text("Close")).on_press(Message::ClearError),
                    ]
                    .spacing(10)
                ]
                    .spacing(20),
            )
            .width(600)
            .padding(10)
            .style(container::rounded_box);

            modal(content.padding(20), error, Message::ClearError)
        } else {
            container(content.padding(20))
                .into()
        }
    }

    fn get_tab_content(&self) -> impl Into<Element<'static, Message, Theme, Renderer>> {
        match &self.active_tab {
            TabId::Prediction => {
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
                let mut opt_col: Column<_> = column![].spacing(SPACING);
                
                for (idx, option) in state.options.iter().enumerate() {
                    let input = text_input(format!("Option {}", idx + 1).as_str(), option)
                        .on_input(move |s| Message::Prediction(PredictionMessage::OptionChanged(idx, s)));
                    let mut rem_btn = button("-");
                    if state.options.len() > 2 && state.phase == None {
                        rem_btn = rem_btn.on_press(Message::Prediction(PredictionMessage::RemoveOption(idx)));
                    }
                    let mut win_btn = button("Winner!");
                    if state.id.is_some() && state.phase == Some(Locked) {
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
                if state.phase == Some(Active) {
                    lock_btn = lock_btn.on_press(Message::Prediction(LockPrediction));
                }
                let mut cancel_btn = button("Cancel");
                if state.phase == Some(Active) || state.phase == Some(Locked) {
                    cancel_btn = cancel_btn.on_press(Message::Prediction(CancelPrediction));
                }
                let btn_row = row![submit_btn, lock_btn, cancel_btn].spacing(SPACING);

                let status_text = get_state_text(&state);
                let status_display = Text::new(status_text);

                Container::new(row![column![save_row, title_input, opt_col, option_btn_row, rule::horizontal(2), duration_row, rule::horizontal(2), btn_row, status_display].spacing(SPACING)])
                    .max_width(600)
            },
            TabId::Poll => {
                let dropdown: PickList<'_, String, Vec<String>, String, Message> = pick_list(self.polls.clone(), self.selected_poll.clone(), |t| Message::Poll(PollMessage::ConfigSelected(t)));
                let load_btn: Button<_> = button("Load")
                    .on_press(Message::Poll(PollMessage::LoadConfig));
                let mut name_input: TextInput<_> = text_input("Config Name", &self.poll_state.name);
                if !self.poll_loaded {
                   name_input = name_input.on_input(|n| Message::Poll(PollMessage::NameChanged(n)));
                }
                let new_btn: Button<_> = button("New")
                    .on_press(Message::Poll(PollMessage::NewConfig));
                let save_btn: Button<_> = button("Save")
                    .on_press(Message::Poll(PollMessage::SaveConfig));

                let save_row = row![dropdown, name_input, new_btn, load_btn, save_btn].spacing(SPACING);

                let title_input = text_input("Poll title", &self.poll_state.title)
                    .on_input(|r| Message::Poll(PollMessage::TitleChanged(r)));
                let mut opt_col: Column<_> = column![].spacing(SPACING);
                for (idx, option) in self.poll_state.options.iter().enumerate() {
                    let input = text_input(format!("Option {}", idx + 1).as_str(), option)
                        .on_input(move |s| Message::Poll(PollMessage::OptionChanged(idx, s)));
                    let mut rem_btn = button("-");
                    if self.poll_state.options.len() > 2 {
                        rem_btn = rem_btn.on_press(Message::Poll(PollMessage::RemoveOption(idx)));
                    }
                    opt_col = opt_col.push(row![rem_btn, input].spacing(SPACING));
                }

                let add_btn = button("+").on_press(Message::Poll(PollMessage::AddOption));
                let switch_btn = button("Switch Options").on_press(Message::Poll(PollMessage::SwitchOptions));
                let shuffle_btn = button("Shuffle Options").on_press(Message::Poll(PollMessage::SwitchOptions));
                let mut option_btn_row = row![add_btn].spacing(SPACING);
                if self.poll_state.options.len() == 2 {
                    option_btn_row = option_btn_row.push(switch_btn)
                } else {
                    option_btn_row = option_btn_row.push(shuffle_btn)
                }

                let duration_text = Text::new("Duration in s: ");
                let duration_inp = number_input(&self.poll_state.duration, 0..=600, |d| Message::Poll(PollMessage::DurationChange(d)));

                let duration_row = row![duration_text, duration_inp].align_y(Center);

                let channel_point_check: Checkbox<_> = checkbox(self.poll_state.uses_channel_points).label("Enable Channel point votes.")
                    .on_toggle(|t| Message::Poll(PollMessage::ChannelPointsToggled(t)));
                let channel_point_text: Text<_> = Text::new("Channel point cost: ");
                let channel_point_input = number_input(&self.poll_state.channel_point_cost, 0..=1_000_000, |c| Message::Poll(PollMessage::PointCostChange(c)));
                let mut channel_row = column![channel_point_check];
                if self.poll_state.uses_channel_points {
                    channel_row = channel_row.push(row![channel_point_text, channel_point_input].align_y(Center))
                }

                let submit_btn = button("Submit").on_press(Message::Poll(PollMessage::Submit));
                let end_btn = button("End Poll").on_press(Message::Poll(PollMessage::EndPoll));
                let mut btns = row![submit_btn].spacing(SPACING);
                if !&self.poll_state.id.is_empty() {
                    btns = btns.push(end_btn)
                }
                Container::new(row![column![save_row, title_input, opt_col, option_btn_row, rule::horizontal(2), duration_row, rule::horizontal(2), channel_row, rule::horizontal(2), btns].spacing(SPACING)])
                    .max_width(600)
            }
            TabId::Misc => Container::new(row![Text::new("Profile content")]),
        }
    }
}

fn get_state_text(state: &PredictionState) -> String {
    if state.phase.is_none() {
        String::from("No Prediction active.")
    } else if state.phase == Some(Active) {
        let distribution = get_points_distribution(&state.current_state);
        String::from(format!("Voting active, currently at: {}", distribution))
    } else if state.phase == Some(Locked) {
        let distribution = get_points_distribution(&state.current_state);
        String::from(format!("Voting closed, prediction active. Distribution: {}", distribution))
    } else if state.phase == Some(Canceled) {
        String::from("Prediction cancelled.")
    } else if state.phase == Some(Resolved) {
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

fn subscription(app: &App) -> Subscription<Message> {
    match app.phase {
        AppPhase::PredictionPolling => time::every(Duration::from_secs(1)).map(|_| Message::PredictionTick),
        AppPhase::PollPolling => Subscription::none(),
        AppPhase::NoPolling => Subscription::none(),
    }
}

fn main() -> iced::Result {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting Streamer Tools");

    iced::application(App::new, App::update, App::view)
        .title("Streamer Tools")
        .subscription(subscription)
        .run()
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let proj = ProjectDirs::from("dev", "gearboros", "streamertools").unwrap();
        let config_path = proj.config_dir().to_path_buf();
        fs::create_dir_all(config_path.clone()).unwrap();
        fs::create_dir_all(config_path.join("polls")).unwrap();
        fs::create_dir_all(config_path.join("predictions")).unwrap();

        let polls = Self::load_polls(proj.config_dir().join("polls"));
        let preds = Self::load_predictions(proj.config_dir().join("predictions"));

        let poll_state = PollState {
            options: vec![String::new(), String::new()],
            duration: 300,
            channel_point_cost: 5000,
            ..Default::default()
        };
        let prediction_state = PredictionState {
            options: vec![String::new(), String::new()],
            duration: 600,
            id: None,
            ..Default::default()
        };
        if let Some((access, refresh)) = load_tokens(&config_path) {
            info!("Loaded tokens from keyring, validating...");
            let app = Self {
                access_token: Some(access.clone()),
                refresh_token: Some(refresh.clone()),
                auth_status: "Checking saved token...".to_string(),
                poll_state,
                prediction_state,
                config_path,
                polls,
                predictions: preds,
                ..Default::default()
            };
            info!("App state: {:?}", app.access_token);
            info!("App state: {:?}", app.refresh_token);
            let task = Task::perform(
                async move { validate_token(&access).await },
                |result| Message::Auth(AuthMessage::TokenValidated(result)),
            );
            return (app, task);
        }
        info!("No tokens found in keyring");
        (Self {
            auth_status: "Not logged in".to_string(),
            poll_state,
            config_path,
            polls,
            prediction_state,
            predictions: preds,
            ..Default::default()
        }, Task::none())
    }

    fn load_polls(path: PathBuf) -> Vec<String> {
        fs::read_dir(path).unwrap().map(|r| {
            r.unwrap().path().file_stem().unwrap().to_str().unwrap().to_string()
        }).collect::<Vec<String>>()
    }

    fn load_predictions(path: PathBuf) -> Vec<String> {
        fs::read_dir(path).unwrap().map(|r| {
            r.unwrap().path().file_stem().unwrap().to_str().unwrap().to_string()
        }).collect::<Vec<String>>()
    }
}

fn modal<'a>(
    base: impl Into<Element<'a, Message>>,
    content: Container<'a, Message, Theme, Renderer>,
    on_blur: Message,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    stack![
        base.into(),
        button("")
            .style(|_, _| button::Style {
                background: Some(Color { a: 0.8, ..Color::BLACK }.into()),
                ..button::Style::default()
            })
            .on_press(on_blur)
            .width(Length::Fill)
            .height(Length::Fill),
        center(content),
    ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}