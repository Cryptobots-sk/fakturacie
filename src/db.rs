use crate::{
    error::{Problem, Result},
    model::{Invoice, InvoiceData},
    rules,
};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde_json::{Value, json};
use std::{path::Path, time::Duration};

pub fn connect(path: &Path) -> Result<Connection> {
    let db = Connection::open(path)?;
    db.busy_timeout(Duration::from_secs(10))?;
    db.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;")?;
    Ok(db)
}
pub fn initialize(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut db = connect(path)?;
    db.execute_batch(include_str!("../sql/schema.sql"))?;
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let count: i64 = tx.query_row("SELECT COUNT(*) FROM settings", [], |r| r.get(0))?;
    if count == 0 {
        let supplier = json!({"name":"","street":"","zip":"","city":"","ico":"","dic":"","vatId":"","email":"","iban":"","bic":"","registry":"","country":"Slovensko","paymentDays":14,"qrProfile":"sk"});
        tx.execute(
            "INSERT INTO settings(id,data) VALUES(1,?1)",
            [supplier.to_string()],
        )?;
    }
    tx.commit()?;
    Ok(())
}
pub fn settings(db: &Connection) -> Result<Value> {
    let row: Option<(String, i64)> = db
        .query_row("SELECT data,version FROM settings WHERE id=1", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?;
    let (data, version) =
        row.ok_or_else(|| Problem::status(503, "Najprv spusti prípravu aplikácie."))?;
    let mut value: Value = serde_json::from_str(&data)?;
    value["version"] = version.into();
    Ok(value)
}
pub fn customer(db: &Connection, id: i64) -> Result<Value> {
    let row: Option<(String, i64, i64)> = db
        .query_row(
            "SELECT data,version,archived FROM customers WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (data, version, archived) =
        row.ok_or_else(|| Problem::status(404, "Zákazník sa nenašiel."))?;
    let mut value: Value = serde_json::from_str(&data)?;
    value["id"] = id.into();
    value["version"] = version.into();
    value["archived"] = (archived != 0).into();
    Ok(value)
}
pub fn customers(db: &Connection, archived: bool) -> Result<Vec<Value>> {
    let mut statement =
        db.prepare("SELECT id FROM customers WHERE archived=?1 ORDER BY id DESC")?;
    let ids = statement
        .query_map([archived as i64], |r| r.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut rows = ids
        .into_iter()
        .map(|id| customer(db, id))
        .collect::<Result<Vec<_>>>()?;
    rows.sort_by(|a, b| {
        natord::compare(
            &crate::model::string(a, "name").to_ascii_lowercase(),
            &crate::model::string(b, "name").to_ascii_lowercase(),
        )
    });
    Ok(rows)
}
type InvoiceRow = (i64, Option<String>, String, String, i64, String, String);
fn invoice_row(row: &Row<'_>) -> rusqlite::Result<InvoiceRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
    ))
}
fn hydrate(row: InvoiceRow) -> Result<Invoice> {
    let data: InvoiceData = serde_json::from_str(&row.3)?;
    Ok(Invoice {
        data,
        id: Some(row.0),
        number: row.1,
        status: row.2,
        version: row.4,
        client_key: row.5,
        updated_at: Some(row.6),
    })
}
pub fn invoice(db: &Connection, id: i64) -> Result<Invoice> {
    let row = db
        .query_row(
            "SELECT id,number,status,data,version,client_key,updated_at FROM invoices WHERE id=?1",
            [id],
            invoice_row,
        )
        .optional()?;
    hydrate(row.ok_or_else(|| Problem::status(404, "Faktúra sa nenašla."))?)
}
pub fn invoices(db: &Connection) -> Result<Vec<Invoice>> {
    let mut statement = db.prepare(
        "SELECT id,number,status,data,version,client_key,updated_at FROM invoices ORDER BY id DESC",
    )?;
    let rows = statement
        .query_map([], invoice_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter().map(hydrate).collect()
}
/// Older imported PDFs can have SQLite storage class TEXT despite containing binary bytes.
/// Reading ValueRef accepts both TEXT and BLOB without transcoding or regenerating the file.
pub fn stored_pdf(db: &Connection, id: i64) -> Result<Vec<u8>> {
    let pdf: Option<Vec<u8>> = db
        .query_row("SELECT pdf FROM invoices WHERE id=?1", [id], |r| {
            Ok(match r.get_ref(0)? {
                rusqlite::types::ValueRef::Blob(b) | rusqlite::types::ValueRef::Text(b) => {
                    Some(b.to_vec())
                }
                _ => None,
            })
        })
        .optional()?
        .flatten();
    pdf.filter(|p| !p.is_empty())
        .ok_or_else(|| Problem::status(500, "Uložené PDF sa nepodarilo načítať."))
}
pub fn save_customer(db: &mut Connection, input: &Value) -> Result<Value> {
    let data = rules::party(input, false)?;
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let id = if input.get("id").is_some_and(|v| !v.is_null()) {
        let id = input["id"]
            .as_i64()
            .filter(|n| *n > 0)
            .ok_or_else(|| Problem::new("Neplatné číslo zákazníka."))?;
        let current = customer(&tx, id)?;
        if current["version"] != input["version"] {
            return Err(Problem::status(
                409,
                "Zákazník sa zmenil v inom okne. Obnov stránku a skús znovu.",
            ));
        }
        tx.execute(
            "UPDATE customers SET data=?1,version=version+1 WHERE id=?2",
            params![data.to_string(), id],
        )?;
        id
    } else {
        tx.execute("INSERT INTO customers(data) VALUES(?1)", [data.to_string()])?;
        tx.last_insert_rowid()
    };
    let customer = customer(&tx, id)?;
    tx.commit()?;
    Ok(customer)
}
pub fn save_settings(db: &Connection, input: &Value) -> Result<Value> {
    let data = rules::party(input, true)?;
    let version = input["version"]
        .as_i64()
        .ok_or_else(|| Problem::new("Neplatná verzia nastavení."))?;
    if db.execute(
        "UPDATE settings SET data=?1,version=version+1 WHERE id=1 AND version=?2",
        params![data.to_string(), version],
    )? != 1
    {
        return Err(Problem::status(
            409,
            "Nastavenia sa zmenili v inom okne. Obnov stránku.",
        ));
    }
    settings(db)
}
pub fn archive_customer(db: &Connection, input: &Value) -> Result<Value> {
    let id = rules::positive_id(input, "id")?;
    let version = rules::positive_id(input, "version")?;
    let archived = input["archived"]
        .as_bool()
        .ok_or_else(|| Problem::new("Neplatný stav zákazníka."))?;
    if db.execute(
        "UPDATE customers SET archived=?1,version=version+1 WHERE id=?2 AND version=?3",
        params![archived as i64, id, version],
    )? != 1
    {
        return Err(Problem::status(
            409,
            "Zákazník sa zmenil. Obnov stránku a skús znovu.",
        ));
    }
    Ok(json!({"saved":true}))
}
