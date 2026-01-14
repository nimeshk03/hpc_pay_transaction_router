use crate::config::loader::{ConfigLoader, RouterConfig};
use crate::config::validator::ConfigValidator;
use crate::errors::ConfigError;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

pub struct ConfigWatcher {
    config_path: PathBuf,
    current_config: Arc<Mutex<RouterConfig>>,
    last_modified: Arc<Mutex<SystemTime>>,
    change_receiver: Receiver<Result<Event, notify::Error>>,
    _watcher: RecommendedWatcher,
}

impl ConfigWatcher {
    pub fn new<P: AsRef<Path>>(config_path: P) -> Result<Self, ConfigError> {
        let config_path = config_path.as_ref().to_path_buf();

        if !config_path.exists() {
            return Err(ConfigError::FileNotFound(
                config_path.display().to_string(),
            ));
        }

        let initial_config = ConfigLoader::load(&config_path)?;
        ConfigValidator::validate(&initial_config)?;

        let (tx, rx): (Sender<Result<Event, notify::Error>>, Receiver<Result<Event, notify::Error>>) = channel();

        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })
        .map_err(|e| ConfigError::WatchError(e.to_string()))?;

        let watch_path = if config_path.is_file() {
            config_path.parent().unwrap_or_else(|| Path::new("."))
        } else {
            &config_path
        };

        watcher
            .watch(watch_path, RecursiveMode::NonRecursive)
            .map_err(|e| ConfigError::WatchError(e.to_string()))?;

        let metadata = std::fs::metadata(&config_path)?;
        let last_modified = metadata
            .modified()
            .unwrap_or_else(|_| SystemTime::now());

        Ok(Self {
            config_path,
            current_config: Arc::new(Mutex::new(initial_config)),
            last_modified: Arc::new(Mutex::new(last_modified)),
            change_receiver: rx,
            _watcher: watcher,
        })
    }

    pub fn has_changes(&self) -> bool {
        while let Ok(event_result) = self.change_receiver.try_recv() {
            if let Ok(event) = event_result {
                if self.is_relevant_event(&event) {
                    return true;
                }
            }
        }
        false
    }

    fn is_relevant_event(&self, event: &Event) -> bool {
        match event.kind {
            EventKind::Modify(_) | EventKind::Create(_) => {
                event.paths.iter().any(|p| p == &self.config_path)
            }
            _ => false,
        }
    }

    pub fn reload(&self) -> Result<RouterConfig, ConfigError> {
        let metadata = std::fs::metadata(&self.config_path)?;
        let modified = metadata
            .modified()
            .unwrap_or_else(|_| SystemTime::now());

        let mut last_modified = self.last_modified.lock().unwrap();

        if modified <= *last_modified {
            return Ok(self.get_current_config());
        }

        std::thread::sleep(Duration::from_millis(100));

        let new_config = ConfigLoader::load(&self.config_path)?;
        ConfigValidator::validate(&new_config)?;

        let mut current = self.current_config.lock().unwrap();
        *current = new_config.clone();
        *last_modified = modified;

        Ok(new_config)
    }

    pub fn get_current_config(&self) -> RouterConfig {
        self.current_config.lock().unwrap().clone()
    }

    pub fn try_reload(&self) -> Result<Option<RouterConfig>, ConfigError> {
        if self.has_changes() {
            Ok(Some(self.reload()?))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::Builder;

    fn create_test_config_json() -> String {
        r#"{
            "version": 1,
            "routes": [
                {
                    "psp_id": "stripe",
                    "name": "Stripe",
                    "base_url": "https://api.stripe.com",
                    "supported_methods": ["Card"],
                    "supported_currencies": ["USD"],
                    "cost_structure": {
                        "fixed_fee": "0.30",
                        "percentage_fee": "2.9",
                        "currency": "USD"
                    },
                    "limits": {
                        "min_amount": "0",
                        "max_amount": "1000000",
                        "daily_volume_cap": "10000000"
                    },
                    "circuit_breaker": {
                        "failure_threshold": 5,
                        "timeout_seconds": 60
                    },
                    "priority": 0,
                    "enabled": true
                }
            ]
        }"#
        .to_string()
    }

    fn create_updated_config_json() -> String {
        r#"{
            "version": 2,
            "routes": [
                {
                    "psp_id": "stripe",
                    "name": "Stripe",
                    "base_url": "https://api.stripe.com",
                    "supported_methods": ["Card"],
                    "supported_currencies": ["USD", "EUR"],
                    "cost_structure": {
                        "fixed_fee": "0.30",
                        "percentage_fee": "2.9",
                        "currency": "USD"
                    },
                    "limits": {
                        "min_amount": "0",
                        "max_amount": "1000000",
                        "daily_volume_cap": "10000000"
                    },
                    "circuit_breaker": {
                        "failure_threshold": 5,
                        "timeout_seconds": 60
                    },
                    "priority": 0,
                    "enabled": true
                }
            ]
        }"#
        .to_string()
    }

    #[test]
    fn test_watcher_initialization() {
        let temp_file = Builder::new().suffix(".json").tempfile().unwrap();
        fs::write(temp_file.path(), create_test_config_json()).unwrap();

        let watcher = ConfigWatcher::new(temp_file.path());
        assert!(watcher.is_ok());

        let watcher = watcher.unwrap();
        let config = watcher.get_current_config();
        assert_eq!(config.version, 1);
        assert_eq!(config.routes.len(), 1);
    }

    #[test]
    fn test_watcher_file_not_found() {
        let result = ConfigWatcher::new("/nonexistent/config.json");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_current_config() {
        let temp_file = Builder::new().suffix(".json").tempfile().unwrap();
        fs::write(temp_file.path(), create_test_config_json()).unwrap();

        let watcher = ConfigWatcher::new(temp_file.path()).unwrap();
        let config = watcher.get_current_config();

        assert_eq!(config.routes[0].psp_id, "stripe");
        assert_eq!(config.routes[0].supported_currencies.len(), 1);
    }

    #[test]
    fn test_reload_config() {
        let temp_file = Builder::new().suffix(".json").tempfile().unwrap();
        fs::write(temp_file.path(), create_test_config_json()).unwrap();

        let watcher = ConfigWatcher::new(temp_file.path()).unwrap();

        std::thread::sleep(Duration::from_millis(100));
        fs::write(temp_file.path(), create_updated_config_json()).unwrap();
        std::thread::sleep(Duration::from_millis(200));

        let new_config = watcher.reload().unwrap();
        assert_eq!(new_config.version, 2);
        assert_eq!(new_config.routes[0].supported_currencies.len(), 2);
    }

    #[test]
    fn test_has_changes_detection() {
        let temp_file = Builder::new().suffix(".json").tempfile().unwrap();
        fs::write(temp_file.path(), create_test_config_json()).unwrap();

        let watcher = ConfigWatcher::new(temp_file.path()).unwrap();

        std::thread::sleep(Duration::from_millis(100));
        fs::write(temp_file.path(), create_updated_config_json()).unwrap();
        std::thread::sleep(Duration::from_millis(200));

        assert!(watcher.has_changes());
    }
}
