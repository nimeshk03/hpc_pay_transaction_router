use std::fs;
use std::thread;
use std::time::Duration;
use tempfile::Builder;
use transaction_router::{ConfigLoader, ConfigValidator, ConfigWatcher};

fn create_initial_config() -> String {
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

fn create_updated_config() -> String {
    r#"{
        "version": 2,
        "routes": [
            {
                "psp_id": "stripe",
                "name": "Stripe",
                "base_url": "https://api.stripe.com",
                "supported_methods": ["Card", "Wallet"],
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
            },
            {
                "psp_id": "adyen",
                "name": "Adyen",
                "base_url": "https://api.adyen.com",
                "supported_methods": ["Card"],
                "supported_currencies": ["USD"],
                "cost_structure": {
                    "fixed_fee": "0.10",
                    "percentage_fee": "2.5",
                    "currency": "USD"
                },
                "limits": {
                    "min_amount": "0",
                    "max_amount": "500000",
                    "daily_volume_cap": "5000000"
                },
                "circuit_breaker": {
                    "failure_threshold": 10,
                    "timeout_seconds": 120
                },
                "priority": 1,
                "enabled": true
            }
        ]
    }"#
    .to_string()
}

#[test]
fn test_config_hot_reload() {
    let temp_file = Builder::new().suffix(".json").tempfile().unwrap();
    fs::write(temp_file.path(), create_initial_config()).unwrap();

    let watcher = ConfigWatcher::new(temp_file.path()).unwrap();
    let initial_config = watcher.get_current_config();

    assert_eq!(initial_config.version, 1);
    assert_eq!(initial_config.routes.len(), 1);
    assert_eq!(initial_config.routes[0].psp_id, "stripe");

    thread::sleep(Duration::from_millis(100));
    fs::write(temp_file.path(), create_updated_config()).unwrap();
    thread::sleep(Duration::from_millis(300));

    let updated_config = watcher.reload().unwrap();

    assert_eq!(updated_config.version, 2);
    assert_eq!(updated_config.routes.len(), 2);
    assert_eq!(updated_config.routes[0].supported_methods.len(), 2);
    assert_eq!(updated_config.routes[1].psp_id, "adyen");
}

#[test]
fn test_config_validation_on_reload() {
    let temp_file = Builder::new().suffix(".json").tempfile().unwrap();
    fs::write(temp_file.path(), create_initial_config()).unwrap();

    let watcher = ConfigWatcher::new(temp_file.path()).unwrap();

    let invalid_config = r#"{
        "version": 2,
        "routes": []
    }"#;

    thread::sleep(Duration::from_millis(100));
    fs::write(temp_file.path(), invalid_config).unwrap();
    thread::sleep(Duration::from_millis(300));

    let result = watcher.reload();
    assert!(result.is_err());

    let current = watcher.get_current_config();
    assert_eq!(current.version, 1);
}

#[test]
fn test_load_json_config_file() {
    let temp_file = Builder::new().suffix(".json").tempfile().unwrap();
    fs::write(temp_file.path(), create_initial_config()).unwrap();

    let config = ConfigLoader::load(temp_file.path()).unwrap();
    assert_eq!(config.routes.len(), 1);
    assert_eq!(config.routes[0].psp_id, "stripe");
}

#[test]
fn test_validate_loaded_config() {
    let temp_file = Builder::new().suffix(".json").tempfile().unwrap();
    fs::write(temp_file.path(), create_initial_config()).unwrap();

    let config = ConfigLoader::load(temp_file.path()).unwrap();
    let result = ConfigValidator::validate(&config);
    assert!(result.is_ok());
}
