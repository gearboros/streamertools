use crate::{App, Message};
use iced::Task;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::path::{Component, Path};

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
    fn set_name(&mut self, name: String);
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

/// check for valid path name
fn validate_config_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("Config name must not be empty.".to_string());
    }
    if name.contains(['/', '\\']) {
        return Err(format!(
            "Invalid config name '{name}': use a single name without path separators."
        ));
    }
    let mut components = Path::new(name).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(()),
        _ => Err(format!(
            "Invalid config name '{name}': use a single name without path separators."
        )),
    }
}

pub fn save_config<T: Serialize>(
    root: &Path,
    subdir: &str,
    name: &str,
    state: &T,
) -> Result<(), String> {
    validate_config_name(name)?;
    let json = serde_json::to_string(state).map_err(|e| e.to_string())?;
    fs::write(root.join(subdir).join(format!("{name}.json")), json).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_config<T: DeserializeOwned>(root: &Path, subdir: &str, name: &str) -> Option<T> {
    fs::read_to_string(root.join(subdir).join(format!("{name}.json")))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
}

#[cfg(test)]
mod tests {
    use super::validate_config_name;

    #[test]
    fn accepts_plain_names() {
        for name in ["poll", "my-config_2", "name.with.dots", "übung", "a b c"] {
            assert!(validate_config_name(name).is_ok(), "should accept {name:?}");
        }
    }

    #[test]
    fn rejects_empty_and_whitespace_only() {
        for name in ["", " ", "   ", "\t", "\n"] {
            assert!(
                validate_config_name(name).is_err(),
                "should reject {name:?}"
            );
        }
    }

    #[test]
    fn rejects_path_traversal() {
        for name in ["..", "../evil", "..\\evil", "a/../b", "polls/../../etc"] {
            assert!(
                validate_config_name(name).is_err(),
                "should reject {name:?}"
            );
        }
    }

    #[test]
    fn rejects_separators_and_absolute_paths() {
        for name in ["a/b", "a\\b", "/etc/passwd", "C:\\Windows", "/", "sub/"] {
            assert!(
                validate_config_name(name).is_err(),
                "should reject {name:?}"
            );
        }
    }

    #[test]
    fn rejects_current_dir() {
        assert!(validate_config_name(".").is_err());
    }
}
