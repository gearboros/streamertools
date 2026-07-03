use crate::config::ConfigList;
use crate::sample_data::{prediction_five, prediction_ongoing, prediction_ten, prediction_two};
use crate::style::{bold_text, thousand_separator};
use crate::twitch_api::{
    cancel_prediction, create_prediction, end_prediction, lock_prediction,
    CreatePredictionRequest, CreatePredictionResponseData, EndPredictionRequest, PollChoice, PredictionOutcome,
    PredictionStatus,
};
use crate::{load_config, prediction, save_config, style, App, Message, BIG_SPACING, SPACING};
use iced::widget::{
    button, column, container, pick_list, row, rule, text, text_input, tooltip, Button, Column,
    PickList, Text, TextInput,
};
use iced::{Center, Element, Length, Renderer, Task, Theme};
use iced_aw::number_input;
use rand::prelude::SliceRandom;
use rand::rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Default, Debug)]
pub struct PredictionTab {
    pub form: PredictionState,
    pub run: PredictionRun,
    pub configs: ConfigList,
    pub active_tab: usize,
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
    PredictionEnded(Result<CreatePredictionResponseData, String>),
    PredictionLocked(Result<CreatePredictionResponseData, String>),
    PredictionCanceled(Result<CreatePredictionResponseData, String>),
    ConfigSelected(String),
    SaveConfig,
    NewConfig,
    NameChanged(String),
    SwitchOptions,
    LockPrediction,
    CancelPrediction,
    ResetPrediction,
    LoadSampleData(CreatePredictionResponseData),
    TabSelected(usize),
}

fn get_state_view(
    run: &PredictionRun,
    active_tab: usize,
) -> Element<'static, Message, Theme, Renderer> {
    if let PredictionRun::Live(current) = run {
        match current.status {
            PredictionStatus::Resolved => {
                let winner = get_winner(current);
                let total_points = current
                    .outcomes
                    .iter()
                    .map(|o| o.channel_points)
                    .sum::<i32>();
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

fn get_winner(current: &CreatePredictionResponseData) -> &PredictionOutcome {
    let winner_id = current
        .winning_outcome_id
        .clone()
        .expect("Should have a winner here");
    current
        .outcomes
        .iter()
        .find(|x| x.id == winner_id)
        .expect("Should have a winner here")
}

fn get_points_distribution(
    state: &Option<&CreatePredictionResponseData>,
    active_tab: usize,
) -> Element<'static, Message, Theme, Renderer> {
    let Some(state) = state.clone() else {
        return Text::new("No Prediction Active").into();
    };
    let resolved = state.status == PredictionStatus::Resolved;
    let total_points = state.outcomes.iter().map(|o| o.channel_points).sum::<i32>();
    let total_users = state.outcomes.iter().map(|o| o.users).sum::<i32>();

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
                    "• {} — {} points",
                    d.user_name,
                    thousand_separator(d.channel_points_used)
                );
                if resolved && d.channel_points_won > 0 {
                    line.push_str(&format!(
                        ", won {}",
                        thousand_separator(d.channel_points_won)
                    ));
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
        ]
        .spacing(SPACING),
    )
    .into()
}

