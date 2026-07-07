use crate::base_form::{handle_base_changes, BaseFormMessage, EditableForm};
use crate::chart::{BarChart, BarData};
use crate::config::{handle_config, ConfigForm, ConfigList, ConfigMessage, Named};
use crate::sample_data::{
    poll_points_winner, poll_popular_winner, poll_tie, poll_total_winner, running_poll,
};
use crate::style::{bold_text, poll_colors, thousand_separator};
use crate::twitch_api::{create_poll, end_poll};
use crate::twitch_types::{
    CreatePollRequest, PollChoice, PollChoiceState, PollPhase, PollStateData,
};
use crate::widgets::{config_bar, duration_row, option_editor};
use crate::{style, App, Message, BIG_SPACING, SPACING};
use iced::widget::{
    button, canvas, checkbox, column, container, row, rule, text, text_input, Checkbox, Column, Row,
    Text,
};
use iced::{Center, Color, Element, Length, Renderer, Task, Theme};
use iced_aw::number_input;
use iced_aw::style::colors::BLACK;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PollBarTabId {
    #[default]
    Total,
    Points,
    Users,
}

impl PollBarTabId {
    pub fn idx(self) -> usize {
        match self {
            PollBarTabId::Users => 2,
            PollBarTabId::Points => 1,
            PollBarTabId::Total => 0,
        }
    }

    pub fn from_idx(idx: usize) -> Self {
        match idx {
            0 => PollBarTabId::Total,
            1 => PollBarTabId::Points,
            2 => PollBarTabId::Users,
            _ => PollBarTabId::Total,
        }
    }
}

#[derive(Default, Debug)]
pub struct PollTab {
    pub form: PollState,
    pub run: PollRun,
    pub configs: ConfigList,
    pub active_tab: PollBarTabId,
}

impl ConfigForm for PollTab {
    type Form = PollState;
    const SUBDIR: &'static str = "polls";

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
pub enum PollRun {
    #[default]
    Idle,
    Live(PollStateData),
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct PollState {
    pub title: String,
    pub options: Vec<String>,
    pub duration: usize,
    pub uses_channel_points: bool,
    pub channel_point_cost: usize,
    pub name: String,
}

impl Named for PollState {
    fn name(&self) -> &str {
        &self.name
    }

    fn set_name(&mut self, name: String) {
        self.name = name;
    }
}

impl EditableForm for PollState {
    const MAX_OPTIONS: usize = 5;

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

impl App {
    pub fn handle_poll(&mut self, poll_message: PollMessage) -> Task<Message> {
        use crate::poll::PollMessage::*;
        match poll_message {
            Submit => {
                let (token, broadcaster_id) = match self.require_token_and_broadcaster_id() {
                    Ok(v) => v,
                    Err(e) => return Self::log_and_show_error(&e),
                };
                let request = CreatePollRequest {
                    broadcaster_id,
                    title: self.poll.form.title.clone(),
                    choices: self
                        .poll
                        .form
                        .options
                        .iter()
                        .map(|o| PollChoice { title: o.clone() })
                        .collect(),
                    // minutes -> seconds for API
                    duration: self.poll.form.duration * 60,
                    channel_points_voting_enabled: self.poll.form.uses_channel_points,
                    channel_points_per_vote: self.poll.form.channel_point_cost,
                };
                let client = self.client.clone();
                Task::perform(
                    async move { create_poll(&client, &token, request).await },
                    |r| Message::Poll(PollCreated(r)),
                )
            }
            PollCreated(r) => self.set_poll_run(r),
            ChannelPointsToggled(t) => {
                self.poll.form.uses_channel_points = t;
                Task::none()
            }
            PointCostChange(c) => {
                self.poll.form.channel_point_cost = c;
                Task::none()
            }
            EndPoll => {
                let (token, broadcaster_id) = match self.require_token_and_broadcaster_id() {
                    Ok(v) => v,
                    Err(e) => return Self::log_and_show_error(&e),
                };
                let PollRun::Live(d) = &self.poll.run else {
                    return Self::log_and_show_error("No current poll when trying to end a poll");
                };
                let poll_id = d.id.clone();
                let client = self.client.clone();
                Task::perform(
                    async move { end_poll(&client, &broadcaster_id, &poll_id, &token).await },
                    |r| Message::Poll(PollEnded(r)),
                )
            }
            PollEnded(r) => self.set_poll_run(r),
            Config(c) => handle_config(&self.config_path, c, &mut self.poll),
            LoadSampleData(data) => {
                self.poll.run = PollRun::Live(data);
                Task::none()
            }
            TabSelected(idx) => {
                self.poll.active_tab = idx;
                Task::none()
            }
            BaseFormChange(b) => handle_base_changes(&mut self.poll.form, b),
        }
    }

