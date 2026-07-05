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
