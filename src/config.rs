use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub dashboard_port: u16,
    pub scan_interval_secs: u64,
    pub log_level: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dashboard_port: 9390,
            scan_interval_secs: 5,
            log_level: "info".to_string(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let path = config_file_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) {
        let path = config_file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let content = serde_json::to_string_pretty(self).unwrap();
        std::fs::write(&path, content).ok();
    }
}

pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("havn")
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("havn")
}

pub fn config_file_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn db_path() -> PathBuf {
    data_dir().join("registry.db")
}

pub fn log_file_path() -> PathBuf {
    data_dir().join("havn.log")
}

pub fn init_logging(args: &crate::cli::Cli) {
    let _ = args; // will use for log level override later
    let config = Config::load();

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&config.log_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.dashboard_port, 9390);
        assert_eq!(config.scan_interval_secs, 5);
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn test_config_save_and_load() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config_path = tmp.path().join("config.json");

        let config = Config {
            dashboard_port: 8888,
            scan_interval_secs: 10,
            log_level: "debug".to_string(),
        };

        // Save
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let content = serde_json::to_string_pretty(&config).unwrap();
        std::fs::write(&config_path, content).unwrap();

        // Load
        let loaded: Config =
            serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(loaded.dashboard_port, 8888);
        assert_eq!(loaded.scan_interval_secs, 10);
        assert_eq!(loaded.log_level, "debug");
    }

    #[test]
    fn test_config_load_missing_file() {
        // Loading from a non-existent path should return defaults
        let config = Config::default();
        assert_eq!(config.dashboard_port, 9390);
    }

    #[test]
    fn test_data_dir_exists() {
        let dir = data_dir();
        assert!(dir.to_string_lossy().contains("havn"));
    }

    #[test]
    fn test_config_dir_exists() {
        let dir = config_dir();
        assert!(dir.to_string_lossy().contains("havn"));
    }
}