    fn set_poll_run(&mut self, result: Result<PollStateData, String>) -> Task<Message> {
        match result {
            Ok(d) => {
                self.poll.run = PollRun::Live(d);
                Task::none()
            }
            Err(e) => Task::done(Message::Error(e)),
        }
    }

    pub fn get_poll_tab_content(&self) -> Element<'_, Message, Theme, Renderer> {
        let state = &self.poll.form;
        let editable = self.poll.run == PollRun::Idle;
        let phase = if let PollRun::Live(d) = &self.poll.run {
            Some(d.status.clone())
        } else {
            None
        };

        let save_row = config_bar(
            &self.poll.configs,
            &state.name,
            |t| Message::Poll(PollMessage::Config(ConfigMessage::ConfigSelected(t))),
            |n| Message::Poll(PollMessage::Config(ConfigMessage::NameChanged(n))),
            Message::Poll(PollMessage::Config(ConfigMessage::New)),
            Message::Poll(PollMessage::Config(ConfigMessage::Save)),
        );

        let title_input = text_input("Poll title", &state.title).on_input(|r| {
            Message::Poll(PollMessage::BaseFormChange(BaseFormMessage::TitleChanged(
                r,
            )))
        });

        let opt_col = option_editor(
            &state.options,
            editable,
            |i, s| {
                Message::Poll(PollMessage::BaseFormChange(BaseFormMessage::OptionChanged(
                    i, s,
                )))
            },
            |idx| {
                Message::Poll(PollMessage::BaseFormChange(BaseFormMessage::RemoveOption(
                    idx,
                )))
            },
        );

        let mut add_btn = button(text("+").center()).width(30);
        if editable && state.options.len() < PollState::MAX_OPTIONS {
            add_btn = add_btn.on_press(Message::Poll(PollMessage::BaseFormChange(
                BaseFormMessage::AddOption,
            )));
        }
        let option_btn_row = row![add_btn].spacing(SPACING);

        let duration_row = duration_row(editable, &state.duration, |d| {
            Message::Poll(PollMessage::BaseFormChange(
                BaseFormMessage::DurationChanged(d),
            ))
        });

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

        let mut submit_btn = button("Submit");
        if editable {
            submit_btn = submit_btn.on_press(Message::Poll(PollMessage::Submit));
        }
        let end_btn = button("End Poll").on_press(Message::Poll(PollMessage::EndPoll));
        let mut btns = row![submit_btn].spacing(SPACING);
        if phase == Some(PollPhase::Active) {
            btns = btns.push(end_btn)
        }

        let mut dbg_row = column![];
        if self.sample {
            // only shown with --sample, buttons to show sample results for testing
            let total_winner =
                button("Total Winner")
                    .style(style::dbg_button)
                    .on_press(Message::Poll(PollMessage::LoadSampleData(
                        poll_total_winner(),
                    )));
            let points_winner = button("Winner wins points, loses popular")
                .style(style::dbg_button)
                .on_press(Message::Poll(PollMessage::LoadSampleData(
                    poll_points_winner(),
                )));
            let popular_winner = button("Winner wins popular, loses points")
                .style(style::dbg_button)
                .on_press(Message::Poll(PollMessage::LoadSampleData(
                    poll_popular_winner(),
                )));
            let running = button("Running")
                .style(style::dbg_button)
                .on_press(Message::Poll(PollMessage::LoadSampleData(running_poll())));
            let tie = button("Two Winners (tie)")
                .style(style::dbg_button)
                .on_press(Message::Poll(PollMessage::LoadSampleData(poll_tie())));
            dbg_row = column![
                rule::horizontal(2),
                row![total_winner, points_winner, popular_winner, running, tie].spacing(SPACING)
            ];
        }

        let status_display = get_state_view(state, &self.poll.run, self.poll.active_tab);

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

