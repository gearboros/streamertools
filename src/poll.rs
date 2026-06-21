use crate::poll::PollMessage::DurationChange;
use crate::twitch_api::{
    CreatePollRequest, PollChoice, PollPhase, PollStateData, create_poll, end_poll,
};
use crate::{App, AppPhase, Message, SPACING};
use iced::widget::{
    Button, Checkbox, Column, Container, PickList, Text, TextInput, button, checkbox, column,
    pick_list, row, rule, text_input,
};
use iced::{Center, Element, Length, Renderer, Task, Theme};
use iced_aw::number_input;
use serde::{Deserialize, Serialize};
use std::fs;

impl App {
    pub fn handle_poll(&mut self, poll_message: PollMessage) -> Task<Message> {
        use crate::poll::PollMessage::*;
        match poll_message {
            TitleChanged(t) => {
                self.poll_state.title = t;
                Task::none()
            }
            Submit => {
                self.poll_state.phase = Some(PollPhase::Active);
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
                    Task::none()
                }
                Err(e) => Task::done(Message::Error(e.to_string())),
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
                    |_| Message::Poll(PollEnded),
                )
            }
            PollEnded => {
                self.poll_state.current_state = None;
                Task::none()
            }
            SaveConfig => {
                let json = serde_json::to_string(&self.poll_state).unwrap();
                let poll = self.poll_state.name.clone();
                let path = self
                    .config_path
                    .join("polls")
                    .join(format!("{}.json", poll));
                fs::write(&path, json).unwrap();
                let polls = Self::load_files(self.config_path.join("polls"));
                self.polls = polls;
                self.selected_poll = Some(poll);
                self.poll_loaded = true;
                Task::none()
            }
            ConfigSelected(c) => {
                self.selected_poll = Some(c.clone());
                let selection = &self.selected_poll;
                if let Some(poll) = selection {
                    let path = &self
                        .config_path
                        .join("polls")
                        .join(format!("{}.json", poll));
                    let config: Option<PollState> = fs::read_to_string(path).ok().and_then(|t| {
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
            NewConfig => {
                self.poll_loaded = false;
                self.selected_poll = None;
                Task::none()
            }
            NameChanged(name) => {
                self.poll_state.name = name;
                Task::none()
            }
        }
    }

    pub(crate) fn get_poll_tab_content(&self) -> Element<'static, Message, Theme, Renderer> {
        let dropdown: PickList<'_, String, Vec<String>, String, Message> =
            pick_list(self.polls.clone(), self.selected_poll.clone(), |t| {
                Message::Poll(PollMessage::ConfigSelected(t))
            });
        let state = self.poll_state.clone();
        let mut name_input: TextInput<_> = text_input("Config Name", &state.name);
        if !self.poll_loaded {
            name_input = name_input.on_input(|n| Message::Poll(PollMessage::NameChanged(n)));
        }
        let new_btn: Button<_> = button("New").on_press(Message::Poll(PollMessage::NewConfig));
        let save_btn: Button<_> = button("Save").on_press(Message::Poll(PollMessage::SaveConfig));

        let save_row = row![dropdown, name_input, new_btn, save_btn].spacing(SPACING);

        let title_input = text_input("Poll title", &state.title)
            .on_input(|r| Message::Poll(PollMessage::TitleChanged(r)));
        let mut opt_col: Column<_> = iced::widget::column![].spacing(SPACING);
        for (idx, option) in state.options.iter().enumerate() {
            let input = text_input(format!("Option {}", idx + 1).as_str(), option)
                .on_input(move |s| Message::Poll(PollMessage::OptionChanged(idx, s)));
            let mut rem_btn = button("-");
            if state.options.len() > 2 {
                rem_btn = rem_btn.on_press(Message::Poll(PollMessage::RemoveOption(idx)));
            }
            opt_col = opt_col.push(row![rem_btn, input].spacing(SPACING));
        }

        let add_btn = button("+").on_press(Message::Poll(PollMessage::AddOption));
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
                channel_row,
                rule::horizontal(2),
                btns,
                status_display
            ]
            .spacing(SPACING)
        ])
        .max_width(600)
        .into()
    }
}

fn get_state_view(state: &PollState) -> Element<'static, Message, Theme, Renderer> {
    if state.phase.is_none() {
        Text::new("No Poll active.").into()
    } else if state.phase == Some(PollPhase::Active) {
        column![
            Text::new("Voting active, currently at:"),
            get_votes_result(&state.current_state)
        ]
        .spacing(SPACING)
        .into()
    } else if state.phase == Some(PollPhase::Completed) || state.phase == Some(PollPhase::Archived)
    {
        column![
            Text::new("Voting closed, Result:"),
            get_votes_result(&state.current_state)
        ]
        .spacing(SPACING)
        .into()
    } else {
        Text::new("").into()
    }
}

fn get_votes_result(state: &Option<PollStateData>) -> Element<'static, Message, Theme, Renderer> {
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

    let winner_text = get_winner_text(&state);

    let mut by_votes = state.choices.clone();
    by_votes.sort_by_key(|c| std::cmp::Reverse(c.votes));

    let mut col: Column<_> = column![
        Text::new(winner_text),
        Text::new(format!(
            "Total: {} votes, {} user votes, {} point votes",
            total_votes, total_popular_votes, total_point_votes
        )),
    ]
    .spacing(SPACING);
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
        col = col.push(
            row![
                Text::new(o.title.clone()).width(Length::FillPortion(2)),
                Text::new(format!("{:.2}% of votes", vote_percent)).width(Length::FillPortion(2)),
                Text::new(format!("{:.2}% of user votes", pop_vote_percent))
                    .width(Length::FillPortion(2)),
                Text::new(format!("{:.2}% of point votes", point_vote_percent))
                    .width(Length::FillPortion(2)),
            ]
            .spacing(SPACING),
        );
    }
    col.into()
}

fn get_winner_text(state: &PollStateData) -> String {
    let winner = state
        .choices
        .iter()
        .max_by_key(|c| c.votes)
        .expect("No empty choices");
    let popular_winner = state
        .choices
        .iter()
        .max_by_key(|c| c.popular_votes())
        .expect("No empty choices");
    let point_winner = state
        .choices
        .iter()
        .max_by_key(|c| c.channel_point_votes)
        .expect("No empty choices");

    if winner.id == popular_winner.id && winner.id == point_winner.id {
        format!(
            "{} is the winner, the popular vote winner and the points vote winner!",
            winner.title
        )
    } else if winner.id == point_winner.id || winner.id == popular_winner.id {
        if winner.id == point_winner.id {
            format!(
                "{} is the winner and points vote winner, {} is the popular vote winner",
                winner.title, popular_winner.title
            )
        } else {
            // if winner.id == popular_winner.id {
            format!(
                "{} is the winner and popular vote winner, {} is the points vote winner",
                winner.title, point_winner.title
            )
        }
    } else {
        // not checking for popular winner == point winner but != winner, because it's impossible
        format!(
            "{} is the winner, {} the popular vote winner and {} is the points vote winner",
            winner.title, popular_winner.title, point_winner.title
        )
    }
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
    PollEnded,
    ConfigSelected(String),
    SaveConfig,
    NewConfig,
    NameChanged(String),
}
