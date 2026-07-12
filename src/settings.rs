use crate::config::{Settings, save_settings};
use crate::settings::SettingsMessage::*;
use crate::{App, Message, SPACING};
use iced::alignment::Vertical;
use iced::widget::{Container, PickList, Text, column, pick_list, row};
use iced::{Element, Length, Renderer, Task, Theme};
use std::path::PathBuf;
use std::string::ToString;

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    DefaultTab(String),
    LightTheme(String),
    DarkTheme(String),
}

const LIGHT_THEMES: &[Theme] = &[
    Theme::Light,
    Theme::SolarizedLight,
    Theme::GruvboxLight,
    Theme::CatppuccinLatte,
    Theme::KanagawaLotus,
    Theme::TokyoNightLight,
];
const DARK_THEMES: &[Theme] = &[
    Theme::Dark,
    Theme::Dracula,
    Theme::Nord,
    Theme::SolarizedDark,
    Theme::GruvboxDark,
    Theme::CatppuccinFrappe,
    Theme::CatppuccinMacchiato,
    Theme::CatppuccinMocha,
    Theme::TokyoNight,
    Theme::TokyoNightStorm,
    Theme::KanagawaWave,
    Theme::KanagawaDragon,
    Theme::Moonfly,
    Theme::Nightfly,
    Theme::Oxocarbon,
    Theme::Ferra,
];

impl App {
    pub fn get_settings_tab_content(&self) -> Element<'_, Message, Theme, Renderer> {
        let default_tab_text = Text::new("Default Tab: ").width(Length::FillPortion(1));
        let default_tab_pick: PickList<'_, String, Vec<String>, String, Message> = pick_list(
            vec!["Prediction".to_string(), "Poll".to_string()],
            self.settings.default_tab.clone(),
            |t| Message::Settings(DefaultTab(t)),
        )
        .width(Length::FillPortion(2))
        .placeholder("Select tab");
        let tab_row = row![default_tab_text, default_tab_pick]
            .spacing(SPACING)
            .align_y(Vertical::Center);

        let light_text = Text::new("Light Mode: ").width(Length::FillPortion(1));
        let light_pick: PickList<'_, Theme, &[Theme], Theme, Message> = pick_list(
            LIGHT_THEMES,
            Some(resolve_theme(&self.settings.light_theme, Theme::Light)),
            |t| Message::Settings(LightTheme(t.to_string())),
        )
        .width(Length::FillPortion(2));
        let light_row = row![light_text, light_pick]
            .spacing(SPACING)
            .align_y(Vertical::Center);

        let dark_text = Text::new("Dark Mode: ").width(Length::FillPortion(1));
        let dark_pick: PickList<'_, Theme, &[Theme], Theme, Message> = pick_list(
            DARK_THEMES,
            Some(resolve_theme(&self.settings.dark_theme, Theme::Dark)),
            |t| Message::Settings(DarkTheme(t.to_string())),
        )
        .width(Length::FillPortion(2));
        let dark_row = row![dark_text, dark_pick]
            .spacing(SPACING)
            .align_y(Vertical::Center);

        Container::new(column![tab_row, light_row, dark_row].spacing(SPACING))
            .max_width(500)
            .height(Length::Fill)
            .into()
    }

    pub fn handle_settings(&mut self, message: SettingsMessage) -> Task<Message> {
        match message {
            DefaultTab(tab) => {
                self.settings.default_tab = Some(tab);
                try_save_settings(&self.config_path, &self.settings)
            }
            LightTheme(theme) => {
                self.settings.light_theme = Some(theme);
                try_save_settings(&self.config_path, &self.settings)
            }
            DarkTheme(theme) => {
                self.settings.dark_theme = Some(theme);
                try_save_settings(&self.config_path, &self.settings)
            }
        }
    }
}

pub fn try_save_settings(path_buf: &PathBuf, settings: &Settings) -> Task<Message> {
    match save_settings(path_buf, settings) {
        Ok(_) => Task::none(),
        Err(err) => Task::done(Message::Error(err)),
    }
}

pub fn resolve_theme(name: &Option<String>, default: Theme) -> Theme {
    name.as_deref()
        .and_then(|n| Theme::ALL.iter().find(|t| t.to_string() == n).cloned())
        .unwrap_or(default)
}
