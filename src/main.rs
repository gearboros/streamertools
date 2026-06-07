mod twitch;

use std::fs;
use std::path::PathBuf;
use iced::widget::{button, column, text, row, container, Text, Row, text_input, Column, checkbox, Checkbox, rule, Container, stack, opaque, mouse_area, center, Button};
use iced::{Center, Color, Element, Length, Renderer, Task, Theme};
use iced::alignment::Vertical;
use iced::widget::text::State;
use crate::twitch::*;
use tracing::info;
use iced_aw::{number_input, TabBar, TabLabel};
use serde::{Deserialize, Serialize};
use directories::ProjectDirs;
use iced::advanced::svg::Data::Path;

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

#[derive(Default, Debug, Serialize, Deserialize)]
struct PollState {
    title: String,
    options: Vec<String>,
    duration: usize,
    uses_channel_points: bool,
    channel_point_cost: usize,
    id: String,
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
    active_tab: TabId,
    tabs: Vec<(String, String)>,
    err: String,
    poll_state: PollState,
    config_path: std::path::PathBuf,
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
    SaveConfig,
}

#[derive(Debug, Clone)]
enum Message {
    Auth(AuthMessage),
    TabSelected(usize),
    Poll(PollMessage),
    TabClosed(usize),
    Error(String),
    ClearError,
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
                let path = self.config_path.join("polls").join("test.json");
                fs::write(&path, json).unwrap();
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

    fn view(&'_ self) -> Element<Message> {
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
            TabId::Prediction => Container::new(row![Text::new("Dashboard content")]),
            TabId::Poll => {
                let save_btn: Button<_> = button("Save")
                    .on_press(Message::Poll(PollMessage::SaveConfig));

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
                Container::new(row![column![save_btn, title_input, opt_col, add_btn, rule::horizontal(2), duration_row, rule::horizontal(2), channel_row, rule::horizontal(2), btns].spacing(SPACING)])
                    .max_width(600)
            }
            TabId::Misc => Container::new(row![Text::new("Profile content")]),
        }
    }
}

fn main() -> iced::Result {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting Streamer Tools");

    iced::application(App::new, App::update, App::view)
        .title("Streamer Tools")
        .run()
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let proj = ProjectDirs::from("dev", "gearboros", "streamertools").unwrap();
        let config_path = proj.config_dir().to_path_buf();
        fs::create_dir_all(config_path.clone()).unwrap();
        fs::create_dir_all(config_path.join("polls")).unwrap();
        fs::create_dir_all(config_path.join("predictions")).unwrap();

        let poll_state = PollState {
            options: vec![String::new(), String::new()],
            duration: 300,
            channel_point_cost: 5000,
            ..Default::default()
        };
        if let Some((access, refresh)) = load_tokens(&config_path) {
            info!("Loaded tokens from keyring, validating...");
            let app = Self {
                access_token: Some(access.clone()),
                refresh_token: Some(refresh.clone()),
                poll_state,
                auth_status: "Checking saved token...".to_string(),
                config_path,
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
            ..Default::default()
        }, Task::none())
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