        crate::widgets::split_pane(form, results)
    }
}

fn get_state_view(
    state: &PollState,
    run: &PollRun,
    active_tab: PollBarTabId,
) -> Element<'static, Message, Theme, Renderer> {
    if let PollRun::Live(d) = run {
        let (winners, popular_winners, point_winners) = get_winners(d);
        let active = d.status == PollPhase::Active;
        let main_label = if active {
            format!(
                "Voting active, current {}: ",
                winner_noun(active, winners.len())
            )
        } else {
            format!("Voting ended, {}: ", winner_noun(active, winners.len()))
        };
        let popular_label = format!(
            "Popular vote {}: ",
            winner_noun(active, popular_winners.len())
        );
        let non_tied_label = "Non tied winner: ";
        let point_label = format!("Point vote {}: ", winner_noun(active, point_winners.len()));
        let mut col = column![row![Text::new(main_label), bold_text(titles(&winners))],];
        if !same_set(&winners, &popular_winners) {
            col = col.push(row![
                Text::new(popular_label),
                bold_text(titles(&popular_winners))
            ])
        }
        if !same_set(&winners, &point_winners) {
            col = col.push(row![
                Text::new(point_label),
                bold_text(titles(&point_winners))
            ])
        }
        if let Some(non_tied_winner) = get_non_tied_winner(d)
            && !winners.iter().any(|w| w.id == non_tied_winner.id)
        {
            col = col.push(row![
                Text::new(non_tied_label),
                bold_text(non_tied_winner.title.clone())
            ])
        };
        col = col.push(get_votes_result(&Some(d), state.channel_point_cost));

        // I'd love to have a voter breakdown similar to the top predictor breakdown here
        // but Twitch API is being weird and for some reason doesn't return top voters, but top predictors
        // It's technically possible via private GraphQL Endpoint, but we'd need to get a browser token for that
        // you can vote for it here https://twitch.uservoice.com/forums/310213-developers/suggestions/51471106-top-point-voters-as-part-of-poll-result
        let tab_bar = get_tab_bar(active_tab);

        let bar_chart = canvas(get_bar_chart(active_tab, d))
            .width(Length::Fill)
            .height(Length::Fill);

        col = col.push(tab_bar);
        col = col.push(bar_chart);
        col.spacing(SPACING).into()
    } else {
        crate::widgets::empty_panel("📊", "No poll running yet")
    }
}

fn get_bar_chart(active_tab: PollBarTabId, d: &PollStateData) -> BarChart {
    let mut data: Vec<BarData> = d
        .choices
        .iter()
        .map(|c| BarData {
            color: get_choice_color(&d.choices, &c.id),
            title: c.title.clone(),
            value: {
                if active_tab == PollBarTabId::Points {
                    c.channel_points_votes
                } else if active_tab == PollBarTabId::Users {
                    c.popular_votes()
                } else {
                    c.votes
                }
            },
        })
        .collect();
    data.sort_by_key(|d| std::cmp::Reverse(d.value));
    BarChart { data }
}

// get color by position of id in original choice array, so sorting can't break it.
fn get_choice_color(choices: &[PollChoiceState], id: &str) -> Color {
    let colors = poll_colors();
    choices
        .iter()
        .position(|c| c.id == id)
        .filter(|&i| i < colors.len())
        .map_or(BLACK, |i| colors[i])
}

fn get_tab_bar(active_tab: PollBarTabId) -> Row<'static, Message> {
    let colors = style::poll_tab_colors();
    let tab_button = |label: &'static str, color: Color, idx: usize| {
        button(text(label))
            .style(move |_, status| style::color_button(color, status, active_tab.idx() == idx))
            .padding(SPACING as u16)
            .on_press(Message::Poll(PollMessage::TabSelected(
                PollBarTabId::from_idx(idx),
            )))
    };

    row![
        tab_button(
            "Total",
            colors[PollBarTabId::Total.idx()],
            PollBarTabId::Total.idx()
        ),
        tab_button(
            "Points",
            colors[PollBarTabId::Points.idx()],
            PollBarTabId::Points.idx()
        ),
        tab_button(
            "Users",
            colors[PollBarTabId::Users.idx()],
            PollBarTabId::Users.idx()
        ),
    ]
}

