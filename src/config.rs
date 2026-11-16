use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: String,
    pub show_hidden: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "tokyonight".to_string(),
            show_hidden: false,
        }
    }
}

impl Config {
    pub fn path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| home_dir().map(|home| home.join(".config")))?;
        Some(base.join("box2cell").join("config.toml"))
    }

    pub fn load() -> (Self, bool) {
        let Some(path) = Self::path() else {
            return (Self::default(), false);
        };
        match fs::read_to_string(&path) {
            Ok(text) => (toml::from_str(&text).unwrap_or_default(), true),
            Err(_) => (Self::default(), false),
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = Self::path() else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self).unwrap_or_default();
        fs::write(path, text)
    }
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}
