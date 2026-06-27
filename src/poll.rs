use crate::poll::PollMessage::DurationChange;
use crate::sample_data::{
    poll_points_winner, poll_popular_winner, poll_total_winner, running_poll,
};
use crate::style::bold_text;
use crate::twitch_api::{
    create_poll, end_poll, CreatePollRequest, PollChoice, PollChoiceState, PollPhase, PollStateData,
};
use crate::AppPhase::NoPolling;
use crate::{load_config, save_config, App, AppPhase, Message, BIG_SPACING, SPACING};
use iced::widget::{
    button, checkbox, column, container, pick_list, row, rule, text, text_input, tooltip,
    Button, Checkbox, Column, Container, PickList, Text, TextInput,
};
use iced::{Center, Element, Length, Renderer, Task, Theme};
use iced_aw::number_input;
use serde::{Deserialize, Serialize};

impl App {
    pub fn handle_poll(&mut self, poll_message: PollMessage) -> Task<Message> {
        use crate::poll::PollMessage::*;
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
                    choices: self
                        .poll_state
                        .options
                        .clone()
                        .iter()
                        .map(|o| PollChoice { title: o.clone() })
                        .collect(),
                    duration: self.poll_state.duration * 60,
                    channel_points_voting_enabled: self.poll_state.uses_channel_points,
                    channel_points_per_vote: self.poll_state.channel_point_cost,
                };
                let client = self.client.clone();
                Task::perform(
                    async move { create_poll(&client, &token, request).await },
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
            PollCreated(r) => match r {
                Ok(resp) => {
                    self.phase = AppPhase::PollPolling;
                    self.poll_state.current_state = Some(resp);
                    self.poll_state.phase = Some(PollPhase::Active);
                    Task::none()
                }
                Err(e) => {
                    self.poll_state.phase = None;
                    Task::done(Message::Error(e.to_string()))
                }
            },
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
                let poll_id = self.poll_state.current_state.clone().unwrap().id.clone();
                let client = self.client.clone();
                Task::perform(
                    async move { end_poll(&client, &broadcaster, &poll_id, &token).await },
                    |r| Message::Poll(PollEnded(r)),
                )
            }
            PollEnded(r) => match r {
                Ok(()) => {
                    self.phase = NoPolling;
                    self.poll_state.current_state = None;
                    Task::none()
                }
                Err(e) => Task::done(Message::Error(e)),
            },
            SaveConfig => {
                if let Err(e) = save_config(
                    &self.config_path,
                    "polls",
                    &self.poll_state.name,
                    &self.poll_state,
                ) {
                    return Task::done(Message::Error(e.to_string()));
                };
                self.polls = Self::load_files(self.config_path.join("polls"));
                self.selected_poll = Some(self.poll_state.name.clone());
                self.poll_loaded = true;
                Task::none()
            }
            ConfigSelected(c) => {
                if let Some(state) = load_config::<PollState>(&self.config_path, "polls", &c) {
                    self.poll_state = state;
                    self.poll_loaded = true;
                }
                self.selected_poll = Some(c);
                Task::none()
            }
            NewConfig => {
                self.poll_state.name = String::new();
                self.poll_loaded = false;
                self.selected_poll = None;
                Task::none()
            }
            NameChanged(name) => {
                self.poll_state.name = name;
                Task::none()
            }
            LoadSampleData(data) => {
                self.poll_state.current_state = Some(data);
                self.poll_state.phase = Some(self.poll_state.current_state.clone().unwrap().status);
                Task::none()
            }
        }
    }

    pub(crate) fn get_poll_tab_content(&self) -> Element<'static, Message, Theme, Renderer> {
        let dropdown: PickList<'_, String, Vec<String>, String, Message> =
            pick_list(self.polls.clone(), self.selected_poll.clone(), |t| {
                Message::Poll(PollMessage::ConfigSelected(t))
            })
            .placeholder("Select a config to load");
        let state = self.poll_state.clone();
        let mut name_input: TextInput<_> = text_input("Config Name", &state.name);
        if !self.poll_loaded {
            name_input = name_input.on_input(|n| Message::Poll(PollMessage::NameChanged(n)));
        }
        let new_btn: Button<_> = button("New")
            .on_press(Message::Poll(PollMessage::NewConfig))
            .style(crate::style::neutral_button);

        let can_save =
            self.poll_loaded || (!self.poll_loaded && !self.polls.contains(&self.poll_state.name));

        let save_btn = button("Save").style(crate::style::neutral_button);
        let save_elem: Element<'_, Message> = if can_save {
            save_btn
                .on_press(Message::Poll(PollMessage::SaveConfig))
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

        let title_input = text_input("Poll title", &state.title)
            .on_input(|r| Message::Poll(PollMessage::TitleChanged(r)));
        let mut opt_col: Column<_> = iced::widget::column![].spacing(SPACING);
        for (idx, option) in state.options.iter().enumerate() {
            let input = text_input(format!("Option {}", idx + 1).as_str(), option)
                .on_input(move |s| Message::Poll(PollMessage::OptionChanged(idx, s)));
            let mut rem_btn = button(text("-").center())
                .width(30)
                .style(crate::style::red_button);
            if state.options.len() > 2 {
                rem_btn = rem_btn.on_press(Message::Poll(PollMessage::RemoveOption(idx)));
            }
            opt_col = opt_col.push(row![rem_btn, input].spacing(SPACING));
        }

        let add_btn = button(text("+").center())
            .width(30)
            .on_press(Message::Poll(PollMessage::AddOption));
        let option_btn_row = row![add_btn].spacing(SPACING);

        let duration_text = Text::new("Duration in mins: ");
        let mut duration_inp = number_input(&state.duration, 1..=30, |d| {
            Message::Poll(DurationChange(d))
        });
        if state.current_state.is_some() {
            duration_inp = duration_inp.on_input_maybe(None::<fn(usize) -> Message>)
        }

        let duration_row = row![duration_text, duration_inp].align_y(Center);

        let channel_point_check: Checkbox<_> = checkbox(state.uses_channel_points)
            .label("Enable Channel point votes.")
            .on_toggle(|t| Message::Poll(PollMessage::ChannelPointsToggled(t)));
        let channel_point_text: Text<_> = Text::new("Channel point cost: ");
        let channel_point_input = number_input(&state.channel_point_cost, 0..=1_000_000, |c| {
            Message::Poll(PollMessage::PointCostChange(c))
        });
        let mut channel_row = iced::widget::column![channel_point_check];
        if state.uses_channel_points {
            channel_row =
                channel_row.push(row![channel_point_text, channel_point_input].align_y(Center))
        }

        let submit_btn = button("Submit").on_press(Message::Poll(PollMessage::Submit));
        let end_btn = button("End Poll").on_press(Message::Poll(PollMessage::EndPoll));
        let mut btns = row![submit_btn].spacing(SPACING);
        if state.phase == Some(PollPhase::Active) {
            btns = btns.push(end_btn)
        }

        let mut dbg_row = column![];
        if self.debug {
            let total_winner = button("Total Winner")
                .style(crate::style::dbg_button)
                .on_press(Message::Poll(PollMessage::LoadSampleData(
                    poll_total_winner(),
                )));
            let points_winner = button("Winner wins points, loses popular")
                .style(crate::style::dbg_button)
                .on_press(Message::Poll(PollMessage::LoadSampleData(
                    poll_points_winner(),
                )));
            let popular_winner = button("Winner wins popular, loses points")
                .style(crate::style::dbg_button)
                .on_press(Message::Poll(PollMessage::LoadSampleData(
                    poll_popular_winner(),
                )));
            let running = button("Running")
                .style(crate::style::dbg_button)
                .on_press(Message::Poll(PollMessage::LoadSampleData(running_poll())));
            dbg_row = column![
                rule::horizontal(2),
                row![total_winner, points_winner, popular_winner, running].spacing(SPACING)
            ];
        }

        let status_display = get_state_view(&state);

        let form = column![
            save_row,
            rule::horizontal(2),
            title_input,
            opt_col,
            option_btn_row,
            duration_row,
            channel_row,
            rule::horizontal(2),
            btns,
            dbg_row,
        ]
        .spacing(SPACING);

        let results = container(status_display)
            .padding(SPACING as u16 * 2)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(container::rounded_box);

        Container::new(
            row![
                container(form).width(Length::FillPortion(2)).max_width(600),
                container(results).width(Length::FillPortion(3)),
            ]
            .spacing(SPACING * 2),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

fn get_state_view(state: &PollState) -> Element<'static, Message, Theme, Renderer> {
    if state.phase.is_none() {
        crate::empty_panel("📊", "No poll running yet")
    } else {
        let (winner, popular_winner, point_winner) =
            get_winners(&state.current_state.clone().unwrap());
        let active = state.phase == Some(PollPhase::Active);
        let main_label = if active {
            "Voting active, current leader: "
        } else {
            "Voting ended, winner: "
        };
        let popular_label = if active {
            "Popular vote leader: "
        } else {
            "Popular vote winner: "
        };
        let point_label = if active {
            "Point vote leader: "
        } else {
            "Point vote winner: "
        };
        let mut col = column![row![
            Text::new(main_label),
            bold_text(winner.title.clone())
        ],];
        if winner.id != popular_winner.id {
            col = col.push(row![
                Text::new(popular_label),
                bold_text(popular_winner.title.clone())
            ])
        }
        if winner.id != point_winner.id {
            col = col.push(row![
                Text::new(point_label),
                bold_text(point_winner.title.clone())
            ])
        }
        col = col.push(get_votes_result(
            &state.current_state,
            state.channel_point_cost,
        ));
        col.spacing(SPACING).into()
    }
}

