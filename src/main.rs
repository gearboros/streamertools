mod auth;
mod poll;
mod prediction;
mod sample_data;
mod style;
mod twitch_api;
mod twitch_auth;
mod widgets;

use crate::style::{twitch_button, twitch_tab};
use crate::twitch_auth::*;
use auth::AuthMessage;
use directories::ProjectDirs;
use iced::alignment::Vertical;
use iced::widget::operation::{focus_next, focus_previous};
use iced::widget::space::horizontal;
use iced::widget::{button, column, container, row, text, Container, Text};
use iced::{keyboard, time, Element, Renderer, Subscription, Task, Theme};
use iced_aw::{TabBar, TabLabel};
use poll::{PollMessage, PollState};
use prediction::{PredictionMessage, PredictionState};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{error, info};
use tracing_appender::rolling::Rotation;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use twitch_api::*;

pub const CLIENT_ID: &str = "9w729lqufngx4sztgex20eztz7o879";

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
            TabId::Misc => 2,
            TabId::Poll => 1,
            TabId::Prediction => 0,
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

#[derive(Default, Debug, Eq, PartialEq)]
enum AppPolling {
    #[default]
    Not,
    Prediction,
    Poll,
}

#[derive(Default, Debug)]
struct App {
    client: reqwest::Client,
    broadcaster_id: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
    _loading: bool,
    auth_status: String,
    // Device code flow UI state
    device_code_info: Option<DeviceCodeInfo>,
    auth_in_progress: bool,
    polling: AppPolling,
    active_tab: TabId,
    err: String,
    confirm: Option<String>,
    poll_state: PollState,
    polls: Vec<String>,
    selected_poll: Option<String>,
    poll_loaded: bool,
    prediction_state: PredictionState,
    predictions: Vec<String>,
    selected_prediction: Option<String>,
    prediction_loaded: bool,
    config_path: PathBuf,
    debug: bool,
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
enum Message {
    Auth(AuthMessage),
    TabSelected(usize),
    Poll(PollMessage),
    Prediction(PredictionMessage),
    Error(String),
    ClearError,
    PredictionTick,
    PredictionPolled(Result<CreatePredictionResponseData, String>),
    PollTick,
    PollPolled(Result<PollStateData, String>),
    Keyboard(keyboard::Event),
}

const SPACING: u32 = 10;
const BIG_SPACING: u32 = 30;

impl App {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Auth(auth_message) => self.handle_auth(auth_message),
            Message::TabSelected(idx) => {
                self.active_tab = TabId::from_idx(idx);
                Task::none()
            }
            Message::Prediction(pred_message) => self.handle_pred(pred_message),
            Message::Poll(poll_message) => self.handle_poll(poll_message),
            Message::Error(err) => {
                self.err = err;
                Task::none()
            }
            Message::ClearError => {
                self.err = String::new();
                Task::none()
            }
            Message::PredictionTick => {
                if self.polling == AppPolling::Prediction {
                    let broadcaster_id = self.broadcaster_id.clone().unwrap();
                    let pred_id = self.prediction_state.current_state.clone().unwrap().id;
                    let token = self.access_token.clone().unwrap();
                    let client = self.client.clone();
                    Task::perform(
                        async move {
                            check_prediction(&client, &broadcaster_id, &pred_id, &token).await
                        },
                        Message::PredictionPolled,
                    )
                } else {
                    Task::none()
                }
            }
            Message::PredictionPolled(resp) => {
                match resp {
                    Ok(r) => {
                        self.prediction_state.phase = Some(r.status.clone());
                        if r.status == PredictionStatus::Canceled
                            || r.status == PredictionStatus::Resolved
                        {
                            self.polling = AppPolling::Not;
                        }
                        self.prediction_state.current_state = Some(r);
                    }
                    Err(err) => {
                        error!("{:?}", err);
                        self.polling = AppPolling::Not;
                    }
                }
                Task::none()
            }
            Message::PollTick => {
                if self.polling == AppPolling::Poll {
                    let broadcaster_id = self.broadcaster_id.clone().unwrap();
                    let poll_id = self.poll_state.current_state.clone().unwrap().id.clone();
                    let token = self.access_token.clone().unwrap();
                    let client = self.client.clone();
                    Task::perform(
                        async move { check_poll(&client, &broadcaster_id, &poll_id, &token).await },
                        Message::PollPolled,
                    )
                } else {
                    Task::none()
                }
            }
            Message::PollPolled(resp) => {
                match resp {
                    Ok(r) => {
                        self.poll_state.phase = Some(r.status.clone());
                        if r.status == PollPhase::Archived || r.status == PollPhase::Completed {
                            self.polling = AppPolling::Not;
                        }
                        self.poll_state.current_state = Some(r);
                    }
                    Err(err) => {
                        error!("{:?}", err);
                        self.polling = AppPolling::Not;
                    }
                }
                Task::none()
            }
            Message::Keyboard(event) => match event {
                keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Tab),
                    modifiers,
                    ..
                } => {
                    if modifiers.shift() {
                        focus_previous()
                    } else {
                        focus_next()
                    }
                }
                keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(c),
                    modifiers,
                    ..
                } => {
                    if modifiers.control() {
                        let tab = match c.as_str() {
                            "1" => Some(TabId::Prediction),
                            "2" => Some(TabId::Poll),
                            "3" => Some(TabId::Misc),
                            _ => None,
                        };
                        if let Some(tab) = tab {
                            return self.update(Message::TabSelected(tab.idx()));
                        }
                    }
                    Task::none()
                }
                _ => Task::none(),
            },
        }
    }

    fn view(&'_ self) -> Element<'_, Message> {
        let mut tab_bar = TabBar::new(Message::TabSelected)
            .push(
                TabId::Prediction.idx(),
                TabLabel::Text("Prediction 🎲".into()),
            )
            .push(TabId::Poll.idx(), TabLabel::Text("Poll 📊".into()))
            .push(TabId::Misc.idx(), TabLabel::Text("Misc".into()))
            .set_active_tab(&self.active_tab.idx());

        tab_bar = tab_bar.style(twitch_tab);

        let auth_btn = if self.auth_in_progress {
            button("Authenticating...")
        } else if self.access_token.is_some() {
            button("Re-authenticate").on_press(Message::Auth(AuthMessage::StartAuth))
        } else {
            button("Login with Twitch").on_press(Message::Auth(AuthMessage::StartAuth))
        }
        .style(twitch_button);

        let mut content = column![].spacing(SPACING);
        let auth_text = self.auth_status.clone().trim().to_string();
        let auth = row![auth_btn, text(auth_text)]
            .align_y(Vertical::Center)
            .spacing(SPACING);
        content = content.push(auth);

        // Show device code info if available
        if let Some(info) = &self.device_code_info {
            content = content.push(
                column![
                    text(format!("Visit: {}", info.verification_uri)),
                    row![text(format!("Enter code: {}", info.user_code)).size(20),]
                        .spacing(SPACING),
                ]
                .spacing(SPACING),
            );
        }

        content = content.push(tab_bar);
        content = content.push(self.get_tab_content());

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

            widgets::modal(content.padding(20), error, Message::ClearError)
        } else if self.confirm.is_some() {
            let confirm = container(
                column![
                    text("Confirm").size(24),
                    column![
                        text(self.confirm.clone().unwrap().clone()),
                        row![
                            horizontal(),
                            button(text("No"))
                                .on_press(Message::Auth(AuthMessage::FallbackConfirmed(false))),
                            button(text("Yes"))
                                .on_press(Message::Auth(AuthMessage::FallbackConfirmed(true))),
                        ]
                        .spacing(SPACING)
                    ]
                    .spacing(10)
                ]
                .spacing(20),
            )
            .width(600)
            .padding(10)
            .style(container::rounded_box);

            widgets::modal(
                content.padding(20),
                confirm,
                Message::Auth(AuthMessage::FallbackConfirmed(false)),
            )
        } else {
            container(content.padding(20)).into()
        }
    }

    fn get_tab_content(&self) -> Element<'static, Message, Theme, Renderer> {
        match &self.active_tab {
            TabId::Prediction => self.get_prediction_tab_content(),
            TabId::Poll => self.get_poll_tab_content(),
            TabId::Misc => Container::new(row![Text::new("Your ad could be here!")]).into(),
        }
    }
}