fn get_percentages(total_points: i32, total_users: i32, o: &PredictionOutcome) -> (f64, f64) {
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

impl App {
    pub fn handle_pred(&mut self, pred_message: PredictionMessage) -> Task<Message> {
        use prediction::PredictionMessage::*;
        match pred_message {
            TitleChanged(t) => {
                self.prediction.form.title = t;
                Task::none()
            }
            SaveConfig => {
                if let Err(e) = save_config(
                    &self.config_path,
                    "predictions",
                    &self.prediction.form.name,
                    &self.prediction.form,
                ) {
                    return Task::done(Message::Error(e.to_string()));
                };
                self.prediction.configs.items =
                    match Self::load_files(self.config_path.join("predictions")) {
                        Ok(predictions) => predictions,
                        Err(e) => return Task::done(Message::Error(e)),
                    };
                self.prediction.configs.selected = Some(self.prediction.form.name.clone());
                self.prediction.configs.loaded = true;
                Task::none()
            }
            NewConfig => {
                self.prediction.form.name = String::new();
                self.prediction.configs.loaded = false;
                self.prediction.configs.selected = None;
                Task::none()
            }
            NameChanged(name) => {
                self.prediction.form.name = name;
                Task::none()
            }
            SwitchOptions => {
                if self.prediction.form.options.len() == 2 {
                    self.prediction.form.options.swap(0, 1);
                } else {
                    self.prediction.form.options.shuffle(&mut rng())
                }
                Task::none()
            }
            OptionChanged(idx, val) => {
                if let Some(o) = self.prediction.form.options.get_mut(idx) {
                    *o = val;
                }
                Task::none()
            }
            AddOption => {
                self.prediction.form.options.push(String::new());
                Task::none()
            }
            RemoveOption(idx) => {
                if self.prediction.form.options.len() > 2 {
                    self.prediction.form.options.remove(idx);
                }
                Task::none()
            }
            Submit => {
                let access_token = self.access_token.clone();
                if let Some(token) = access_token {
                    let request = CreatePredictionRequest {
                        broadcaster_id: self.broadcaster_id.clone().unwrap_or_default(),
                        title: self.prediction.form.title.clone(),
                        outcomes: self
                            .prediction
                            .form
                            .options
                            .iter()
                            .map(|o| PollChoice { title: o.clone() })
                            .collect(),
                        prediction_window: self.prediction.form.duration * 60,
                    };
                    let client = self.client.clone();
                    Task::perform(
                        async move { create_prediction(&client, &token, request).await },
                        |r| Message::Prediction(PredictionCreated(r)),
                    )
                } else {
                    Task::done(Message::Error(
                        "Missing Auth token! Try to re-authenticate".to_string(),
                    ))
                }
            }
            PredictionCreated(r) => self.set_prediction_phase(r),
            DurationChange(d) => {
                self.prediction.form.duration = d;
                Task::none()
            }
            WinnerChosen(id) => {
                if let PredictionRun::Live(d) = &self.prediction.run {
                    let token = self.access_token.clone().unwrap_or_default();
                    let prediction_id = d.id.clone();
                    let request = EndPredictionRequest {
                        outcome_id: id,
                        broadcaster_id: self.broadcaster_id.clone().unwrap_or_default(),
                        prediction_id,
                    };
                    let client = self.client.clone();
                    Task::perform(
                        async move { end_prediction(&client, request, &token).await },
                        |r| Message::Prediction(PredictionEnded(r)),
                    )
                } else {
                    Task::done(Message::Error(
                        "Should have prediction state here.".to_string(),
                    ))
                }
            }
            PredictionEnded(r) => self.set_prediction_phase(r),
            ConfigSelected(c) => {
                if let Some(state) =
                    load_config::<PredictionState>(&self.config_path, "predictions", &c)
                {
                    self.prediction.form = state;
                    self.prediction.configs.loaded = true;
                    self.prediction.configs.selected = Some(c);
                }
                Task::none()
            }
            LockPrediction => {
                let token = self.access_token.clone().unwrap_or_default();
                match self.create_end_prediction_request() {
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
                let token = self.access_token.clone().unwrap_or_default();
                match self.create_end_prediction_request() {
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

    fn create_end_prediction_request(&self) -> Result<EndPredictionRequest, String> {
        if let PredictionRun::Live(d) = &self.prediction.run {
            let prediction_id = d.id.clone();
            Ok(EndPredictionRequest {
                outcome_id: String::new(),
                broadcaster_id: self.broadcaster_id.clone().unwrap_or_default(),
                prediction_id,
            })
        } else {
            Err("Prediction run should be live".to_string())
        }
    }

    pub fn get_prediction_tab_content(&self) -> Element<'static, Message, Theme, Renderer> {
        let dropdown: PickList<'_, String, Vec<String>, String, Message> = pick_list(
            self.prediction.configs.items.clone(),
            self.prediction.configs.selected.clone(),
            |t| Message::Prediction(PredictionMessage::ConfigSelected(t)),
        )
        .placeholder("Select a config to load");
        let state = self.prediction.form.clone();
        let editable = self.prediction.run == PredictionRun::Idle;
        let phase = if let PredictionRun::Live(d) = &self.prediction.run {
            Some(d.status.clone())
        } else {
            None
        };

        let mut name_input: TextInput<_> = text_input("Config Name", &state.name);
        if !self.prediction.configs.loaded {
            name_input =
                name_input.on_input(|n| Message::Prediction(PredictionMessage::NameChanged(n)));
        }
        let new_btn: Button<_> = button("New")
            .on_press(Message::Prediction(PredictionMessage::NewConfig))
            .style(style::neutral_button);

        let can_save = self.prediction.configs.loaded
            || !self
                .prediction
                .configs
                .items
                .contains(&self.prediction.form.name);

        let save_btn = button("Save").style(style::neutral_button);
        let save_elem: Element<'_, Message> = if can_save {
            save_btn
                .on_press(Message::Prediction(PredictionMessage::SaveConfig))
                .into()
        } else {
            tooltip(
                save_btn,
                container("Config with this name already exists, to change load the config first.")
                    .padding(10)
                    .style(container::dark),
                tooltip::Position::Bottom,
            )
            .into()
        };

        let save_row = row![dropdown, name_input, new_btn, save_elem].spacing(SPACING);

        let title_input = text_input("Prediction title", &state.title)
            .on_input(|r| Message::Prediction(PredictionMessage::TitleChanged(r)));
        let mut opt_col: Column<_> = iced::widget::column![].spacing(SPACING);

        if editable {
            for (idx, option) in state.options.iter().enumerate() {
                let input =
                    text_input(format!("Option {}", idx + 1).as_str(), option).on_input(move |s| {
                        Message::Prediction(PredictionMessage::OptionChanged(idx, s))
                    });
                let mut rem_btn = button(text("-").center())
                    .width(30)
                    .style(style::red_button);
                if state.options.len() > 2 {
                    rem_btn =
                        rem_btn.on_press(Message::Prediction(PredictionMessage::RemoveOption(idx)));
                }
                opt_col = opt_col.push(row![rem_btn, input].spacing(SPACING));
            }
        } else {
            let options = if let PredictionRun::Live(d) = &self.prediction.run {
                d.outcomes.clone()
            } else {
                vec![]
            };
            for option in options.clone().iter() {
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
        if !editable {
            duration_inp = duration_inp.on_input_maybe(None::<fn(usize) -> Message>);
        }

        let duration_row = row![duration_text, duration_inp].align_y(Center);

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
            save_row,
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
