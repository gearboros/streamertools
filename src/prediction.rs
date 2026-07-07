use crate::base_form::{handle_base_changes, BaseFormMessage, EditableForm};
use crate::chart::{BarChart, BarData};
use crate::config::{handle_config, ConfigForm, ConfigList, ConfigMessage, Named};
use crate::sample_data::{prediction_five, prediction_ongoing, prediction_ten, prediction_two};
use crate::style::{bold_text, get_base_color, thousand_separator};
use crate::twitch_api::{cancel_prediction, create_prediction, end_prediction, lock_prediction};
use crate::twitch_types::{
    CreatePredictionRequest, CreatePredictionResponseData, EndPredictionRequest, PollChoice,
    PredictionOutcome, PredictionStatus,
};
use crate::widgets::{config_bar, duration_row, option_editor};
use crate::{prediction, style, App, Message, BIG_SPACING, SPACING};
use iced::widget::{button, canvas, column, container, row, rule, text, text_input, Column, Text};
use iced::{Center, Element, Length, Renderer, Task, Theme};
use rand::prelude::SliceRandom;
use rand::rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::error;

#[derive(Default, Debug)]
pub struct PredictionTab {
    pub form: PredictionState,
    pub run: PredictionRun,
    pub configs: ConfigList,
    pub active_tab: usize,
}

impl ConfigForm for PredictionTab {
    type Form = PredictionState;
    const SUBDIR: &'static str = "predictions";

    fn form(&self) -> &Self::Form {
        &self.form
    }

    fn form_mut(&mut self) -> &mut Self::Form {
        &mut self.form
    }