fn subscription(app: &App) -> Subscription<Message> {
    let keys = keyboard::listen().map(Message::Keyboard);

    let polling = match app.polling {
        AppPolling::Prediction => {
            time::every(Duration::from_secs(1)).map(|_| Message::PredictionTick)
        }
        AppPolling::Poll => time::every(Duration::from_secs(1)).map(|_| Message::PollTick),
        AppPolling::Not => Subscription::none(),
    };
    Subscription::batch([keys, polling])
}

fn main() -> iced::Result {
    let debug = std::env::args().any(|x| x == "--debug");

    // create all config dirs
    let proj = ProjectDirs::from("dev", "gearboros", "streamertools").unwrap();
    let config_path = proj.config_dir().to_path_buf();
    fs::create_dir_all(config_path.clone()).expect("Could not create config directory");
    fs::create_dir_all(config_path.join("polls")).expect("Could not create polls directory");
    fs::create_dir_all(config_path.join("predictions"))
        .expect("Could not create predictions directory");
    fs::create_dir_all(config_path.join("logs")).expect("Could not create log dir.");

    let file_appender = tracing_appender::rolling::Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix("streamertools")
        .filename_suffix("log")
        .max_log_files(10)
        .build(config_path.join("logs"))
        .expect("Could not initialize rolling file appender");
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);

    // Honor RUST_LOG if set, otherwise default to info.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::stdout))
        .with(fmt::layer().with_ansi(false).with_writer(file_writer))
        .init();

    info!("Starting Streamer Tools");

    iced::application(
        move || App::new(&config_path, debug),
        App::update,
        App::view,
    )
    .title("Streamer Tools")
    .font(iced_aw::ICED_AW_FONT_BYTES)
    .subscription(subscription)
    .run()
}