fn titles(choices: &[PollChoiceState]) -> String {
    choices
        .iter()
        .map(|c| c.title.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

fn winner_noun(active: bool, count: usize) -> String {
    let mut noun = if active {
        "leader".to_string()
    } else {
        "winner".to_string()
    };
    if count > 1 {
        noun.push('s')
    }
    noun
}

// Compares two winner sets by choice id. Used so the results view only shows the separate
// popular-vote / point-vote winner rows when they differ from the overall winner set.
fn same_set(a: &[PollChoiceState], b: &[PollChoiceState]) -> bool {
    a.len() == b.len() && {
        let ids: std::collections::HashSet<&str> = a.iter().map(|c| c.id.as_str()).collect();
        b.iter().all(|c| ids.contains(c.id.as_str()))
    }
}

fn winners_by(
    choices: &[PollChoiceState],
    key: impl Fn(&PollChoiceState) -> i64,
) -> Vec<PollChoiceState> {
    match choices.iter().map(&key).max() {
        Some(max) => choices.iter().filter(|c| key(c) == max).cloned().collect(),
        None => Vec::new(),
    }
}

fn get_winners(
    state: &PollStateData,
) -> (
    Vec<PollChoiceState>,
    Vec<PollChoiceState>,
    Vec<PollChoiceState>,
) {
    (
        winners_by(&state.choices, |c| c.votes),
        winners_by(&state.choices, |c| c.popular_votes()),
        winners_by(&state.choices, |c| c.channel_points_votes),
    )
}

// Intended to surface a single decisive winner when the overall vote is a tie.
// which during a top tie is a lower-placed option (e.g. third place)
fn get_non_tied_winner(state: &PollStateData) -> Option<&PollChoiceState> {
    state
        .choices
        .iter()
        .filter(|&c| state.choices.iter().filter(|x| x.votes == c.votes).count() == 1)
        .max_by_key(|&c| c.votes)
}

fn get_votes_result(
    state: &Option<&PollStateData>,
    cost: usize,
) -> Element<'static, Message, Theme, Renderer> {
    let Some(state) = *state else {
        return Text::new("No Poll Active").into();
    };
    let total_votes = state.choices.iter().map(|c| c.votes).sum::<i64>();
    let total_popular_votes = state.choices.iter().map(|c| c.popular_votes()).sum::<i64>();
    let total_point_votes = state
        .choices
        .iter()
        .map(|c| c.channel_points_votes)
        .sum::<i64>();

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
            (o.channel_points_votes as f64) / (total_point_votes as f64) * 100.0
        };
        title_col = title_col.push(
            row![
                text("●").color(get_choice_color(&state.choices, &o.id)),
                text(o.title.clone())
            ]
            .spacing(SPACING)
            .align_y(Center),
        );
        votes_col = votes_col.push(text(format!("{} votes, {:.2}%", o.votes, vote_percent)));
        user_col = user_col.push(text(format!(
            "{} votes, {:05.2}%",
            o.popular_votes(),
            pop_vote_percent
        )));
        point_col = point_col.push(text(format!(
            "{} votes, {:.2}% ({} points)",
            o.channel_points_votes,
            point_vote_percent,
            thousand_separator(o.channel_points_votes * cost as i64)
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
                thousand_separator(total_point_votes * cost as i64)
            )),
            grid,
        ]
        .spacing(SPACING),
    )
    .into()
}
#[derive(Debug, Clone)]
pub enum PollMessage {
    BaseFormChange(BaseFormMessage),
    Submit,
    PollCreated(Result<PollStateData, String>),
    ChannelPointsToggled(bool),
    PointCostChange(usize),
    EndPoll,
    PollEnded(Result<PollStateData, String>),
    Config(ConfigMessage),
    LoadSampleData(PollStateData),
    TabSelected(PollBarTabId),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn poll_choice_state(title: String, votes: i64, channel_points_votes: i64) -> PollChoiceState {
        PollChoiceState {
            title,
            votes,
            channel_points_votes,
            ..PollChoiceState::default()
        }
    }
    #[test]
    fn test_winners_by() {
        let choices = [
            poll_choice_state("Loser".to_string(), 1, 1),
            poll_choice_state("Winner".to_string(), 10, 8),
            poll_choice_state("Winner of the hearts".to_string(), 8, 0),
            poll_choice_state("Winner of the money".to_string(), 1, 20),
        ];
        assert_eq!(
            winners_by(&choices, |c| c.votes).first().unwrap().title,
            "Winner"
        );
        assert_eq!(
            winners_by(&choices, |c| c.popular_votes())
                .first()
                .unwrap()
                .title,
            "Winner of the hearts"
        );
        assert_eq!(
            winners_by(&choices, |c| c.channel_points_votes)
                .first()
                .unwrap()
                .title,
            "Winner of the money"
        );
    }

    #[test]
    fn test_winners_by_returns_multiple_winners_on_tie() {
        let choices = [
            poll_choice_state("First".to_string(), 10, 0),
            poll_choice_state("Second".to_string(), 10, 0),
            poll_choice_state("Loser".to_string(), 5, 0),
        ];
        let winners = winners_by(&choices, |c| c.votes);
        assert_eq!(winners.len(), 2);
        assert_eq!(winners[0].title, "First");
        assert_eq!(winners[1].title, "Second");
    }

    #[test]
    fn test_get_non_tied_winner() {
        let state = PollStateData {
            choices: vec![
                poll_choice_state("Tied First A".to_string(), 10, 0),
                poll_choice_state("Tied First B".to_string(), 10, 0),
                poll_choice_state("Third".to_string(), 8, 0),
                poll_choice_state("Loser".to_string(), 5, 0),
            ],
            ..PollStateData::default()
        };
        let non_tied = get_non_tied_winner(&state);
        assert!(non_tied.is_some());
        assert_eq!(non_tied.unwrap().title, "Third");
    }

    #[test]
    fn test_winner_noun() {
        assert_eq!(winner_noun(true, 1), "leader");
        assert_eq!(winner_noun(true, 2), "leaders");
        assert_eq!(winner_noun(false, 1), "winner");
        assert_eq!(winner_noun(false, 2), "winners");
        assert_eq!(winner_noun(true, 0), "leader");
        assert_eq!(winner_noun(false, 0), "winner");
    }
}