    fn configs_mut(&mut self) -> &mut ConfigList {
        &mut self.configs
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub enum PredictionRun {
    #[default]
    Idle,
    Live(CreatePredictionResponseData),
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct PredictionState {
    pub title: String,
    pub options: Vec<String>,
    pub duration: usize,
    pub name: String,
}

impl Named for PredictionState {
    fn name(&self) -> &str {
        &self.name
    }

    fn set_name(&mut self, name: String) {
        self.name = name;
    }
}

impl EditableForm for PredictionState {
    const MAX_OPTIONS: usize = 10;

    fn options_mut(&mut self) -> &mut Vec<String> {
        &mut self.options
    }

    fn set_duration(&mut self, d: usize) {
        self.duration = d;
    }

    fn set_title(&mut self, title: String) {
        self.title = title;
    }
}

#[derive(Debug, Clone)]
pub enum PredictionMessage {
    BaseFormChange(BaseFormMessage),
    Submit,
    PredictionCreated(Result<CreatePredictionResponseData, String>),
    WinnerChosen(String),
    PredictionEnded(Result<CreatePredictionResponseData, String>),
    PredictionLocked(Result<CreatePredictionResponseData, String>),
    PredictionCanceled(Result<CreatePredictionResponseData, String>),
    Config(ConfigMessage),
    SwitchOptions,
    LockPrediction,
    CancelPrediction,
    ResetPrediction,
    LoadSampleData(CreatePredictionResponseData),
    TabSelected(usize),
}

impl App {
    pub fn handle_pred(&mut self, pred_message: PredictionMessage) -> Task<Message> {
        use prediction::PredictionMessage::*;
        match pred_message {
            SwitchOptions => {
                if self.prediction.form.options.len() == 2 {
                    self.prediction.form.options.swap(0, 1);
                } else {
                    self.prediction.form.options.shuffle(&mut rng())
                }
                Task::none()
            }
            Submit => {
                let (token, broadcaster_id) = match self.require_token_and_broadcaster_id() {
                    Ok(v) => v,
                    Err(e) => return Self::log_and_show_error(&e),
                };
                let request = CreatePredictionRequest {
                    broadcaster_id,
                    title: self.prediction.form.title.clone(),
                    outcomes: self
                        .prediction
                        .form
                        .options
                        .iter()
                        .map(|o| PollChoice { title: o.clone() })
                        .collect(),
                    // minutes -> seconds for api
                    prediction_window: self.prediction.form.duration * 60,
                };
                let client = self.client.clone();
                Task::perform(
                    async move { create_prediction(&client, &token, request).await },
                    |r| Message::Prediction(PredictionCreated(r)),
                )
            }
            PredictionCreated(r) => self.set_prediction_phase(r),
            WinnerChosen(id) => {
                let (token, broadcaster_id) = match self.require_token_and_broadcaster_id() {
                    Ok(v) => v,
                    Err(e) => return Self::log_and_show_error(&e),
                };
                let PredictionRun::Live(d) = &self.prediction.run else {
                    return Self::log_and_show_error(
                        "No active prediction when trying to choose winner.",
                    );
                };
                let prediction_id = d.id.clone();
                let request = EndPredictionRequest {
                    outcome_id: id,
                    broadcaster_id,
                    prediction_id,
                };
                let client = self.client.clone();
                Task::perform(
                    async move { end_prediction(&client, request, &token).await },
                    |r| Message::Prediction(PredictionEnded(r)),
                )
            }
            PredictionEnded(r) => self.set_prediction_phase(r),
            LockPrediction => {
                let (token, broadcaster_id) = match self.require_token_and_broadcaster_id() {
                    Ok(v) => v,
                    Err(e) => return Self::log_and_show_error(&e),
                };
                match self.create_prediction_state_change_request(broadcaster_id) {
                    Ok(request) => {
                        let client = self.client.clone();
                        Task::perform(
                            async move { lock_prediction(&client, request, &token).await },
                            |r| Message::Prediction(PredictionLocked(r)),
                        )
                    }
                    Err(e) => Task::done(Message::Error(e)),
                }
            }
            PredictionLocked(r) => self.set_prediction_phase(r),
            CancelPrediction => {
                let (token, broadcaster_id) = match self.require_token_and_broadcaster_id() {
                    Ok(v) => v,
                    Err(e) => return Self::log_and_show_error(&e),
                };
                match self.create_prediction_state_change_request(broadcaster_id) {
                    Ok(request) => {
                        let client = self.client.clone();
                        Task::perform(
                            async move { cancel_prediction(&client, request, &token).await },
                            |r| Message::Prediction(PredictionCanceled(r)),
                        )
                    }
                    Err(e) => Task::done(Message::Error(e)),
                }
            }
            PredictionCanceled(r) => self.set_prediction_phase(r),
            ResetPrediction => {
                self.prediction.run = PredictionRun::Idle;
                Task::none()
            }
            LoadSampleData(data) => {
                self.prediction.run = PredictionRun::Live(data);
                Task::none()
            }
            TabSelected(idx) => {
                self.prediction.active_tab = idx;
                Task::none()
            }
            Config(c) => handle_config(&self.config_path, c, &mut self.prediction),
            BaseFormChange(b) => handle_base_changes(&mut self.prediction.form, b),
        }
    }

    fn set_prediction_phase(
        &mut self,
        result: Result<CreatePredictionResponseData, String>,
    ) -> Task<Message> {
        match result {
            Ok(r) => {
                self.prediction.run = PredictionRun::Live(r);
                Task::none()
            }
            Err(e) => Task::done(Message::Error(e)),
        }
    }

    fn create_prediction_state_change_request(
        &self,
        broadcaster_id: String,
    ) -> Result<EndPredictionRequest, String> {
        if let PredictionRun::Live(d) = &self.prediction.run {
            let prediction_id = d.id.clone();
            // Empty `outcome_id` = no winner; used for lock/cancel. `set_prediction_state`
            // omits `winning_outcome_id` from the request when this is empty.
            Ok(EndPredictionRequest {
                outcome_id: String::new(),
                broadcaster_id,
                prediction_id,
            })
        } else {
            Err("Prediction run should be live".to_string())
        }
    }

    pub fn get_prediction_tab_content(&self) -> Element<'_, Message, Theme, Renderer> {
        let state = &self.prediction.form;
        let editable = self.prediction.run == PredictionRun::Idle;
        let phase = if let PredictionRun::Live(d) = &self.prediction.run {
            Some(d.status.clone())
        } else {
            None
        };

        let config_row = config_bar(
            &self.prediction.configs,
            &state.name,
            |t| Message::Prediction(PredictionMessage::Config(ConfigMessage::ConfigSelected(t))),
            |n| Message::Prediction(PredictionMessage::Config(ConfigMessage::NameChanged(n))),
            Message::Prediction(PredictionMessage::Config(ConfigMessage::New)),
            Message::Prediction(PredictionMessage::Config(ConfigMessage::Save)),
        );

        let title_input = text_input("Prediction title", &state.title).on_input(|r| {
            Message::Prediction(PredictionMessage::BaseFormChange(
                BaseFormMessage::TitleChanged(r),
            ))
        });
        let mut opt_col: Column<_> = iced::widget::column![].spacing(SPACING);

        if editable {
            opt_col = option_editor(
                &state.options,
                editable,
                |i, s| {
                    Message::Prediction(PredictionMessage::BaseFormChange(
                        BaseFormMessage::OptionChanged(i, s),
                    ))
                },
                |idx| {
                    Message::Prediction(PredictionMessage::BaseFormChange(
                        BaseFormMessage::RemoveOption(idx),
                    ))
                },
            );
        } else {
            // list of options with win buttons
            let options: &[PredictionOutcome] = if let PredictionRun::Live(d) = &self.prediction.run
            {
                &d.outcomes
            } else {
                &[]
            };
            for option in options.iter() {
                let text = Text::new(option.title.clone()).width(Length::Fill);
                let mut win_btn = button("Winner!");
                if phase == Some(PredictionStatus::Locked) {
                    win_btn = win_btn.on_press(Message::Prediction(
                        PredictionMessage::WinnerChosen(option.id.clone()),
                    ));
                }
                opt_col = opt_col.push(row![text, win_btn].align_y(Center).spacing(SPACING));
            }
        }

        let mut add_btn = button(text("+").center()).width(30);
        let mut switch_btn = button("Switch Options");
        let mut shuffle_btn = button("Shuffle Options");
        if editable {
            if state.options.len() < PredictionState::MAX_OPTIONS {
                add_btn = add_btn.on_press(Message::Prediction(PredictionMessage::BaseFormChange(
                    BaseFormMessage::AddOption,
                )));
            }
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

        let duration_row = duration_row(editable, &state.duration, |d| {
            Message::Prediction(PredictionMessage::BaseFormChange(
                BaseFormMessage::DurationChanged(d),
            ))
        });

        let mut submit_btn = button("Submit");
        if editable {
            submit_btn = submit_btn.on_press(Message::Prediction(PredictionMessage::Submit));
        }
        let mut lock_btn = button("Lock");
        if phase == Some(PredictionStatus::Active) {
            lock_btn = lock_btn.on_press(Message::Prediction(PredictionMessage::LockPrediction));
        }
        let mut cancel_btn = button("Cancel").style(style::red_button);
        if phase == Some(PredictionStatus::Active) || phase == Some(PredictionStatus::Locked) {
            cancel_btn =
                cancel_btn.on_press(Message::Prediction(PredictionMessage::CancelPrediction));
        }
        let mut reset_btn = button("Reset").style(style::neutral_button);
        if phase == Some(PredictionStatus::Resolved) || phase == Some(PredictionStatus::Canceled) {
            reset_btn = reset_btn.on_press(Message::Prediction(PredictionMessage::ResetPrediction));
        }
        let btn_row = row![submit_btn, lock_btn, cancel_btn, reset_btn].spacing(SPACING);
        let mut dbg_row = column![];

        if self.sample {
            // only shown with --sample, buttons to show sample results for testing
            let two_option_sample =
                button("Two Options")
                    .style(style::dbg_button)
                    .on_press(Message::Prediction(PredictionMessage::LoadSampleData(
                        prediction_two(),
                    )));
            let five_option_sample =
                button("Five Options")
                    .style(style::dbg_button)
                    .on_press(Message::Prediction(PredictionMessage::LoadSampleData(
                        prediction_five(),
                    )));
            let ten_option_sample =
                button("Ten Options")
                    .style(style::dbg_button)
                    .on_press(Message::Prediction(PredictionMessage::LoadSampleData(
                        prediction_ten(),
                    )));
            let ongoing_sample =
                button("Ongoing")
                    .style(style::dbg_button)
                    .on_press(Message::Prediction(PredictionMessage::LoadSampleData(
                        prediction_ongoing(),
                    )));
            dbg_row = column![
                rule::horizontal(2),
                row![
                    two_option_sample,
                    five_option_sample,
                    ten_option_sample,
                    ongoing_sample
                ]
                .spacing(SPACING)
            ];
        }

        let status_display = get_state_view(&self.prediction.run, self.prediction.active_tab);

        let form = column![
            config_row,
            rule::horizontal(2),
            title_input,
            opt_col,
            option_btn_row,
            duration_row,
            rule::horizontal(2),
            btn_row,
            dbg_row,
        ]
        .spacing(SPACING);

        let results = container(status_display)
            .padding(SPACING as u16 * 2)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(container::rounded_box);

        crate::widgets::split_pane(form, results)
    }
}

fn get_state_view(
    run: &PredictionRun,
    active_tab: usize,
) -> Element<'static, Message, Theme, Renderer> {
    if let PredictionRun::Live(current) = run {
        match current.status {
            PredictionStatus::Resolved => {
                let winner = match get_winner(current) {
                    Ok(w) => w,
                    Err(e) => {
                        error!("{}", e);
                        return column![
                            Text::new(format!("Could not determine winner: {e}")),
                            get_points_distribution(&Some(current), active_tab)
                        ]
                        .spacing(SPACING)
                        .into();
                    }
                };
                let total_points = current
                    .outcomes
                    .iter()
                    .map(|o| o.channel_points)
                    .sum::<i64>();
                let ratio = if winner.channel_points == 0 {
                    0f64
                } else {
                    total_points as f64 / winner.channel_points as f64
                };
                column![
                    row![
                        Text::new("Prediction resolved, Winner: "),
                        bold_text(winner.title.clone()),
                        Text::new(format!(" ({ratio:.2}x)")),
                    ],
                    get_points_distribution(&Some(current), active_tab)
                ]
                .spacing(SPACING)
                .into()
            }
            PredictionStatus::Active => column![
                Text::new("Voting active, currently at:"),
                get_points_distribution(&Some(current), active_tab)
            ]
            .spacing(SPACING)
            .into(),
            PredictionStatus::Locked => column![
                Text::new("Voting closed, prediction active."),
                get_points_distribution(&Some(current), active_tab)
            ]
            .spacing(SPACING)
            .into(),
            PredictionStatus::Canceled => Text::new("Prediction cancelled.").into(),
        }
    } else {
        crate::widgets::empty_panel("🎲", "No prediction running yet")
    }
}

fn get_winner(current: &CreatePredictionResponseData) -> Result<&PredictionOutcome, String> {
    let winner_id = current
        .winning_outcome_id
        .as_ref()
        .ok_or_else(|| "Resolved prediction has no winning outcome.".to_string())?;
    current
        .outcomes
        .iter()
        .find(|x| &x.id == winner_id)
        .ok_or_else(|| format!("Winning outcome {winner_id} not found in prediction outcomes."))
}

fn get_points_distribution(
    state: &Option<&CreatePredictionResponseData>,
    active_tab: usize,
) -> Element<'static, Message, Theme, Renderer> {
    let Some(state) = *state else {
        return Text::new("No Prediction Active").into();
    };
    let resolved = state.status == PredictionStatus::Resolved;
    let total_points = state.outcomes.iter().map(|o| o.channel_points).sum::<i64>();
    let total_users = state.outcomes.iter().map(|o| o.users).sum::<i64>();

    let mut by_points = state.outcomes.clone();
    by_points.sort_by_key(|o| std::cmp::Reverse(o.channel_points));

    let mut title_col: Column<_> = column![bold_text("".to_string())].spacing(SPACING);
    let mut point_col: Column<_> = column![bold_text("Points".to_string())].spacing(SPACING);
    let mut user_col: Column<_> = column![bold_text("Users".to_string())].spacing(SPACING);

    // we have tab_bar at home -> enables custom colors on the tabs.
    let mut tab_bar = row![];
    let mut tab_content = HashMap::new();

    for (idx, o) in by_points.into_iter().enumerate() {
        let (user_percent, point_percent) = get_percentages(total_points, total_users, &o);
        title_col = title_col.push(text(format!("• {}", o.title.clone())));
        point_col = point_col.push(text(format!(
            "{} points, {:.2}%",
            thousand_separator(o.channel_points),
            point_percent
        )));
        user_col = user_col.push(text(format!("{} users, {:.2}%", o.users, user_percent)));

        let is_active = active_tab == idx;
        let btn = button(text(o.title.clone()))
            .style(move |_, status| style::prediction_button(&o.color.clone(), status, is_active))
            .padding(SPACING as u16)
            .on_press(Message::Prediction(PredictionMessage::TabSelected(idx)));
        tab_bar = tab_bar.push(btn);
        let lines = o
            .top_predictors
            .unwrap_or_default()
            .into_iter()
            .map(|d| {
                let mut line = format!(
                    "• {}: {} points",
                    d.user_name,
                    thousand_separator(d.channel_points_used)
                );
                if resolved {
                    if d.channel_points_won > 0 {
                        line.push_str(&format!(
                            ", won {}",
                            thousand_separator(d.channel_points_won)
                        ));
                    } else {
                        line.push_str(", lost");
                    }
                }
                line
            })
            .collect::<Vec<_>>();
        tab_content.insert(idx, lines);
    }

    let grid = row![title_col, point_col, user_col].spacing(BIG_SPACING);

    let selected = if tab_content.contains_key(&active_tab) {
        active_tab
    } else {
        0
    };

    let mut content_col: Column<_> = column![].spacing(SPACING);
    match tab_content.get(&selected) {
        Some(lines) if !lines.is_empty() => {
            for line in lines {
                content_col = content_col.push(text(line.clone()));
            }
        }
        _ => {
            content_col = content_col.push(text("No predictors yet"));
        }
    }

    let bar_chart = canvas(BarChart {
        data: state
            .outcomes
            .clone()
            .iter()
            .map(|c| BarData {
                color: get_base_color(&c.color),
                title: c.title.clone(),
                value: c.channel_points,
            })
            .collect(),
    })
    .width(Length::Fill)
    .height(Length::Fill);

    container(
        column![
            Text::new(format!(
                "Total: {} points & {} users",
                thousand_separator(total_points),
                total_users
            )),
            grid,
            tab_bar,
            content_col,
            bar_chart,
        ]
        .spacing(SPACING),
    )
    .into()
}

fn get_percentages(total_points: i64, total_users: i64, o: &PredictionOutcome) -> (f64, f64) {
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
    (user_percent, point_percent)
}
