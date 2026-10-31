use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// What the model returns for one receipt, and the format of the eval set's ground truth.
/// Amounts are decimal strings as printed (`"2.50"`, `"-0.08"`), never JSON numbers, so they
/// reach Rust without passing through a float.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelReceipt {
    pub merchant: Option<String>,
    /// `YYYY-MM-DD`.
    pub date: Option<String>,
    /// ISO 4217.
    pub currency: Option<String>,
    pub items: Vec<ModelItem>,
    /// The amount paid.
    pub total: Option<String>,
}

/// One printed line: a product, a discount (negative) or a charge added on top (tip, service).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelItem {
    pub description: String,
    pub quantity: Option<String>,
    pub unit_price: Option<String>,
    pub total: String,
}

/// JSON Schema for [`ModelReceipt`], in the subset strict structured-output modes accept
/// (every property required, nullable via a type union, no extra properties).
#[must_use]
pub fn json_schema() -> Value {
    let nullable =
        |description: &str| json!({ "type": ["string", "null"], "description": description });
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["merchant", "date", "currency", "items", "total"],
        "properties": {
            "merchant": nullable("Shop or restaurant name as printed, or null if not visible."),
            "date": nullable("Purchase date as YYYY-MM-DD, or null if not visible."),
            "currency": nullable("ISO 4217 code such as EUR or USD, or null if unclear."),
            "items": {
                "type": "array",
                "description": "Every printed line in order: products, discounts (negative) and charges added on top.",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["description", "quantity", "unit_price", "total"],
                    "properties": {
                        "description": { "type": "string", "description": "As printed." },
                        "quantity": nullable("Units or weight as a decimal string, null if not printed."),
                        "unit_price": nullable("Price per unit as a decimal string, null if not printed."),
                        "total": {
                            "type": "string",
                            "description": "Line amount as a decimal string with a dot, e.g. \"2.50\" or \"-0.08\"."
                        }
                    }
                }
            },
            "total": nullable("Amount paid as a decimal string with a dot, e.g. \"44.16\".")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_lists_every_field() {
        let receipt = ModelReceipt {
            merchant: None,
            date: None,
            currency: None,
            items: vec![ModelItem {
                description: String::new(),
                quantity: None,
                unit_price: None,
                total: String::new(),
            }],
            total: None,
        };
        let value = serde_json::to_value(&receipt).expect("serializes");
        let schema = json_schema();
        let keys = |v: &Value| {
            let mut k: Vec<String> = v.as_object().expect("object").keys().cloned().collect();
            k.sort();
            k
        };
        let required = |v: &Value| {
            let mut k: Vec<String> = v["required"]
                .as_array()
                .expect("array")
                .iter()
                .map(|s| s.as_str().expect("str").to_owned())
                .collect();
            k.sort();
            k
        };
        assert_eq!(keys(&value), keys(&schema["properties"]));
        assert_eq!(keys(&value), required(&schema));
        let item_schema = &schema["properties"]["items"]["items"];
        assert_eq!(keys(&value["items"][0]), keys(&item_schema["properties"]));
        assert_eq!(keys(&value["items"][0]), required(item_schema));
    }

    #[test]
    fn missing_optional_fields_read_as_null() {
        let r: ModelReceipt =
            serde_json::from_str(r#"{"items":[{"description":"A","total":"1.00"}]}"#)
                .expect("parses");
        assert_eq!(r.total, None);
        assert_eq!(r.items[0].quantity, None);
    }
}
