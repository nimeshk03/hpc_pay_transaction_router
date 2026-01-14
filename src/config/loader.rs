use crate::errors::ConfigError;
use crate::models::RouteConfig;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct RouterConfig {
    pub routes: Vec<RouteConfig>,
    pub version: u32,
}

impl RouterConfig {
    pub fn new() -> Self {
        Self {
            routes: Vec::new(),
            version: 1,
        }
    }

    pub fn with_routes(mut self, routes: Vec<RouteConfig>) -> Self {
        self.routes = routes;
        self
    }

    pub fn add_route(&mut self, route: RouteConfig) {
        self.routes.push(route);
    }
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<RouterConfig, ConfigError> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(ConfigError::FileNotFound(
                path.display().to_string(),
            ));
        }

        let content = fs::read_to_string(path)?;

        Self::parse(&content, path)
    }

    pub fn parse(content: &str, path: &Path) -> Result<RouterConfig, ConfigError> {
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        match extension {
            "json" => Self::parse_json(content),
            "yaml" | "yml" => Self::parse_yaml(content),
            _ => Err(ConfigError::ValidationError(
                format!("Unsupported file format: {}", extension),
            )),
        }
    }

    pub fn parse_json(content: &str) -> Result<RouterConfig, ConfigError> {
        let config: RouterConfig = serde_json::from_str(content)?;
        Ok(config)
    }

    pub fn parse_yaml(content: &str) -> Result<RouterConfig, ConfigError> {
        let config: RouterConfig = serde_yaml::from_str(content)?;
        Ok(config)
    }

    pub fn save<P: AsRef<Path>>(config: &RouterConfig, path: P) -> Result<(), ConfigError> {
        let path = path.as_ref();
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        let content = match extension {
            "json" => serde_json::to_string_pretty(config)
                .map_err(|e| ConfigError::JsonParseError(e))?,
            "yaml" | "yml" => serde_yaml::to_string(config)
                .map_err(|e| ConfigError::YamlParseError(e))?,
            _ => {
                return Err(ConfigError::ValidationError(
                    format!("Unsupported file format: {}", extension),
                ))
            }
        };

        fs::write(path, content)?;
        Ok(())
    }
}

impl serde::Serialize for RouterConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("RouterConfig", 2)?;
        state.serialize_field("version", &self.version)?;
        state.serialize_field("routes", &self.routes)?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for RouterConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct RouterConfigVisitor;

        impl<'de> Visitor<'de> for RouterConfigVisitor {
            type Value = RouterConfig;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct RouterConfig")
            }

            fn visit_map<V>(self, mut map: V) -> Result<RouterConfig, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut version = None;
                let mut routes = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "version" => {
                            if version.is_some() {
                                return Err(de::Error::duplicate_field("version"));
                            }
                            version = Some(map.next_value()?);
                        }
                        "routes" => {
                            if routes.is_some() {
                                return Err(de::Error::duplicate_field("routes"));
                            }
                            routes = Some(map.next_value()?);
                        }
                        _ => {
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                let version = version.unwrap_or(1);
                let routes = routes.ok_or_else(|| de::Error::missing_field("routes"))?;

                Ok(RouterConfig { version, routes })
            }
        }

        deserializer.deserialize_struct(
            "RouterConfig",
            &["version", "routes"],
            RouterConfigVisitor,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PaymentMethod;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn create_test_route() -> RouteConfig {
        let mut route = RouteConfig::new(
            "stripe".to_string(),
            "Stripe".to_string(),
            "https://api.stripe.com".to_string(),
        )
        .unwrap();

        route.supported_methods = vec![PaymentMethod::Card];
        route.supported_currencies = vec!["USD".to_string()];
        route.cost_structure.fixed_fee = Decimal::from_str("0.30").unwrap();
        route.cost_structure.percentage_fee = Decimal::from_str("2.9").unwrap();

        route
    }

    #[test]
    fn test_parse_json() {
        let json = r#"{
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
        }"#;

        let config = ConfigLoader::parse_json(json).unwrap();
        assert_eq!(config.version, 1);
        assert_eq!(config.routes.len(), 1);
        assert_eq!(config.routes[0].psp_id, "stripe");
    }

    #[test]
    fn test_parse_yaml() {
        let yaml = r#"
version: 1
routes:
  - psp_id: stripe
    name: Stripe
    base_url: https://api.stripe.com
    supported_methods:
      - Card
    supported_currencies:
      - USD
    cost_structure:
      fixed_fee: "0.30"
      percentage_fee: "2.9"
      currency: USD
    limits:
      min_amount: "0"
      max_amount: "1000000"
      daily_volume_cap: "10000000"
    circuit_breaker:
      failure_threshold: 5
      timeout_seconds: 60
    priority: 0
    enabled: true
"#;

        let config = ConfigLoader::parse_yaml(yaml).unwrap();
        assert_eq!(config.version, 1);
        assert_eq!(config.routes.len(), 1);
        assert_eq!(config.routes[0].psp_id, "stripe");
    }

    #[test]
    fn test_invalid_json() {
        let invalid_json = r#"{ invalid json }"#;
        let result = ConfigLoader::parse_json(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_router_config_builder() {
        let route = create_test_route();
        let config = RouterConfig::new().with_routes(vec![route]);

        assert_eq!(config.routes.len(), 1);
        assert_eq!(config.version, 1);
    }
}