fn get_votes_result(
    state: &Option<PollStateData>,
    cost: usize,
) -> Element<'static, Message, Theme, Renderer> {
    let Some(state) = state.clone() else {
        return Text::new("No Poll Active").into();
    };
    let total_votes = state.choices.iter().map(|c| c.votes).sum::<i32>();
    let total_popular_votes = state.choices.iter().map(|c| c.popular_votes()).sum::<i32>();
    let total_point_votes = state
        .choices
        .iter()
        .map(|c| c.channel_point_votes)
        .sum::<i32>();

    let mut by_votes = state.choices.clone();
    by_votes.sort_by_key(|c| std::cmp::Reverse(c.votes));

    let mut title_col: Column<_> = column![bold_text("".to_string())].spacing(SPACING);
    let mut votes_col: Column<_> = column![bold_text("Votes".to_string())].spacing(SPACING);
    let mut user_col: Column<_> = column![bold_text("User votes".to_string())].spacing(SPACING);
    let mut point_col: Column<_> = column![bold_text("Point votes".to_string())].spacing(SPACING);

    for o in &by_votes {
        let vote_percent = if total_votes == 0 {
            0f64
        } else {
            (o.votes as f64) / (total_votes as f64) * 100.0
        };
        let pop_vote_percent = if total_popular_votes == 0 {
            0f64
        } else {
            (o.popular_votes() as f64) / (total_popular_votes as f64) * 100.0
        };
        let point_vote_percent = if total_point_votes == 0 {
            0f64
        } else {
            (o.channel_point_votes as f64) / (total_point_votes as f64) * 100.0
        };
        title_col = title_col.push(text(format!("• {}", o.title)));
        votes_col = votes_col.push(text(format!("{} votes, {:.2}%", o.votes, vote_percent)));
        user_col = user_col.push(text(format!(
            "{} votes, {:05.2}%",
            o.popular_votes(),
            pop_vote_percent
        )));
        point_col = point_col.push(text(format!(
            "{} votes, {:.2}% ({} points)",
            o.channel_point_votes,
            point_vote_percent,
            o.channel_point_votes * (cost as i32)
        )));
    }

    let grid = row![title_col, votes_col, user_col, point_col].spacing(BIG_SPACING);

    container(
        column![
            Text::new(format!(
                "Total: {} votes, {} user votes, {} point votes ({} points)",
                total_votes,
                total_popular_votes,
                total_point_votes,
                total_point_votes * (cost as i32)
            )),
            grid,
        ]
        .spacing(SPACING),
    )
    .into()
}

