use serde::{Deserialize, Deserializer};

fn deserialize_optional_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProductFileDto {
    pub file_url: String,
    pub file_name: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateProductDto {
    pub name: String,
    pub sku: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub unit_price: f64,
    pub currency: Option<String>,
    pub file_url: Option<String>,
    pub file_name: Option<String>,
    pub files: Option<Vec<ProductFileDto>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProductDto {
    pub name: Option<String>,
    pub sku: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub unit_price: Option<f64>,
    pub currency: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub file_url: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub file_name: Option<Option<String>>,
    pub files: Option<Vec<ProductFileDto>>,
    pub is_active: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::UpdateProductDto;

    #[test]
    fn update_distinguishes_omitted_and_removed_product_file() {
        let omitted: UpdateProductDto =
            serde_json::from_value(serde_json::json!({})).expect("update should deserialize");
        assert_eq!(omitted.file_url, None);
        assert_eq!(omitted.file_name, None);

        let removed: UpdateProductDto = serde_json::from_value(serde_json::json!({
            "file_url": null,
            "file_name": null
        }))
        .expect("nullable product file should deserialize");
        assert_eq!(removed.file_url, Some(None));
        assert_eq!(removed.file_name, Some(None));
    }

    #[test]
    fn update_distinguishes_omitted_and_empty_product_file_list() {
        let omitted: UpdateProductDto =
            serde_json::from_value(serde_json::json!({})).expect("update should deserialize");
        assert!(omitted.files.is_none());

        let removed: UpdateProductDto = serde_json::from_value(serde_json::json!({
            "files": []
        }))
        .expect("empty file list should deserialize");
        assert_eq!(removed.files.expect("files should be present").len(), 0);
    }
}
