use super::transaction::PaymentMethod;
use crate::errors::RouteError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostStructure {
    pub fixed_fee: Decimal,
    pub percentage_fee: Decimal,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limits {
    pub min_amount: Decimal,
    pub max_amount: Decimal,
    pub daily_volume_cap: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub timeout_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    pub psp_id: String,
    pub name: String,
    pub base_url: String,
    pub supported_methods: Vec<PaymentMethod>,
    pub supported_currencies: Vec<String>,
    pub cost_structure: CostStructure,
    pub limits: Limits,
    pub circuit_breaker: CircuitBreakerConfig,
    pub priority: i32,
    pub enabled: bool,
}

impl RouteConfig {
    pub fn new(psp_id: String, name: String, base_url: String) -> Result<Self, RouteError> {
        if psp_id.is_empty() {
            return Err(RouteError::InvalidConfig(
                "PSP ID cannot be empty".to_string(),
            ));
        }

        if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
            return Err(RouteError::InvalidConfig("Invalid base URL".to_string()));
        }

        Ok(Self {
            psp_id,
            name,
            base_url,
            supported_methods: vec![],
            supported_currencies: vec![],
            cost_structure: CostStructure {
                fixed_fee: Decimal::ZERO,
                percentage_fee: Decimal::ZERO,
                currency: "USD".to_string(),
            },
            limits: Limits {
                min_amount: Decimal::ZERO,
                max_amount: Decimal::new(1_000_000, 0),
                daily_volume_cap: Decimal::new(10_000_000, 0),
            },
            circuit_breaker: CircuitBreakerConfig {
                failure_threshold: 5,
                timeout_seconds: 60,
            },
            priority: 0,
            enabled: true,
        })
    }

    pub fn validate(&self) -> Result<(), RouteError> {
        if self.limits.min_amount > self.limits.max_amount {
            return Err(RouteError::InvalidLimits(
                "min_amount cannot exceed max_amount".to_string(),
            ));
        }

        if self.cost_structure.percentage_fee < Decimal::ZERO
            || self.cost_structure.percentage_fee > Decimal::new(100, 0)
        {
            return Err(RouteError::InvalidFeeStructure(
                "Percentage fee must be between 0 and 100".to_string(),
            ));
        }

        if self.supported_methods.is_empty() {
            return Err(RouteError::InvalidConfig(
                "At least one payment method must be supported".to_string(),
            ));
        }

        Ok(())
    }

    pub fn supports_method(&self, method: &PaymentMethod) -> bool {
        self.supported_methods.contains(method)
    }

    pub fn supports_currency(&self, currency: &str) -> bool {
        self.supported_currencies
            .iter()
            .any(|c| c.eq_ignore_ascii_case(currency))
    }
}
