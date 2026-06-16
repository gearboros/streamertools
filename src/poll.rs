use std::fmt::format;
use std::fs;
use iced::{Center, Element, Renderer, Task, Theme};
use iced::widget::{button, checkbox, pick_list, row, text_input, column, Button, Checkbox, Column, Container, PickList, Text, TextInput, rule};
use iced_aw::number_input;
use rand::prelude::SliceRandom;
use rand::rng;
use serde::{Deserialize, Serialize};
use crate::{App, Message, SPACING};
use crate::poll::PollMessage::{AddOption, ChannelPointsToggled, ConfigLoaded, ConfigSelected, DurationChange, EndPoll, LoadConfig, NameChanged, NewConfig, OptionChanged, PointCostChange, PollCreated, PollEnded, RemoveOption, SaveConfig, Submit, SwitchOptions, TitleChanged};
use crate::twitch_api::{create_poll, end_poll, CreatePollRequest, PollChoice, PollPhase, PollStateData};

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
                        self.poll_state.id = Some(id);
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
                let poll_id = self.poll_state.id.clone().unwrap_or_default();
                Task::perform(
                    async move { end_poll(&broadcaster, &poll_id, &token).await },
                    |_| Message::Poll(PollEnded)
                )
            }
            PollEnded => {
                self.poll_state.id = None;
                Task::none()
            }
            SaveConfig => {
                let json = serde_json::to_string(&self.poll_state).unwrap();
                let poll = self.poll_state.name.clone();
                let path = self.config_path.join("polls").join(format!("{}.json", poll));
                fs::write(&path, json).unwrap();
                let polls = Self::load_files(self.config_path.join("polls"));
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

    pub(crate) fn get_poll_tab_content(&self) -> Element<'static, Message, Theme, Renderer> {
        let dropdown: PickList<'_, String, Vec<String>, String, Message> = pick_list(self.polls.clone(), self.selected_poll.clone(), |t| Message::Poll(PollMessage::ConfigSelected(t)));
        let load_btn: Button<_> = button("Load")
            .on_press(Message::Poll(PollMessage::LoadConfig));
        let state = self.poll_state.clone();
        let mut name_input: TextInput<_> = text_input("Config Name", &state.name);
        if !self.poll_loaded {
            name_input = name_input.on_input(|n| Message::Poll(PollMessage::NameChanged(n)));
        }
        let new_btn: Button<_> = button("New")
            .on_press(Message::Poll(PollMessage::NewConfig));
        let save_btn: Button<_> = button("Save")
            .on_press(Message::Poll(PollMessage::SaveConfig));

        let save_row = row![dropdown, name_input, new_btn, load_btn, save_btn].spacing(SPACING);

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
        let switch_btn = button("Switch Options").on_press(Message::Poll(PollMessage::SwitchOptions));
        let shuffle_btn = button("Shuffle Options").on_press(Message::Poll(PollMessage::SwitchOptions));
        let mut option_btn_row = row![add_btn].spacing(SPACING);
        if state.options.len() == 2 {
            option_btn_row = option_btn_row.push(switch_btn)
        } else {
            option_btn_row = option_btn_row.push(shuffle_btn)
        }

        let duration_text = Text::new("Duration in s: ");
        let duration_inp = number_input(&state.duration, 0..=600, |d| Message::Poll(PollMessage::DurationChange(d)));

        let duration_row = row![duration_text, duration_inp].align_y(Center);

        let channel_point_check: Checkbox<_> = checkbox(state.uses_channel_points).label("Enable Channel point votes.")
            .on_toggle(|t| Message::Poll(PollMessage::ChannelPointsToggled(t)));
        let channel_point_text: Text<_> = Text::new("Channel point cost: ");
        let channel_point_input = number_input(&state.channel_point_cost, 0..=1_000_000, |c| Message::Poll(PollMessage::PointCostChange(c)));
        let mut channel_row = iced::widget::column![channel_point_check];
        if state.uses_channel_points {
            channel_row = channel_row.push(row![channel_point_text, channel_point_input].align_y(Center))
        }

        let submit_btn = button("Submit").on_press(Message::Poll(PollMessage::Submit));
        let end_btn = button("End Poll").on_press(Message::Poll(PollMessage::EndPoll));
        let mut btns = row![submit_btn].spacing(SPACING);
        if !&state.id.is_none() {
            btns = btns.push(end_btn)
        }

        let status_text = get_state_text(&state);
        let status_display = Text::new(status_text);

        Container::new(row![column![save_row, title_input, opt_col, option_btn_row, rule::horizontal(2), duration_row, rule::horizontal(2), channel_row, rule::horizontal(2), btns, status_display].spacing(SPACING)])
            .max_width(600).into()
    }

}

