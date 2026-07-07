use crate::{App, Message};
use iced::Task;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Default, Debug)]
pub struct ConfigList {
    pub items: Vec<String>,
    pub selected: Option<String>,
    pub loaded: bool,
}

impl ConfigList {
    pub fn with_list(items: Vec<String>) -> ConfigList {
        ConfigList {
            items,
            selected: None,
            loaded: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConfigMessage {
    Save,
    New,
    NameChanged(String),
    ConfigSelected(String),
}

pub trait Named {
    fn name(&self) -> &str;
    fn set_name(&mut self, name: String) -> ();
}

pub trait ConfigForm {
    type Form: DeserializeOwned + Serialize + Named;
    const SUBDIR: &'static str;
    fn form(&self) -> &Self::Form;
    fn form_mut(&mut self) -> &mut Self::Form;
    fn configs_mut(&mut self) -> &mut ConfigList;
}

pub fn handle_config<T: ConfigForm>(
    config_path: &Path,
    message: ConfigMessage,
    tab: &mut T,
) -> Task<Message> {
    match message {
        ConfigMessage::Save => {
            if let Err(e) = save_config(config_path, T::SUBDIR, tab.form().name(), tab.form()) {
                return Task::done(Message::Error(e.to_string()));
            };
            let name = tab.form().name().to_string();
            let configs = tab.configs_mut();
            configs.items = match App::load_files(config_path.join(T::SUBDIR)) {
                Ok(files) => files,
                Err(e) => return Task::done(Message::Error(e.to_string())),
            };
            configs.loaded = true;
            configs.selected = Some(name);
            Task::none()
        }
        ConfigMessage::New => {
            tab.form_mut().set_name(String::new());
            let configs = tab.configs_mut();
            configs.loaded = false;
            configs.selected = None;
            Task::none()
        }
        ConfigMessage::NameChanged(name) => {
            tab.form_mut().set_name(name);
            Task::none()
        }
        ConfigMessage::ConfigSelected(name) => {
            if let Some(state) = load_config::<T::Form>(config_path, T::SUBDIR, &name) {
                *tab.form_mut() = state;
                let configs = tab.configs_mut();
                configs.selected = Some(name);
                configs.loaded = true;
            }
            Task::none()
        }
    }
}

pub fn save_config<T: Serialize>(
    root: &Path,
    subdir: &str,
    name: &str,
    state: &T,
) -> Result<(), String> {
    let json = serde_json::to_string(state).map_err(|e| e.to_string())?;
    fs::write(root.join(subdir).join(format!("{name}.json")), json).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_config<T: DeserializeOwned>(root: &Path, subdir: &str, name: &str) -> Option<T> {
    fs::read_to_string(root.join(subdir).join(format!("{name}.json")))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
}
