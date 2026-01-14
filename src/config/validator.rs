use crate::config::loader::RouterConfig;
use crate::errors::{ConfigError, RouteError};
use std::collections::HashSet;

pub struct ConfigValidator;

impl ConfigValidator {
    pub fn validate(config: &RouterConfig) -> Result<(), ConfigError> {
        if config.routes.is_empty() {
            return Err(ConfigError::ValidationError(
                "Configuration must contain at least one route".to_string(),
            ));
        }

        Self::validate_unique_psp_ids(config)?;
        Self::validate_routes(config)?;
        Self::validate_priorities(config)?;

        Ok(())
    }

    fn validate_unique_psp_ids(config: &RouterConfig) -> Result<(), ConfigError> {
        let mut seen = HashSet::new();

        for route in &config.routes {
            if !seen.insert(&route.psp_id) {
                return Err(ConfigError::ValidationError(format!(
                    "Duplicate PSP ID found: {}",
                    route.psp_id
                )));
            }
        }

        Ok(())
    }

    fn validate_routes(config: &RouterConfig) -> Result<(), ConfigError> {
        for route in &config.routes {
            route.validate().map_err(|e| match e {
                RouteError::InvalidConfig(msg) => ConfigError::ValidationError(format!(
                    "Route '{}': {}",
                    route.psp_id, msg
                )),
                RouteError::InvalidFeeStructure(msg) => ConfigError::ValidationError(format!(
                    "Route '{}': {}",
                    route.psp_id, msg
                )),
                RouteError::InvalidLimits(msg) => ConfigError::ValidationError(format!(
                    "Route '{}': {}",
                    route.psp_id, msg
                )),
            })?;
        }

        Ok(())
    }

    fn validate_priorities(config: &RouterConfig) -> Result<(), ConfigError> {
        let enabled_routes: Vec<_> = config
            .routes
            .iter()
            .filter(|r| r.enabled)
            .collect();

        if enabled_routes.is_empty() {
            return Err(ConfigError::ValidationError(
                "At least one route must be enabled".to_string(),
            ));
        }

        Ok(())
    }

    pub fn validate_and_load(config: RouterConfig) -> Result<RouterConfig, ConfigError> {
        Self::validate(&config)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{PaymentMethod, RouteConfig};
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn create_valid_route(psp_id: &str) -> RouteConfig {
        let mut route = RouteConfig::new(
            psp_id.to_string(),
            format!("{} PSP", psp_id),
            format!("https://api.{}.com", psp_id),
        )
        .unwrap();

        route.supported_methods = vec![PaymentMethod::Card];
        route.supported_currencies = vec!["USD".to_string()];
        route
    }

    #[test]
    fn test_validate_valid_config() {
        let config = RouterConfig::new().with_routes(vec![
            create_valid_route("stripe"),
            create_valid_route("adyen"),
        ]);

        let result = ConfigValidator::validate(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_empty_routes() {
        let config = RouterConfig::new();
        let result = ConfigValidator::validate(&config);

        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::ValidationError(msg) => {
                assert!(msg.contains("at least one route"));
            }
            _ => panic!("Expected ValidationError"),
        }
    }

    #[test]
    fn test_validate_duplicate_psp_ids() {
        let config = RouterConfig::new().with_routes(vec![
            create_valid_route("stripe"),
            create_valid_route("stripe"),
        ]);

        let result = ConfigValidator::validate(&config);
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::ValidationError(msg) => {
                assert!(msg.contains("Duplicate PSP ID"));
            }
            _ => panic!("Expected ValidationError"),
        }
    }

    #[test]
    fn test_validate_invalid_route_limits() {
        let mut route = create_valid_route("stripe");
        route.limits.min_amount = Decimal::from_str("1000.00").unwrap();
        route.limits.max_amount = Decimal::from_str("100.00").unwrap();

        let config = RouterConfig::new().with_routes(vec![route]);
        let result = ConfigValidator::validate(&config);

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_no_enabled_routes() {
        let mut route = create_valid_route("stripe");
        route.enabled = false;

        let config = RouterConfig::new().with_routes(vec![route]);
        let result = ConfigValidator::validate(&config);

        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::ValidationError(msg) => {
                assert!(msg.to_lowercase().contains("at least one route must be enabled"));
            }
            _ => panic!("Expected ValidationError"),
        }
    }

    #[test]
    fn test_validate_and_load() {
        let config = RouterConfig::new().with_routes(vec![create_valid_route("stripe")]);

        let result = ConfigValidator::validate_and_load(config);
        assert!(result.is_ok());
    }
}
