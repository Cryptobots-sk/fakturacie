use crate::{
    Backend, db,
    error::{Problem, Result},
    model::{Invoice, InvoiceData, Item, Totals},
    now, random_key, rules,
};
use chrono::Duration;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

impl Backend {
    pub fn blank(&self, db: &Connection) -> Result<Invoice> {
        let supplier = db::settings(db)?;
        let today = chrono::Utc::now()
            .with_timezone(&chrono_tz::Europe::Bratislava)
            .date_naive();
        let due = today + Duration::days(supplier["paymentDays"].as_i64().unwrap_or(14));
        let qr = supplier["qrProfile"].as_str().unwrap_or("sk").to_owned();
        Ok(Invoice {
            id: None,
            version: 0,
            client_key: random_key(16),
            number: None,
            status: "draft".into(),
            updated_at: None,
            data: InvoiceData {
                customer_id: None,
                customer: None,
                supplier,
                issue_date: today.to_string(),
                delivery_date: today.to_string(),
                due_date: due.to_string(),
                order_number: String::new(),
                currency: "EUR".into(),
                note: String::new(),
                qr_profiles: vec![qr],
                items: vec![Item {
                    description: String::new(),
                    quantity: "1.000".into(),
                    unit: "ks".into(),
                    unit_price: "0.00".into(),
                    vat_rate: 23,
                }],
                totals: Totals::default(),
            },
        })
    }
    pub fn save_invoice(&self, db: &mut Connection, input: &Value) -> Result<Invoice> {
        let key = input["clientKey"]
            .as_str()
            .filter(|s| is_key(s, 32))
            .ok_or_else(|| Problem::new("Neplatný identifikátor konceptu. Obnov stránku."))?;
        let version = input["version"]
            .as_i64()
            .ok_or_else(|| Problem::new("Neplatná verzia faktúry."))?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let existing: Option<i64> = tx
            .query_row("SELECT id FROM invoices WHERE client_key=?1", [key], |r| {
                r.get(0)
            })
            .optional()?;
        let current = existing.map(|id| db::invoice(&tx, id)).transpose()?;
        if current.is_none() && input.get("id").is_some_and(|v| !v.is_null()) {
            return Err(Problem::status(404, "Faktúra sa nenašla."));
        }
        if current
            .as_ref()
            .is_some_and(|c| c.id != input["id"].as_i64())
        {
            return Err(Problem::status(
                409,
                "Identifikátor faktúry nesúhlasí. Obnov stránku.",
            ));
        }
        if current.as_ref().map_or(0, |c| c.version) != version {
            return Err(Problem::status(
                409,
                "Faktúra sa zmenila v inom okne alebo sa predchádzajúce uloženie už dokončilo. Obnov stránku, aby si načítal uloženú verziu.",
            ));
        }
        let issued = current.as_ref().is_some_and(|c| c.status == "issued");
        let mut data = rules::invoice(input, issued)?;
        let customer = data
            .customer_id
            .map(|id| db::customer(&tx, id))
            .transpose()?;
        if customer.as_ref().is_some_and(|c| c["archived"] == true)
            && current.as_ref().and_then(|c| c.data.customer_id) != data.customer_id
        {
            return Err(Problem::new(
                "Tento zákazník je archivovaný. Najprv ho obnov v adresári.",
            ));
        }
        data.customer = match &current {
            Some(c)
                if c.data.customer_id == data.customer_id
                    && !input.get("refreshCustomer").is_some_and(truthy) =>
            {
                c.data.customer.clone()
            }
            _ => customer,
        };
        data.supplier = match &current {
            Some(c) => c.data.supplier.clone(),
            None => db::settings(&tx)?,
        };
        if current.is_none() && input["supplier"]["version"] != data.supplier["version"] {
            return Err(Problem::status(
                409,
                "Údaje tvojej firmy sa zmenili počas úpravy. Obnov stránku pred uložením konceptu.",
            ));
        }
        let pdf = if issued {
            data.supplier = rules::party(&data.supplier, true)?;
            data.customer = Some(rules::party(
                data.customer.as_ref().ok_or_else(Problem::internal)?,
                false,
            )?);
            let mut rendered = current.as_ref().ok_or_else(Problem::internal)?.clone();
            rendered.data = data.clone();
            Some(self.render_pdf(&rendered, false)?)
        } else {
            None
        };
        let now = now();
        let id = if let Some(current) = current {
            let id = current.id.ok_or_else(Problem::internal)?;
            tx.execute(
                "UPDATE invoices SET data=?1,pdf=?2,version=version+1,updated_at=?3 WHERE id=?4",
                params![serde_json::to_string(&data)?, pdf, now, id],
            )?;
            id
        } else {
            tx.execute("INSERT INTO invoices(client_key,status,data,created_at,updated_at) VALUES(?1,'draft',?2,?3,?3)",params![key,serde_json::to_string(&data)?,now])?;
            tx.last_insert_rowid()
        };
        let result = db::invoice(&tx, id)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn issue_invoice(&self, db: &mut Connection, id: i64, version: i64) -> Result<Invoice> {
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current = db::invoice(&tx, id)?;
        if current.status == "issued" {
            tx.commit()?;
            return Ok(current);
        }
        if current.version != version {
            return Err(Problem::status(
                409,
                "Koncept sa zmenil. Načítaj jeho aktuálnu verziu pred vystavením.",
            ));
        }
        let mut data = rules::invoice(&serde_json::to_value(&current)?, true)?;
        data.supplier = rules::party(&current.data.supplier, true)?;
        data.customer = Some(rules::party(
            current
                .data
                .customer
                .as_ref()
                .ok_or_else(Problem::internal)?,
            false,
        )?);
        let year = data.issue_date[..4]
            .parse::<i64>()
            .map_err(|_| Problem::internal())?;
        tx.execute(
            "INSERT OR IGNORE INTO sequences(year,last_number) VALUES(?1,0)",
            [year],
        )?;
        let sequence: i64 = tx.query_row(
            "SELECT last_number+1 FROM sequences WHERE year=?1",
            [year],
            |r| r.get(0),
        )?;
        if sequence > 9999 {
            return Err(Problem::new("Ročný číselný rad je vyčerpaný."));
        }
        let number = format!("{year:04}{sequence:04}");
        let mut issued = current;
        issued.data = data;
        issued.number = Some(number.clone());
        let pdf = self.render_pdf(&issued, false)?;
        tx.execute(
            "UPDATE sequences SET last_number=?1 WHERE year=?2",
            params![sequence, year],
        )?;
        tx.execute("UPDATE invoices SET data=?1,number=?2,status='issued',version=version+1,pdf=?3,updated_at=?4 WHERE id=?5",params![serde_json::to_string(&issued.data)?,number,pdf,now(),id])?;
        let result = db::invoice(&tx, id)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn copy_invoice(&self, db: &mut Connection, id: i64) -> Result<Invoice> {
        let original = db::invoice(db, id)?;
        let mut blank = self.blank(db)?;
        blank.data.customer_id = original.data.customer_id;
        blank.data.items = original.data.items;
        blank.data.note = original.data.note;
        blank.data.qr_profiles = original.data.qr_profiles;
        if let Some(id) = blank.data.customer_id {
            let customer = db::customer(db, id)?;
            if customer["archived"] == true {
                blank.data.customer_id = None;
            } else {
                let date = chrono::NaiveDate::parse_from_str(&blank.data.issue_date, "%Y-%m-%d")
                    .map_err(|_| Problem::internal())?;
                blank.data.due_date = (date
                    + Duration::days(customer["paymentDays"].as_i64().unwrap_or(14)))
                .to_string();
            }
        }
        self.save_invoice(db, &serde_json::to_value(blank)?)
    }
    pub fn delete_invoice(&self, db: &mut Connection, id: i64, version: i64) -> Result<Value> {
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current = db::invoice(&tx, id)?;
        if current.version != version {
            return Err(Problem::status(
                409,
                "Faktúra sa zmenila v inom okne. Pred vymazaním obnov stránku.",
            ));
        }
        tx.execute(
            "DELETE FROM invoices WHERE id=?1 AND version=?2",
            params![id, version],
        )?;
        tx.commit()?;
        Ok(json!({"deleted":true}))
    }
    pub fn invoice_pdf(&self, db: &Connection, id: i64) -> Result<Vec<u8>> {
        let invoice = db::invoice(db, id)?;
        if invoice.status == "draft" {
            self.render_pdf(&invoice, true)
        } else {
            db::stored_pdf(db, id)
        }
    }
}
pub fn is_key(key: &str, len: usize) -> bool {
    key.len() == len
        && key
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty() && s != "0",
        Value::Number(n) => n.as_i64() != Some(0),
        _ => true,
    }
}
