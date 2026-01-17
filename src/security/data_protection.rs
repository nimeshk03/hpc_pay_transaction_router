use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    pub algorithm: String,
    pub key_rotation_days: u32,
    pub encrypt_at_rest: bool,
    pub encrypt_in_transit: bool,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            algorithm: "AES-256-GCM".to_string(),
            key_rotation_days: 90,
            encrypt_at_rest: true,
            encrypt_in_transit: true,
        }
    }
}

impl EncryptionConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_algorithm(mut self, algorithm: &str) -> Self {
        self.algorithm = algorithm.to_string();
        self
    }

    pub fn with_key_rotation(mut self, days: u32) -> Self {
        self.key_rotation_days = days;
        self
    }
}

pub struct SensitiveDataMasker {
    patterns: HashMap<String, MaskPattern>,
}

#[derive(Debug, Clone)]
pub struct MaskPattern {
    pub name: String,
    pub visible_prefix: usize,
    pub visible_suffix: usize,
    pub mask_char: char,
}

impl MaskPattern {
    pub fn card_number() -> Self {
        Self {
            name: "card_number".to_string(),
            visible_prefix: 4,
            visible_suffix: 4,
            mask_char: '*',
        }
    }

    pub fn email() -> Self {
        Self {
            name: "email".to_string(),
            visible_prefix: 2,
            visible_suffix: 0,
            mask_char: '*',
        }
    }

    pub fn phone() -> Self {
        Self {
            name: "phone".to_string(),
            visible_prefix: 0,
            visible_suffix: 4,
            mask_char: '*',
        }
    }

    pub fn full_mask() -> Self {
        Self {
            name: "full".to_string(),
            visible_prefix: 0,
            visible_suffix: 0,
            mask_char: '*',
        }
    }
}

impl SensitiveDataMasker {
    pub fn new() -> Self {
        let mut patterns = HashMap::new();
        patterns.insert("card_number".to_string(), MaskPattern::card_number());
        patterns.insert("cvv".to_string(), MaskPattern::full_mask());
        patterns.insert("pin".to_string(), MaskPattern::full_mask());
        patterns.insert("password".to_string(), MaskPattern::full_mask());
        patterns.insert("email".to_string(), MaskPattern::email());
        patterns.insert("phone".to_string(), MaskPattern::phone());

        Self { patterns }
    }

    pub fn add_pattern(&mut self, field_name: &str, pattern: MaskPattern) {
        self.patterns.insert(field_name.to_string(), pattern);
    }

    pub fn mask(&self, field_name: &str, value: &str) -> String {
        if let Some(pattern) = self.patterns.get(field_name) {
            self.apply_mask(value, pattern)
        } else {
            value.to_string()
        }
    }

    pub fn mask_with_pattern(&self, value: &str, pattern: &MaskPattern) -> String {
        self.apply_mask(value, pattern)
    }

    fn apply_mask(&self, value: &str, pattern: &MaskPattern) -> String {
        let len = value.len();

        if len <= pattern.visible_prefix + pattern.visible_suffix {
            return pattern.mask_char.to_string().repeat(len);
        }

        let prefix = &value[..pattern.visible_prefix];
        let suffix = &value[len - pattern.visible_suffix..];
        let mask_len = len - pattern.visible_prefix - pattern.visible_suffix;
        let mask = pattern.mask_char.to_string().repeat(mask_len);

        format!("{}{}{}", prefix, mask, suffix)
    }

    pub fn mask_map(&self, data: &HashMap<String, String>) -> HashMap<String, String> {
        data.iter()
            .map(|(k, v)| (k.clone(), self.mask(k, v)))
            .collect()
    }

    pub fn is_sensitive_field(&self, field_name: &str) -> bool {
        self.patterns.contains_key(field_name)
    }
}

