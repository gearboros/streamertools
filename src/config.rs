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
