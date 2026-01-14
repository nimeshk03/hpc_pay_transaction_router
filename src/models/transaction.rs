use crate::errors::TransactionError;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PaymentMethod {
    Card,
    BankTransfer,
    Wallet,
    UPI,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRequest {
    pub id: Uuid,
    pub merchant_id: String,
    pub amount: Decimal,
    pub currency: String,
    pub payment_method: PaymentMethod,
    pub card_token: Option<String>,
    pub metadata: std::collections::HashMap<String, String>,
    pub priority: Priority,
    pub timeout_ms: u32,
    pub idempotency_key: String,
    pub created_at: DateTime<Utc>,
}

impl TransactionRequest {
    pub fn new(
        merchant_id: String,
        amount: Decimal,
        currency: String,
        payment_method: PaymentMethod,
    ) -> Result<Self, TransactionError> {
        if merchant_id.is_empty() {
            return Err(TransactionError::MissingField("merchant_id".to_string()));
        }

        if amount <= Decimal::ZERO {
            return Err(TransactionError::InvalidAmount(
                "Amount must be positive".to_string(),
            ));
        }

        if currency.len() != 3 {
            return Err(TransactionError::InvalidCurrency(
                "Currency must be 3-letter ISO code".to_string(),
            ));
        }

        Ok(Self {
            id: Uuid::new_v4(),
            merchant_id,
            amount,
            currency: currency.to_uppercase(),
            payment_method,
            card_token: None,
            metadata: std::collections::HashMap::new(),
            priority: Priority::Normal,
            timeout_ms: 5000,
            idempotency_key: Uuid::new_v4().to_string(),
            created_at: Utc::now(),
        })
    }

    pub fn is_valid(&self) -> bool {
        !self.merchant_id.is_empty() && self.amount > Decimal::ZERO && self.currency.len() == 3
    }

    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_card_token(mut self, token: String) -> Self {
        self.card_token = Some(token);
        self
    }

    pub fn with_timeout(mut self, timeout_ms: u32) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
}