impl Default for SensitiveDataMasker {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DataProtector {
    config: EncryptionConfig,
    masker: SensitiveDataMasker,
}

impl DataProtector {
    pub fn new(config: EncryptionConfig) -> Self {
        Self {
            config,
            masker: SensitiveDataMasker::new(),
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(EncryptionConfig::default())
    }

    pub fn mask_sensitive_data(&self, field_name: &str, value: &str) -> String {
        self.masker.mask(field_name, value)
    }

    pub fn mask_map(&self, data: &HashMap<String, String>) -> HashMap<String, String> {
        self.masker.mask_map(data)
    }

    pub fn is_sensitive(&self, field_name: &str) -> bool {
        self.masker.is_sensitive_field(field_name)
    }

    pub fn validate_tls_required(&self, url: &str) -> bool {
        if !self.config.encrypt_in_transit {
            return true;
        }
        url.starts_with("https://")
    }

    pub fn config(&self) -> &EncryptionConfig {
        &self.config
    }

    pub fn add_sensitive_field(&mut self, field_name: &str, pattern: MaskPattern) {
        self.masker.add_pattern(field_name, pattern);
    }
}

impl Clone for DataProtector {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            masker: SensitiveDataMasker::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_pattern_card_number() {
        let masker = SensitiveDataMasker::new();
        let masked = masker.mask("card_number", "4111111111111111");
        assert_eq!(masked, "4111********1111");
    }

    #[test]
    fn test_mask_pattern_cvv() {
        let masker = SensitiveDataMasker::new();
        let masked = masker.mask("cvv", "123");
        assert_eq!(masked, "***");
    }

    #[test]
    fn test_mask_pattern_email() {
        let masker = SensitiveDataMasker::new();
        let masked = masker.mask("email", "user@example.com");
        assert_eq!(masked, "us**************");
    }

    #[test]
    fn test_mask_pattern_phone() {
        let masker = SensitiveDataMasker::new();
        let masked = masker.mask("phone", "1234567890");
        assert_eq!(masked, "******7890");
    }

    #[test]
    fn test_mask_unknown_field() {
        let masker = SensitiveDataMasker::new();
        let masked = masker.mask("unknown_field", "some_value");
        assert_eq!(masked, "some_value");
    }

    #[test]
    fn test_mask_map() {
        let masker = SensitiveDataMasker::new();
        let mut data = HashMap::new();
        data.insert("card_number".to_string(), "4111111111111111".to_string());
        data.insert("amount".to_string(), "100.00".to_string());
        data.insert("cvv".to_string(), "123".to_string());

        let masked = masker.mask_map(&data);

        assert_eq!(masked.get("card_number"), Some(&"4111********1111".to_string()));
        assert_eq!(masked.get("amount"), Some(&"100.00".to_string()));
        assert_eq!(masked.get("cvv"), Some(&"***".to_string()));
    }

    #[test]
    fn test_is_sensitive_field() {
        let masker = SensitiveDataMasker::new();
        assert!(masker.is_sensitive_field("card_number"));
        assert!(masker.is_sensitive_field("cvv"));
        assert!(!masker.is_sensitive_field("amount"));
    }

    #[test]
    fn test_data_protector_tls_validation() {
        let protector = DataProtector::with_default_config();

        assert!(protector.validate_tls_required("https://api.stripe.com"));
        assert!(!protector.validate_tls_required("http://insecure.example.com"));
    }

    #[test]
    fn test_encryption_config() {
        let config = EncryptionConfig::new()
            .with_algorithm("AES-128-GCM")
            .with_key_rotation(30);

        assert_eq!(config.algorithm, "AES-128-GCM");
        assert_eq!(config.key_rotation_days, 30);
    }

    #[test]
    fn test_custom_mask_pattern() {
        let mut masker = SensitiveDataMasker::new();
        masker.add_pattern(
            "account_number",
            MaskPattern {
                name: "account".to_string(),
                visible_prefix: 2,
                visible_suffix: 2,
                mask_char: 'X',
            },
        );

        let masked = masker.mask("account_number", "123456789");
        assert_eq!(masked, "12XXXXX89");
    }

    #[test]
    fn test_short_value_masking() {
        let masker = SensitiveDataMasker::new();
        let masked = masker.mask("card_number", "1234");
        assert_eq!(masked, "****");
    }
}