fn get_winners(state: &PollStateData) -> (PollChoiceState, PollChoiceState, PollChoiceState) {
    let winner = state
        .choices
        .iter()
        .max_by_key(|c| c.votes)
        .expect("No empty choices")
        .clone();
    let popular_winner = state
        .choices
        .iter()
        .max_by_key(|c| c.popular_votes())
        .expect("No empty choices")
        .clone();
    let point_winner = state
        .choices
        .iter()
        .max_by_key(|c| c.channel_point_votes)
        .expect("No empty choices")
        .clone();

    (winner, popular_winner, point_winner)
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct PollState {
    pub title: String,
    pub options: Vec<String>,
    pub duration: usize,
    pub uses_channel_points: bool,
    pub channel_point_cost: usize,
    #[serde(skip_serializing, skip_deserializing)]
    pub phase: Option<PollPhase>,
    #[serde(skip_serializing, skip_deserializing)]
    pub current_state: Option<PollStateData>,
    pub name: String,
}

#[derive(Debug, Clone)]
pub enum PollMessage {
    TitleChanged(String),
    OptionChanged(usize, String),
    AddOption,
    RemoveOption(usize),
    Submit,
    PollCreated(Result<PollStateData, String>),
    DurationChange(usize),
    ChannelPointsToggled(bool),
    PointCostChange(usize),
    EndPoll,
    PollEnded(Result<(), String>),
    ConfigSelected(String),
    SaveConfig,
    NewConfig,
    NameChanged(String),
    LoadSampleData(PollStateData),
}
