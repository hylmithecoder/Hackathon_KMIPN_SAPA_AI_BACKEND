//! Helpers for WhatsApp message formatting and phone normalization.

/// Strip non-digits and return a clean international number if possible.
/// Accepts inputs like `08123456789`, `+62 812-3456-7890`, `628123456789`.
pub fn normalize_phone(phone: &str) -> Option<String> {
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    if let Some(stripped) = digits.strip_prefix('0') {
        // Indonesian local mobile format: 08xxx -> 628xxx
        Some(format!("62{stripped}"))
    } else {
        Some(digits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_indonesian_local() {
        assert_eq!(
            normalize_phone("081234567890"),
            Some("6281234567890".to_string())
        );
    }

    #[test]
    fn normalize_international() {
        assert_eq!(
            normalize_phone("+62 812-3456-7890"),
            Some("6281234567890".to_string())
        );
    }

    #[test]
    fn normalize_already_clean() {
        assert_eq!(
            normalize_phone("6281234567890"),
            Some("6281234567890".to_string())
        );
    }

    #[test]
    fn normalize_empty() {
        assert_eq!(normalize_phone("abc"), None);
    }
}
