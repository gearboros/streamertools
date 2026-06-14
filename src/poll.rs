use std::fs;
use iced::{Center, Element, Renderer, Task, Theme};
use iced::widget::{button, checkbox, pick_list, row, text_input, column, Button, Checkbox, Column, Container, PickList, Text, TextInput, rule};
use iced_aw::number_input;
use rand::prelude::SliceRandom;
use rand::rng;
use serde::{Deserialize, Serialize};
use crate::{App, Message, SPACING};
use crate::poll::PollMessage::{AddOption, ChannelPointsToggled, ConfigLoaded, ConfigSelected, DurationChange, EndPoll, LoadConfig, NameChanged, NewConfig, OptionChanged, PointCostChange, PollCreated, PollEnded, RemoveOption, SaveConfig, Submit, SwitchOptions, TitleChanged};
use crate::twitch_api::{create_poll, end_poll, CreatePollRequest, PollChoice};

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

    pub(crate) fn get_poll_tab_content(&self) -> Element<'static, Message, Theme, Renderer> {
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
        let mut opt_col: Column<_> = iced::widget::column![].spacing(SPACING);
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
        let mut channel_row = iced::widget::column![channel_point_check];
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
            .max_width(600).into()
    }

}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PollState {
    pub title: String,
    pub options: Vec<String>,
    pub duration: usize,
    pub uses_channel_points: bool,
    pub channel_point_cost: usize,
    #[serde(skip_serializing, skip_deserializing)]
    pub id: String,
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