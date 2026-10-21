use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub description: String,
    pub quantity: String,
    pub unit: String,
    pub unit_price: String,
    pub vat_rate: i64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Amounts {
    pub net: i64,
    pub vat: i64,
    pub gross: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct VatGroup {
    pub rate: i64,
    #[serde(flatten)]
    pub amounts: Amounts,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Totals {
    #[serde(flatten)]
    pub amounts: Amounts,
    pub groups: Vec<VatGroup>,
    pub lines: Vec<Amounts>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceData {
    pub customer_id: Option<i64>,
    pub customer: Option<Value>,
    pub supplier: Value,
    pub issue_date: String,
    pub delivery_date: String,
    pub due_date: String,
    pub order_number: String,
    pub currency: String,
    pub note: String,
    pub qr_profiles: Vec<String>,
    pub items: Vec<Item>,
    pub totals: Totals,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Invoice {
    #[serde(flatten)]
    pub data: InvoiceData,
    pub id: Option<i64>,
    pub number: Option<String>,
    pub status: String,
    pub version: i64,
    pub client_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrImage {
    pub profile: String,
    pub label: String,
    pub data_uri: String,
}

pub fn string<'a>(data: &'a Value, key: &str) -> &'a str {
    data.get(key).and_then(Value::as_str).unwrap_or("")
}
