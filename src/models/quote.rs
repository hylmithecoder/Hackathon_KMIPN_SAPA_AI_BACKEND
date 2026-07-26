use serde::{Deserialize, Deserializer};

fn deserialize_optional_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(Debug, Deserialize)]
pub struct CreateQuoteDto {
    pub deal_id: u64,
    pub quote_number: String,
    pub issue_date: String,
    pub expiry_date: Option<String>,
    pub tax_rate: Option<f64>,
    pub currency: Option<String>,
    pub notes: Option<String>,
    pub items: Vec<CreateQuoteItemDto>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateQuoteDto {
    pub quote_number: Option<String>,
    pub issue_date: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub expiry_date: Option<Option<String>>,
    pub tax_rate: Option<f64>,
    pub currency: Option<String>,
    pub status: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub notes: Option<Option<String>>,
    pub items: Option<Vec<CreateQuoteItemDto>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateQuoteItemDto {
    pub product_id: Option<u64>,
    pub description: String,
    pub quantity: f64,
    pub unit_price: f64,
    pub discount: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateQuoteItemDto {
    pub product_id: Option<u64>,
    pub description: Option<String>,
    pub quantity: Option<f64>,
    pub unit_price: Option<f64>,
    pub discount: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct QuoteStatusDto {
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::UpdateQuoteDto;

    #[test]
    fn update_distinguishes_omitted_and_explicitly_null_fields() {
        let omitted: UpdateQuoteDto =
            serde_json::from_value(serde_json::json!({})).expect("empty update should deserialize");
        assert_eq!(omitted.expiry_date, None);
        assert_eq!(omitted.notes, None);

        let cleared: UpdateQuoteDto = serde_json::from_value(serde_json::json!({
            "expiry_date": null,
            "notes": null
        }))
        .expect("nullable update should deserialize");
        assert_eq!(cleared.expiry_date, Some(None));
        assert_eq!(cleared.notes, Some(None));
    }
}
