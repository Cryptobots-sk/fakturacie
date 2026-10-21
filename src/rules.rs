use crate::{
    error::{Problem, Result},
    model::{Amounts, InvoiceData, Item, Totals, VatGroup},
};
use chrono::NaiveDate;
use regex::Regex;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const QR_LABELS: [(&str, &str); 3] = [
    ("sk", "PAY by square · Slovensko"),
    ("cz", "QR Platba · Česko"),
    ("epc", "SEPA QR · Európa"),
];
pub fn label(profile: &str) -> Option<&'static str> {
    QR_LABELS
        .iter()
        .find_map(|(key, label)| (*key == profile).then_some(*label))
}
pub fn text(
    value: &Value,
    max: usize,
    label: &str,
    required: bool,
    multiline: bool,
) -> Result<String> {
    let value = value
        .as_str()
        .ok_or_else(|| Problem::new(format!("Pole „{label}“ musí obsahovať text.")))?
        .trim();
    if value.chars().count() > max
        || value
            .chars()
            .any(|c| (c <= '\u{1f}' || c == '\u{7f}') && !(multiline && (c == '\n' || c == '\t')))
    {
        return Err(Problem::new(format!(
            "Skontroluj pole „{label}“ (najviac {max} znakov, bez riadiacich znakov)."
        )));
    }
    if required && value.is_empty() {
        return Err(Problem::new(format!("Vyplň pole „{label}“.")));
    }
    Ok(value.to_owned())
}
fn field_text(input: &Value, key: &str, max: usize, label: &str, required: bool) -> Result<String> {
    text(
        input.get(key).unwrap_or(&json!("")),
        max,
        label,
        required,
        false,
    )
}
pub fn integer(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64(),
        Value::String(s) if !s.is_empty() && !s.starts_with('0') || s == "0" => s.parse().ok(),
        _ => None,
    }
}
pub fn positive_id(input: &Value, key: &str) -> Result<i64> {
    input
        .get(key)
        .and_then(Value::as_i64)
        .filter(|n| *n > 0)
        .ok_or_else(|| Problem::new("Chýba platný identifikátor alebo verzia."))
}
pub fn valid_email(email: &str) -> bool {
    email.is_ascii()
        && email.len() <= 160
        && !email.contains(char::is_whitespace)
        && email.parse::<lettre::Address>().is_ok()
}
pub fn email(value: &Value, label: &str, required: bool) -> Result<String> {
    let value = text(value, 160, label, required, false)?;
    if !value.is_empty() && !valid_email(&value) {
        return Err(Problem::new(format!(
            "Skontroluj e-mailovú adresu „{label}“."
        )));
    }
    Ok(value)
}
pub fn party(input: &Value, supplier: bool) -> Result<Value> {
    let mut result = json!({});
    for (key, max, label, required) in [
        ("name", 70, "Názov firmy", true),
        ("street", 120, "Ulica a číslo", true),
        ("zip", 20, "PSČ", true),
        ("city", 80, "Mesto", true),
        ("country", 70, "Krajina", true),
        ("ico", 30, "IČO", false),
        ("dic", 30, "DIČ", false),
        ("vatId", 30, "IČ DPH", false),
        ("email", 160, "E-mail", false),
    ] {
        result[key] = field_text(input, key, max, label, required)?.into();
    }
    if !result["email"].as_str().unwrap_or("").is_empty()
        && !valid_email(result["email"].as_str().unwrap_or(""))
    {
        return Err(Problem::new("Zadaj platnú e-mailovú adresu."));
    }
    let days = integer(input.get("paymentDays").unwrap_or(&json!(14)))
        .filter(|n| (0..=365).contains(n))
        .ok_or_else(|| Problem::new("Splatnosť musí byť od 0 do 365 dní."))?;
    result["paymentDays"] = days.into();
    let profile = match input.get("qrProfile") {
        None | Some(Value::Null) => "sk",
        Some(Value::String(profile)) => profile.as_str(),
        _ => return Err(Problem::new("Vyber platný typ QR kódu.")),
    };
    if label(profile).is_none() {
        return Err(Problem::new("Vyber platný typ QR kódu."));
    }
    result["qrProfile"] = profile.into();
    if supplier {
        let iban = field_text(input, "iban", 42, "IBAN", true)?
            .replace(' ', "")
            .to_uppercase();
        if !valid_iban(&iban) {
            return Err(Problem::new("IBAN nie je platný. Skontroluj číslo účtu."));
        }
        let bic = field_text(input, "bic", 11, "BIC", false)?.to_uppercase();
        if !bic.is_empty()
            && !Regex::new(r"^[A-Z]{6}[A-Z0-9]{2}([A-Z0-9]{3})?$")
                .expect("static regex")
                .is_match(&bic)
        {
            return Err(Problem::new("BIC musí mať 8 alebo 11 znakov."));
        }
        result["iban"] = iban.into();
        result["bic"] = bic.into();
        result["registry"] = field_text(input, "registry", 300, "Zápis v registri", false)?.into();
    }
    Ok(result)
}
pub fn valid_iban(iban: &str) -> bool {
    if !Regex::new(r"^[A-Z]{2}[0-9]{2}[A-Z0-9]{11,30}$")
        .expect("static regex")
        .is_match(iban)
    {
        return false;
    }
    let lengths = [
        ("SK", 24),
        ("CZ", 24),
        ("DE", 22),
        ("AT", 20),
        ("BE", 16),
        ("FR", 27),
        ("NL", 18),
        ("IE", 22),
        ("IT", 27),
        ("ES", 24),
        ("PT", 25),
        ("PL", 28),
        ("HU", 28),
        ("RO", 24),
        ("BG", 22),
        ("HR", 21),
        ("SI", 19),
        ("EE", 20),
        ("LV", 21),
        ("LT", 20),
        ("FI", 18),
        ("SE", 24),
        ("DK", 18),
        ("LU", 20),
        ("MT", 31),
        ("CY", 28),
        ("GR", 27),
        ("GB", 22),
        ("CH", 21),
        ("NO", 15),
        ("IS", 26),
        ("LI", 21),
    ];
    if lengths
        .iter()
        .any(|(country, len)| iban.starts_with(country) && iban.len() != *len)
    {
        return false;
    }
    let mut remainder = 0_u32;
    for byte in iban[4..].bytes().chain(iban[..4].bytes()) {
        remainder = if byte.is_ascii_digit() {
            (remainder * 10 + u32::from(byte - b'0')) % 97
        } else {
            (remainder * 100 + u32::from(byte - b'A') + 10) % 97
        };
    }
    remainder == 1
}
pub fn date(value: &Value, label: &str, required: bool) -> Result<String> {
    if !required && value.as_str() == Some("") {
        return Ok(String::new());
    }
    let value = value
        .as_str()
        .ok_or_else(|| Problem::new(format!("Skontroluj dátum „{label}“.")))?;
    let valid = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .is_ok_and(|d| d.format("%Y-%m-%d").to_string() == value);
    if !valid || !("2000-01-01"..="2099-12-31").contains(&value) {
        return Err(Problem::new(format!("Skontroluj dátum „{label}“.")));
    }
    Ok(value.to_owned())
}
/// Parse decimal input into fixed units. Never route money through a binary float.
pub fn decimal(value: &Value, scale: u32, label: &str, max_units: i64) -> Result<(String, i64)> {
    let raw = match value {
        Value::String(s) => s.clone(),
        Value::Number(n) if n.is_i64() => n.to_string(),
        _ => {
            return Err(Problem::new(format!(
                "Pole „{label}“ musí obsahovať číslo."
            )));
        }
    };
    let cleaned = raw.trim().replace([' ', '\u{a0}'], "").replace(',', ".");
    let cleaned = if cleaned.is_empty() { "0" } else { &cleaned };
    let invalid = || {
        Problem::new(format!(
            "Skontroluj pole „{label}“ (kladné číslo, najviac {scale} desatinné miesta)."
        ))
    };
    let (whole, fractional) = cleaned.split_once('.').unwrap_or((cleaned, ""));
    if whole.is_empty()
        || whole.len() > 9
        || !whole.bytes().all(|c| c.is_ascii_digit())
        || fractional.len() > scale as usize
        || !fractional.bytes().all(|c| c.is_ascii_digit())
        || (cleaned.contains('.') && fractional.is_empty())
    {
        return Err(invalid());
    }
    let factor = 10_i64.pow(scale);
    let units = whole.parse::<i64>().map_err(|_| invalid())? * factor
        + if fractional.is_empty() {
            0
        } else {
            fractional.parse::<i64>().map_err(|_| invalid())?
                * 10_i64.pow(scale - fractional.len() as u32)
        };
    if units > max_units {
        return Err(invalid());
    }
    Ok((
        format!(
            "{}.{:0width$}",
            units / factor,
            units % factor,
            width = scale as usize
        ),
        units,
    ))
}
pub fn totals(items: &[Item]) -> Result<Totals> {
    let mut totals = Totals::default();
    let mut groups: BTreeMap<i64, Amounts> = BTreeMap::new();
    for item in items {
        let (_, qty) = decimal(&json!(item.quantity), 3, "Množstvo", 1_000_000_000)?;
        let (_, price) = decimal(&json!(item.unit_price), 2, "Jednotková cena", 9_999_999_999)?;
        let net = i64::try_from((i128::from(qty) * i128::from(price) + 500) / 1000)
            .map_err(|_| Problem::internal())?;
        let vat = (net * item.vat_rate + 50) / 100;
        if totals.amounts.gross + net + vat > 99_999_999_999 {
            return Err(Problem::new(
                "Suma faktúry je príliš vysoká (maximum 999 999 999,99 EUR).",
            ));
        }
        let line = Amounts {
            net,
            vat,
            gross: net + vat,
        };
        totals.amounts.net += net;
        totals.amounts.vat += vat;
        totals.amounts.gross += line.gross;
        let group = groups.entry(item.vat_rate).or_default();
        group.net += net;
        group.vat += vat;
        group.gross += line.gross;
        totals.lines.push(line);
    }
    totals.groups = groups
        .into_iter()
        .map(|(rate, amounts)| VatGroup { rate, amounts })
        .collect();
    Ok(totals)
}
pub fn invoice(input: &Value, issuing: bool) -> Result<InvoiceData> {
    let issue_date = date(
        input.get("issueDate").unwrap_or(&json!("")),
        "Vystavenie",
        issuing,
    )?;
    let delivery_date = date(
        input.get("deliveryDate").unwrap_or(&json!("")),
        "Dodanie",
        issuing,
    )?;
    let due_date = date(
        input.get("dueDate").unwrap_or(&json!("")),
        "Splatnosť",
        issuing,
    )?;
    if !due_date.is_empty() && !issue_date.is_empty() && due_date < issue_date {
        return Err(Problem::new(
            "Splatnosť nesmie byť pred dátumom vystavenia.",
        ));
    }
    if input
        .get("currency")
        .is_some_and(|v| v.as_str() != Some("EUR"))
    {
        return Err(Problem::new("Táto verzia podporuje fakturáciu v EUR."));
    }
    let profiles = input.get("qrProfiles").cloned().unwrap_or(json!(["sk"]));
    let profiles = profiles
        .as_array()
        .filter(|p| p.len() <= 2)
        .ok_or_else(|| Problem::new("Vyber najviac dva rôzne QR kódy."))?;
    let mut qr_profiles = vec![];
    for profile in profiles {
        let profile = profile
            .as_str()
            .filter(|s| label(s).is_some())
            .ok_or_else(|| Problem::new("Neznámy typ QR kódu."))?;
        if qr_profiles.contains(&profile.to_owned()) {
            return Err(Problem::new("Vyber najviac dva rôzne QR kódy."));
        }
        qr_profiles.push(profile.to_owned());
    }
    let customer_id = match input.get("customerId") {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            integer(v)
                .filter(|v| *v > 0)
                .ok_or_else(|| Problem::new("Neplatný zákazník."))?,
        ),
    };
    if issuing && customer_id.is_none() {
        return Err(Problem::new("Vyber odberateľa zo zoznamu zákazníkov."));
    }
    let lines = input
        .get("items")
        .and_then(Value::as_array)
        .filter(|v| !v.is_empty() && v.len() <= 100)
        .ok_or_else(|| Problem::new("Faktúra musí mať 1 až 100 položiek."))?;
    let mut items = vec![];
    for (i, line) in lines.iter().enumerate() {
        if !line.is_object() {
            return Err(Problem::new("Neplatná položka faktúry."));
        }
        let description = text(
            line.get("description").unwrap_or(&json!("")),
            1000,
            &format!("Popis položky {}", i + 1),
            issuing,
            true,
        )?;
        let (quantity, qty) = decimal(
            line.get("quantity").unwrap_or(&json!("1")),
            3,
            "Množstvo",
            1_000_000_000,
        )?;
        let (unit_price, _) = decimal(
            line.get("unitPrice").unwrap_or(&json!("0")),
            2,
            "Jednotková cena",
            9_999_999_999,
        )?;
        let vat_rate = integer(line.get("vatRate").unwrap_or(&json!(23)))
            .filter(|n| [0, 5, 19, 23].contains(n))
            .ok_or_else(|| Problem::new("Vyber sadzbu DPH 0, 5, 19 alebo 23 %."))?;
        if issuing && qty <= 0 {
            return Err(Problem::new(
                "Množstvo každej položky musí byť väčšie ako nula.",
            ));
        }
        items.push(Item {
            description,
            quantity,
            unit: text(
                line.get("unit").unwrap_or(&json!("ks")),
                12,
                "Jednotka",
                issuing,
                false,
            )?,
            unit_price,
            vat_rate,
        });
    }
    let totals = totals(&items)?;
    if issuing && totals.amounts.gross <= 0 {
        return Err(Problem::new(
            "Celková suma faktúry musí byť väčšia ako nula.",
        ));
    }
    Ok(InvoiceData {
        customer_id,
        customer: None,
        supplier: Value::Null,
        issue_date,
        delivery_date,
        due_date,
        order_number: field_text(input, "orderNumber", 80, "Číslo objednávky", false)?,
        currency: "EUR".into(),
        note: text(
            input.get("note").unwrap_or(&json!("")),
            2000,
            "Poznámka",
            false,
            true,
        )?,
        qr_profiles,
        items,
        totals,
    })
}
pub fn money(cents: i64) -> String {
    let digits = (cents.abs() / 100).to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(' ');
        }
        grouped.push(c);
    }
    format!(
        "{}{grouped},{:02} €",
        if cents < 0 { "-" } else { "" },
        cents.abs() % 100
    )
}
