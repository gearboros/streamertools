use crate::config::{save_settings, Settings};
use crate::settings::SettingsMessage::*;
use crate::{App, Message, SPACING};
use iced::alignment::Vertical;
use iced::widget::{column, pick_list, row, Container, PickList, Text};
use iced::{Element, Length, Renderer, Task, Theme};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::string::ToString;
use std::sync::RwLock;

static ACTIVE: RwLock<Separator> = RwLock::new(Separator::DotDecimalCommaThousand);

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    DefaultTab(String),
    LightTheme(String),
    DarkTheme(String),
    NumberFormat(Separator),
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Deserialize, Serialize)]
pub enum Separator {
    #[default]
    DotDecimalCommaThousand,
    CommaDecimalDotThousand,
}

impl Separator {
    const ALL: &'static [Separator] = &[
        Separator::DotDecimalCommaThousand,
        Separator::CommaDecimalDotThousand,
    ];

    pub fn active() -> Separator {
        *ACTIVE.read().unwrap_or_else(|e| e.into_inner())
    }

    pub fn set_active(active: Separator) {
        *ACTIVE.write().unwrap_or_else(|e| e.into_inner()) = active;
    }
}

impl Display for Separator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Separator::DotDecimalCommaThousand => "1,234.56",
            Separator::CommaDecimalDotThousand => "1.234,56",
        };
        write!(f, "{str}")
    }
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

        let i18n_text = Text::new("Number format: ").width(Length::FillPortion(1));
        let i18n_pick: PickList<'_, Separator, &[Separator], Separator, Message> = pick_list(
            Separator::ALL,
            Some(self.settings.separator.unwrap_or_default()),
            |t| Message::Settings(NumberFormat(t)),
        )
        .width(Length::FillPortion(2));
        let i18n_row = row![i18n_text, i18n_pick]
            .spacing(SPACING)
            .align_y(Vertical::Center);

        Container::new(column![tab_row, light_row, dark_row, i18n_row].spacing(SPACING))
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
            NumberFormat(separator) => {
                self.settings.separator = Some(separator);
                Separator::set_active(separator);
                try_save_settings(&self.config_path, &self.settings)
            }
        }
    }
}

pub fn try_save_settings(path_buf: &Path, settings: &Settings) -> Task<Message> {
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
