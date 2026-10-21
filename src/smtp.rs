use crate::{
    Backend, db,
    error::{Problem, Result},
    invoices::is_key,
    model::{Invoice, string},
    now, qr, random_key, rules,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use lettre::{
    Message, SmtpTransport, Transport,
    message::{Attachment, Mailbox, MultiPart, SinglePart, header::ContentType},
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::Duration;

pub fn connection(db: &Connection) -> Result<Value> {
    let row: Option<(String, i64)> = db
        .query_row(
            "SELECT data,version FROM smtp_settings WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match row {
        Some((data, version)) => {
            let mut value: Value = serde_json::from_str(&data)?;
            value["version"] = version.into();
            Ok(value)
        }
        None => Ok(
            json!({"host":"","port":587,"encryption":"tls","username":"","password":"","fromEmail":"","fromName":"","replyTo":"","version":0}),
        ),
    }
}
pub fn settings(db: &Connection) -> Result<Value> {
    let mut data = connection(db)?;
    data["passwordSet"] = (!string(&data, "password").is_empty()).into();
    data["configured"] =
        (!string(&data, "host").is_empty() && !string(&data, "fromEmail").is_empty()).into();
    data.as_object_mut()
        .ok_or_else(Problem::internal)?
        .shift_remove("password");
    Ok(data)
}
pub fn save_settings(db: &mut Connection, input: &Value) -> Result<Value> {
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let current = connection(&tx)?;
    if input["version"] != current["version"] {
        return Err(Problem::status(
            409,
            "SMTP nastavenia sa zmenili v inom okne. Obnov stránku.",
        ));
    }
    let host = rules::text(
        input.get("host").unwrap_or(&json!("")),
        253,
        "SMTP server",
        true,
        false,
    )?;
    if !regex::Regex::new(r"^[a-zA-Z0-9][a-zA-Z0-9.:-]*$")
        .expect("static regex")
        .is_match(&host)
    {
        return Err(Problem::new(
            "Zadaj iba názov SMTP servera alebo IP adresu, bez protokolu a cesty.",
        ));
    }
    let port = rules::integer(&input["port"])
        .filter(|n| (1..=65535).contains(n))
        .ok_or_else(|| Problem::new("SMTP port musí byť od 1 do 65535."))?;
    let encryption = input["encryption"]
        .as_str()
        .filter(|v| ["tls", "ssl", "none"].contains(v))
        .ok_or_else(|| Problem::new("Vyber zabezpečenie SMTP spojenia."))?;
    let username = rules::text(
        input.get("username").unwrap_or(&json!("")),
        254,
        "SMTP používateľ",
        false,
        false,
    )?;
    let password = input.get("password").unwrap_or(&Value::Null);
    let password = if password.is_null() {
        ""
    } else {
        password
            .as_str()
            .ok_or_else(|| Problem::new("Skontroluj SMTP heslo."))?
    };
    if password.len() > 1024 || password.contains(['\0', '\r', '\n']) {
        return Err(Problem::new("Skontroluj SMTP heslo."));
    }
    let clear = input["clearPassword"] == true
        || string(&current, "host") != host
        || string(&current, "username") != username;
    let password = if !password.is_empty() {
        password
    } else if clear {
        ""
    } else {
        string(&current, "password")
    };
    let data = json!({"host":host,"port":port,"encryption":encryption,"username":username,"password":password,
        "fromEmail":rules::email(input.get("fromEmail").unwrap_or(&json!("")),"Odosielateľ",true)?,
        "fromName":rules::text(input.get("fromName").unwrap_or(&json!("")),70,"Meno odosielateľa",false,false)?,
        "replyTo":rules::email(input.get("replyTo").unwrap_or(&json!("")),"Odpovede posielať na",false)?});
    if !username.is_empty() && password.is_empty() {
        return Err(Problem::new(
            "Pre prihlásenie zadaj SMTP heslo. Pri zmene servera alebo používateľa ho vyplň znova.",
        ));
    }
    if current["version"] == 0 {
        tx.execute(
            "INSERT INTO smtp_settings(id,data,version) VALUES(1,?1,1)",
            [data.to_string()],
        )?;
    } else {
        tx.execute(
            "UPDATE smtp_settings SET data=?1,version=version+1 WHERE id=1",
            [data.to_string()],
        )?;
    }
    let result = settings(&tx)?;
    tx.commit()?;
    Ok(result)
}
pub fn history(db: &Connection, id: i64) -> Result<Vec<Value>> {
    let mut query=db.prepare("SELECT id,recipient,subject,status,created_at,finished_at,invoice_version FROM email_deliveries WHERE invoice_id=?1 ORDER BY id DESC LIMIT 10")?;
    Ok(query.query_map([id],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"recipient":r.get::<_,String>(1)?,"subject":r.get::<_,String>(2)?,"status":r.get::<_,String>(3)?,"created_at":r.get::<_,String>(4)?,"finished_at":r.get::<_,Option<String>>(5)?,"invoice_version":r.get::<_,Option<i64>>(6)?})))?.collect::<rusqlite::Result<Vec<_>>>()?)
}
fn check_invoice(invoice: &Invoice, version: i64) -> Result<()> {
    if invoice.version != version {
        return Err(Problem::status(
            409,
            "Faktúra sa zmenila. Obnov stránku a priprav e-mail znova.",
        ));
    }
    if invoice.status != "issued" {
        return Err(Problem::new(
            "Pre odoslanie najprv vystav faktúru, aby mala číslo a platobné údaje.",
        ));
    }
    Ok(())
}
fn result(id: i64, status: &str, recipient: &str) -> Value {
    json!({"id":id,"status":status,"recipient":recipient,"message":match status {
        "sent"=>"SMTP server správu prijal.",
        "sending"=>"Požiadavka už bola prijatá. Odoslanie sa ešte nepotvrdilo; skontroluj históriu pred ďalším pokusom.",
        _=>"Odoslanie sa nepotvrdilo. Skontroluj SMTP nastavenia a schránku príjemcu pred novým pokusom.",
    }})
}
fn envelope(smtp: &Value, content: &Value) -> Result<lettre::message::MessageBuilder> {
    let from = Mailbox::new(
        if string(smtp, "fromName").is_empty() {
            None
        } else {
            Some(string(smtp, "fromName").to_owned())
        },
        string(smtp, "fromEmail")
            .parse()
            .map_err(|_| Problem::internal())?,
    );
    let mut builder = Message::builder()
        .from(from)
        .to(string(content, "to")
            .parse()
            .map_err(|_| Problem::internal())?)
        .subject(string(content, "subject"));
    if !string(smtp, "replyTo").is_empty() {
        builder = builder.reply_to(
            string(smtp, "replyTo")
                .parse()
                .map_err(|_| Problem::internal())?,
        );
    }
    Ok(builder)
}
fn transport_send(message: &Message, smtp: &Value) -> std::result::Result<(), ()> {
    let host = string(smtp, "host");
    let tls = match string(smtp, "encryption") {
        "none" => Tls::None,
        "tls" => Tls::Required(TlsParameters::new_native(host.to_owned()).map_err(|_| ())?),
        "ssl" => Tls::Wrapper(TlsParameters::new_native(host.to_owned()).map_err(|_| ())?),
        _ => return Err(()),
    };
    let port = smtp["port"]
        .as_u64()
        .and_then(|v| u16::try_from(v).ok())
        .ok_or(())?;
    let mut builder = SmtpTransport::builder_dangerous(host)
        .port(port)
        .tls(tls)
        .timeout(Some(Duration::from_secs(15)));
    if !string(smtp, "username").is_empty() {
        builder = builder.credentials(Credentials::new(
            string(smtp, "username").to_owned(),
            string(smtp, "password").to_owned(),
        ));
    }
    builder.build().send(message).map(|_| ()).map_err(|_| ())
}
impl Backend {
    pub fn email_preview(&self, db: &Connection, id: i64, version: i64) -> Result<Value> {
        let invoice = db::invoice(db, id)?;
        check_invoice(&invoice, version)?;
        let smtp = settings(db)?;
        let number = invoice.number.as_deref().ok_or_else(Problem::internal)?;
        let customer = invoice
            .data
            .customer
            .as_ref()
            .ok_or_else(Problem::internal)?;
        let supplier = &invoice.data.supplier;
        let due = chrono::NaiveDate::parse_from_str(&invoice.data.due_date, "%Y-%m-%d")
            .map_err(|_| Problem::internal())?
            .format("%d.%m.%Y")
            .to_string();
        let iban = string(supplier, "iban")
            .as_bytes()
            .chunks(4)
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        Ok(
            json!({"requestKey":random_key(16),"invoiceVersion":version,"smtpVersion":smtp["version"],"configured":smtp["configured"],"fromEmail":smtp["fromEmail"],"to":string(customer,"email"),"subject":format!("Faktúra {number} – {}",string(supplier,"name")),
            "message":format!("Dobrý deň,\n\nv prílohe Vám posielame faktúru č. {number}. Údaje k platbe nájdete nižšie.\n\nĎakujeme za spoluprácu.\n{}",string(supplier,"name")),
            "number":number,"customer":string(customer,"name"),"amount":rules::money(invoice.data.totals.amounts.gross),"dueDate":due,"iban":iban,"bic":string(supplier,"bic"),"qrImages":qr::images(&invoice)?,"attachment":format!("Faktura_{number}.pdf")}),
        )
    }
    fn invoice_message(
        &self,
        invoice: &Invoice,
        pdf: Vec<u8>,
        smtp: &Value,
        content: &Value,
    ) -> Result<Message> {
        let number = invoice.number.as_deref().ok_or_else(Problem::internal)?;
        let mut images = vec![];
        let mut inline = vec![];
        for image in qr::images(invoice)? {
            let bytes = STANDARD
                .decode(
                    image
                        .data_uri
                        .split_once(',')
                        .ok_or_else(Problem::internal)?
                        .1,
                )
                .map_err(|_| Problem::internal())?;
            let cid = format!("qr-{}-{}@fakturacie.local", image.profile, random_key(8));
            images.push(json!({"src":format!("cid:{cid}"),"label":image.label}));
            inline.push(
                Attachment::new_inline(cid)
                    .body(bytes, "image/png".parse().map_err(|_| Problem::internal())?),
            );
        }
        let html = self.views.render(
            "email.html",
            &json!({"invoice":invoice,"content":content,"images":images}),
        )?;
        let supplier = &invoice.data.supplier;
        let customer = invoice
            .data
            .customer
            .as_ref()
            .ok_or_else(Problem::internal)?;
        let due = chrono::NaiveDate::parse_from_str(&invoice.data.due_date, "%Y-%m-%d")
            .map_err(|_| Problem::internal())?
            .format("%d.%m.%Y");
        let plain = format!(
            "{}\n\nFaktúra {number}\nDodávateľ: {}\nOdberateľ: {}\nSuma: {}\nSplatnosť: {due}\nIBAN: {}\nBIC / SWIFT: {}\nVariabilný symbol: {number}\nSpráva pre príjemcu: Faktura {number}\nPDF faktúra je v prílohe.",
            string(content, "message"),
            string(supplier, "name"),
            string(customer, "name"),
            rules::money(invoice.data.totals.amounts.gross),
            string(supplier, "iban"),
            if string(supplier, "bic").is_empty() {
                "—"
            } else {
                string(supplier, "bic")
            }
        );
        let mut related = MultiPart::related().singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_HTML)
                .body(html),
        );
        for image in inline {
            related = related.singlepart(image);
        }
        let body = MultiPart::mixed()
            .multipart(
                MultiPart::alternative()
                    .singlepart(SinglePart::plain(plain))
                    .multipart(related),
            )
            .singlepart(Attachment::new(format!("Faktura_{number}.pdf")).body(
                pdf,
                "application/pdf".parse().map_err(|_| Problem::internal())?,
            ));
        envelope(smtp, content)?
            .multipart(body)
            .map_err(|_| Problem::internal())
    }
    pub fn send_email(&self, db: &mut Connection, input: &Value, probe: bool) -> Result<Value> {
        let (invoice_id, version, content) = if probe {
            (
                None,
                None,
                json!({"to":rules::email(input.get("to").unwrap_or(&json!("")),"Príjemca skúšobného e-mailu",true)?,"subject":"Skúška SMTP – Fakturácie","message":"Toto je skúšobný e-mail z aplikácie Fakturácie. SMTP server prijal túto správu."}),
            )
        } else {
            let id = rules::positive_id(input, "id")?;
            let version = rules::positive_id(input, "version")?;
            (
                Some(id),
                Some(version),
                json!({"to":rules::email(input.get("to").unwrap_or(&json!("")),"Príjemca",true)?,
                "subject":rules::text(input.get("subject").unwrap_or(&json!("")),180,"Predmet",true,false)?,
                "message":rules::text(input.get("message").unwrap_or(&json!("")),4000,"Správa",true,true)?}),
            )
        };
        let key = input["requestKey"]
            .as_str()
            .filter(|s| is_key(s, 32))
            .ok_or_else(|| Problem::new("Neplatná požiadavka na odoslanie. Otvor e-mail znova."))?;
        // Preserve PHP's array/object field order so requests stored before migration reconcile too.
        let hash = hex::encode(Sha256::digest(serde_json::to_vec(&json!([
            invoice_id,
            version,
            input["smtpVersion"],
            content
        ]))?));
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let existing:Option<(i64,String,String,String)>=tx.query_row("SELECT id,payload_hash,status,recipient FROM email_deliveries WHERE request_key=?1",[key],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        if let Some((id, previous, status, recipient)) = existing {
            if hash != previous {
                return Err(Problem::status(
                    409,
                    "Táto požiadavka už bola použitá pre inú správu. Otvor nový e-mail.",
                ));
            }
            tx.commit()?;
            return Ok(result(id, &status, &recipient));
        }
        let smtp = connection(&tx)?;
        if input["smtpVersion"] != smtp["version"] {
            return Err(Problem::status(
                409,
                "SMTP nastavenia sa zmenili. Obnov náhľad správy.",
            ));
        }
        if string(&smtp, "host").is_empty() || string(&smtp, "fromEmail").is_empty() {
            return Err(Problem::new(
                "Najprv nastav SMTP server v časti Moja firma.",
            ));
        }
        let (message, pdf_hash) = if let Some(id) = invoice_id {
            let invoice = db::invoice(&tx, id)?;
            check_invoice(&invoice, version.ok_or_else(Problem::internal)?)?;
            let pdf = db::stored_pdf(&tx, id)?;
            let hash = hex::encode(Sha256::digest(&pdf));
            (
                self.invoice_message(&invoice, pdf, &smtp, &content)?,
                Some(hash),
            )
        } else {
            (
                envelope(&smtp, &content)?
                    .body(string(&content, "message").to_owned())
                    .map_err(|_| Problem::internal())?,
                None,
            )
        };
        tx.execute("INSERT INTO email_deliveries(request_key,payload_hash,invoice_id,invoice_version,recipient,subject,status,pdf_sha256,created_at) VALUES(?1,?2,?3,?4,?5,?6,'sending',?7,?8)",params![key,hash,invoice_id,version,string(&content,"to"),string(&content,"subject"),pdf_hash,now()])?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        // SMTP is outside the SQLite transaction. An ambiguous result is never retried automatically.
        // Do not log transport errors: some SMTP implementations include credentials in diagnostics.
        let status = if transport_send(&message, &smtp).is_ok() {
            "sent"
        } else {
            "unknown"
        };
        db.execute(
            "UPDATE email_deliveries SET status=?1,finished_at=?2 WHERE id=?3",
            params![status, now(), id],
        )?;
        Ok(result(id, status, string(&content, "to")))
    }
}