impl App {
    fn new(path: &Path, debug: bool) -> (Self, Task<Message>) {
        let polls = Self::load_files(path.join("polls"));
        let preds = Self::load_files(path.join("predictions"));

        let poll_state = PollState {
            options: vec![String::new(), String::new()],
            duration: 10,
            channel_point_cost: 5000,
            ..Default::default()
        };
        let prediction_state = PredictionState {
            options: vec![String::new(), String::new()],
            duration: 10,
            ..Default::default()
        };
        let config_path = path.to_path_buf();
        if let Some((access, refresh)) = load_tokens(&config_path) {
            info!("Loaded tokens, validating...");
            let app = Self {
                access_token: Some(access.clone()),
                refresh_token: Some(refresh.clone()),
                auth_status: "Checking saved token...".to_string(),
                poll_state,
                prediction_state,
                config_path,
                polls,
                predictions: preds,
                debug,
                ..Default::default()
            };
            let task = Task::done(Message::Auth(AuthMessage::ValidateToken));
            return (app, task);
        }
        info!("No tokens found in keyring");
        (
            Self {
                auth_status: "Not logged in".to_string(),
                poll_state,
                config_path,
                polls,
                prediction_state,
                predictions: preds,
                debug,
                ..Default::default()
            },
            Task::none(),
        )
    }

    fn load_files(path: PathBuf) -> Vec<String> {
        fs::read_dir(path)
            .unwrap()
            .map(|r| {
                r.unwrap()
                    .path()
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string()
            })
            .collect::<Vec<String>>()
    }
}

fn save_config<T: Serialize>(
    root: &Path,
    subdir: &str,
    name: &str,
    state: &T,
) -> Result<(), String> {
    let json = serde_json::to_string(state).map_err(|e| e.to_string())?;
    fs::write(root.join(subdir).join(format!("{name}.json")), json).unwrap();
    Ok(())
}

fn load_config<T: DeserializeOwned>(root: &Path, subdir: &str, name: &str) -> Option<T> {
    fs::read_to_string(root.join(subdir).join(format!("{name}.json")))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
}