fn get_state_text(state: &PollState) -> String {
    if state.phase.is_none() {
        String::from("No Poll active.")
    } else if state.phase == Some(PollPhase::Active) {
        let distribution = get_votes_result(&state.current_state);
        String::from(format!("Voting active, currently at: {}", distribution))
    } else if state.phase == Some(PollPhase::Completed) || state.phase == Some(PollPhase::Archived) {
        let distribution = get_votes_result(&state.current_state);
        String::from(format!("Voting closed, Result: {}", distribution))
    } else {
        String::new()
    }
}

fn get_votes_result(state: &Option<PollStateData>) -> String {
    state.clone().map_or(String::from("No Poll Active"), |state| {
        let total_votes = state.choices.iter().map(|c| c.votes).sum::<i32>();
        let total_popular_votes = state.choices.iter().map(|c| c.popular_votes()).sum::<i32>();
        let total_point_votes = state.choices.iter().map(|c| c.channel_point_votes).sum::<i32>();

        let winner_text = get_winner_text(&state);

        let mut by_votes = state.choices.clone();
        by_votes.sort_by_key(|c| std::cmp::Reverse(c.votes));

        let list = by_votes.iter().fold(String::new(), |acc, o| {
            let vote_percent = if total_votes == 0 { 0f64 } else { (o.votes as f64) / (total_votes as f64) * 100.0 };
            let pop_vote_percent = if total_popular_votes == 0 { 0f64 } else { (o.popular_votes() as f64) / (total_popular_votes as f64) * 100.0 };
            let point_vote_percent = if total_point_votes == 0 { 0f64 } else { (o.channel_point_votes as f64) / (total_point_votes as f64) * 100.0 };
            let current = format!("- {}: {:.2} of votes, {:.2}% of user votes, {:.2}% of point votes.\n", o.title, vote_percent, pop_vote_percent, point_vote_percent);
            acc + current.as_str()
        });
        String::from(format!("{}\n, Total: {} votes, {} user votes, {} point votes\n{}", winner_text, total_votes, total_popular_votes, total_point_votes, list))
    })
}

fn get_winner_text(state: &PollStateData) -> String {
    let winner = state.choices.iter().max_by_key(|c| c.votes);
    let popular_winner = state.choices.iter().max_by_key(|c| c.popular_votes());
    let point_winner = state.choices.iter().max_by_key(|c| c.channel_point_votes);

    if winner.unwrap().id == popular_winner.unwrap().id && winner.unwrap().id == point_winner.unwrap().id {
        format!("{} is the winner, the popular vote winner and the points vote winner!", winner.unwrap().title)
    } else if winner.unwrap().id == point_winner.unwrap().id || winner.unwrap().id == popular_winner.unwrap().id {
        if winner.unwrap().id == point_winner.unwrap().id {
            format!("{} is the winner and points vote winner, {} is the popular vote winner", winner.unwrap().title, popular_winner.unwrap().title)
        } else { // if winner.unwrap().id == popular_winner.unwrap().id {
            format!("{} is the winner and popular vote winner, {} is the points vote winner", winner.unwrap().title, point_winner.unwrap().title)
        }
    } else {
        // popular vote = point winner but != winner should be impossible ... right?
        format!("{} is the winner, {} the popular vote winner and {} is the points vote winner", winner.unwrap().title, popular_winner.unwrap().title, point_winner.unwrap().title)
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
    pub id: Option<String>,
    #[serde(skip_serializing, skip_deserializing)]
    pub phase: Option<PollPhase>,
    #[serde(skip_serializing, skip_deserializing)]
    pub current_state: Option<PollStateData>,
    pub name: String
}

#[derive(Debug, Clone)]
pub enum PollMessage {
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