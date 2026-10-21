use fakturacie::{
    Backend, db,
    model::{Invoice, Item},
    qr, rules, smtp,
};
use rusqlite::params;
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};

struct Fixture {
    _directory: tempfile::TempDir,
    app: Arc<Backend>,
    customer: Value,
}
fn party() -> Value {
    json!({"name":"Test dodávateľ s. r. o.","street":"Testovacia 1","zip":"010 01","city":"Žilina","country":"Slovensko","ico":"12345678","dic":"1234567890","vatId":"SK1234567890","email":"test@example.test","iban":"SK5000000000000000000001","bic":"TESTSK00"})
}
fn fixture() -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("test.sqlite");
    db::initialize(&path).unwrap();
    let root = PathBuf::from(
        std::env::var("FAKTURACIE_ROOT").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").into()),
    );
    let app = Arc::new(Backend::new(&root, &path).unwrap());
    let mut conn = db::connect(&path).unwrap();
    let mut supplier = party();
    supplier["version"] = 1.into();
    db::save_settings(&conn, &supplier).unwrap();
    conn.execute(
        "INSERT INTO sequences(year,last_number) VALUES(2026,13)",
        [],
    )
    .unwrap();
    let customer = db::save_customer(&mut conn, &party()).unwrap();
    Fixture {
        _directory: directory,
        app,
        customer,
    }
}
fn draft(f: &Fixture) -> Invoice {
    let mut conn = db::connect(&f.app.database).unwrap();
    let mut invoice = f.app.blank(&conn).unwrap();
    invoice.data.customer_id = f.customer["id"].as_i64();
    invoice.data.issue_date = "2026-10-15".into();
    invoice.data.delivery_date = "2026-10-15".into();
    invoice.data.due_date = "2026-10-29".into();
    invoice.data.items = vec![Item {
        description: "Vývoj & test <script>alert(1)</script> „Žilina“".into(),
        quantity: "1.000".into(),
        unit: "JV".into(),
        unit_price: "2880.00".into(),
        vat_rate: 23,
    }];
    invoice.data.qr_profiles = vec!["sk".into(), "epc".into()];
    f.app
        .save_invoice(&mut conn, &serde_json::to_value(invoice).unwrap())
        .unwrap()
}
fn issue(f: &Fixture, invoice: &Invoice) -> Invoice {
    let mut conn = db::connect(&f.app.database).unwrap();
    f.app
        .issue_invoice(&mut conn, invoice.id.unwrap(), invoice.version)
        .unwrap()
}
fn error<T: std::fmt::Debug>(result: fakturacie::error::Result<T>, status: u16) {
    assert_eq!(result.unwrap_err().status, status);
}

#[test]
fn input_validation_and_iban_match_legacy_contract() {
    assert!(rules::valid_iban("SK5000000000000000000001"));
    assert!(!rules::valid_iban("SK5000000000000000000002"));
    let mut invalid = party();
    invalid["name"] = "Injected\tIBAN".into();
    error(rules::party(&invalid, false), 422);
    error(rules::date(&json!("2026-02-30"), "Test", true), 422);
    error(rules::date(&json!("2026-1-01"), "Test", true), 422);
    error(rules::date(&json!("2100-01-01"), "Test", true), 422);
    assert_eq!(
        rules::date(&json!("2099-12-31"), "Test", true).unwrap(),
        "2099-12-31"
    );
    error(rules::decimal(&json!(1.25), 2, "Cena", 99999), 422);
    assert_eq!(
        rules::decimal(&json!("1 234,50"), 2, "Cena", 999999).unwrap(),
        ("1234.50".into(), 123450)
    );
}
#[test]
fn exact_amounts_round_per_line_and_group_vat() {
    let item = |qty: &str, price: &str, rate| Item {
        description: "Test".into(),
        quantity: qty.into(),
        unit: "ks".into(),
        unit_price: price.into(),
        vat_rate: rate,
    };
    let totals = rules::totals(&[item("1.000", "2880.00", 23)]).unwrap();
    assert_eq!(
        (totals.amounts.net, totals.amounts.vat, totals.amounts.gross),
        (288000, 66240, 354240)
    );
    let totals = rules::totals(&[item("0.125", "0.04", 23), item("3.000", "0.10", 19)]).unwrap();
    assert_eq!(
        (totals.amounts.net, totals.amounts.vat, totals.amounts.gross),
        (31, 6, 37)
    );
    assert_eq!(totals.groups[0].rate, 19);
    error(
        rules::totals(&[item("1000000.000", "99999999.99", 23)]),
        422,
    );
    assert_eq!(rules::money(354240), "3 542,40 €");
}
#[test]
fn drafts_versions_and_snapshots() {
    let f = fixture();
    let mut conn = db::connect(&f.app.database).unwrap();
    let saved = draft(&f);
    assert_eq!(saved.number, None);
    assert_eq!(saved.version, 1);
    let mut stale = serde_json::to_value(&saved).unwrap();
    stale["version"] = 0.into();
    error(f.app.save_invoice(&mut conn, &stale), 409);
    let revised = f
        .app
        .save_invoice(&mut conn, &serde_json::to_value(&saved).unwrap())
        .unwrap();
    error(
        f.app
            .save_invoice(&mut conn, &serde_json::to_value(saved).unwrap()),
        409,
    );
    let mut customer = f.customer.clone();
    customer["name"] = "Changed customer".into();
    db::save_customer(&mut conn, &customer).unwrap();
    assert_eq!(
        db::invoice(&conn, revised.id.unwrap())
            .unwrap()
            .data
            .customer
            .unwrap()["name"],
        f.customer["name"]
    );
    error(db::save_customer(&mut conn, &f.customer), 409);
    error(f.app.issue_invoice(&mut conn, revised.id.unwrap(), 1), 409);
}
#[test]
fn issued_edit_delete_pdf_and_sequence_are_atomic() {
    let f = fixture();
    let saved = draft(&f);
    let issued = issue(&f, &saved);
    let mut conn = db::connect(&f.app.database).unwrap();
    assert_eq!(issued.number.as_deref(), Some("20260014"));
    assert_eq!(issued.data.totals.amounts.gross, 354240);
    let pdf = f.app.invoice_pdf(&conn, issued.id.unwrap()).unwrap();
    assert!(pdf.starts_with(b"%PDF") && pdf.len() > 15000);
    assert_eq!(
        f.app
            .issue_invoice(&mut conn, issued.id.unwrap(), 1)
            .unwrap()
            .number,
        issued.number
    );
    let mut edited = serde_json::to_value(&issued).unwrap();
    edited["items"][0]["unitPrice"] = "3000.00".into();
    edited["number"] = "20279999".into();
    edited["status"] = "draft".into();
    edited["issueDate"] = "2027-01-01".into();
    edited["deliveryDate"] = "2027-01-01".into();
    edited["dueDate"] = "2027-01-15".into();
    edited["qrProfiles"] = json!(["cz", "epc"]);
    let updated = f.app.save_invoice(&mut conn, &edited).unwrap();
    assert_eq!(updated.number, issued.number);
    assert_eq!(updated.status, "issued");
    assert_eq!(updated.data.totals.amounts.gross, 369000);
    assert_eq!(
        conn.query_row(
            "SELECT last_number FROM sequences WHERE year=2026",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        14
    );
    let corrected = f.app.invoice_pdf(&conn, updated.id.unwrap()).unwrap();
    assert_ne!(pdf, corrected);
    assert!(qr::payload(&updated, "cz").unwrap().contains("AM:3690.00"));
    error(
        f.app
            .save_invoice(&mut conn, &serde_json::to_value(&issued).unwrap()),
        409,
    );
    error(
        f.app
            .delete_invoice(&mut conn, updated.id.unwrap(), issued.version),
        409,
    );
    let mut invalid = serde_json::to_value(&updated).unwrap();
    invalid["items"][0]["description"] = "".into();
    error(f.app.save_invoice(&mut conn, &invalid), 422);
    invalid = serde_json::to_value(&updated).unwrap();
    invalid["items"][0]["unitPrice"] = "10000000.00".into();
    invalid["qrProfiles"] = json!(["cz"]);
    error(f.app.save_invoice(&mut conn, &invalid), 422);
    assert_eq!(
        db::invoice(&conn, updated.id.unwrap()).unwrap().version,
        updated.version
    );
    assert_eq!(
        f.app.invoice_pdf(&conn, updated.id.unwrap()).unwrap(),
        corrected
    );
    f.app
        .delete_invoice(&mut conn, updated.id.unwrap(), updated.version)
        .unwrap();
    error(db::invoice(&conn, updated.id.unwrap()), 404);
    error(f.app.invoice_pdf(&conn, updated.id.unwrap()), 404);
    error(
        f.app
            .save_invoice(&mut conn, &serde_json::to_value(&updated).unwrap()),
        404,
    );
    let next = issue(&f, &draft(&f));
    assert_eq!(next.number.as_deref(), Some("20260015"));
}
#[test]
fn copy_uses_current_parties_and_archiving_preserves_history() {
    let f = fixture();
    let issued = issue(&f, &draft(&f));
    let mut conn = db::connect(&f.app.database).unwrap();
    let original_pdf = f.app.invoice_pdf(&conn, issued.id.unwrap()).unwrap();
    let mut supplier = db::settings(&conn).unwrap();
    supplier["name"] = "Updated supplier".into();
    db::save_settings(&conn, &supplier).unwrap();
    let mut customer = f.customer.clone();
    customer["name"] = "Changed customer".into();
    let customer = db::save_customer(&mut conn, &customer).unwrap();
    assert_eq!(
        f.app.invoice_pdf(&conn, issued.id.unwrap()).unwrap(),
        original_pdf
    );
    let copy = f.app.copy_invoice(&mut conn, issued.id.unwrap()).unwrap();
    assert!(copy.number.is_none());
    assert_eq!(copy.data.supplier["name"], "Updated supplier");
    assert_eq!(copy.data.customer.unwrap()["name"], "Changed customer");
    db::archive_customer(
        &conn,
        &json!({"id":customer["id"],"version":customer["version"],"archived":true}),
    )
    .unwrap();
    assert!(db::customers(&conn, false).unwrap().is_empty());
    assert_eq!(db::customers(&conn, true).unwrap().len(), 1);
    assert_eq!(
        db::invoice(&conn, issued.id.unwrap())
            .unwrap()
            .data
            .customer
            .unwrap()["name"],
        f.customer["name"]
    );
    let copy = f.app.copy_invoice(&mut conn, issued.id.unwrap()).unwrap();
    assert!(copy.data.customer_id.is_none());
    f.app
        .delete_invoice(&mut conn, copy.id.unwrap(), copy.version)
        .unwrap();
    error(db::invoice(&conn, copy.id.unwrap()), 404);
}
#[test]
fn failed_issue_does_not_consume_number_and_years_are_independent() {
    let f = fixture();
    let mut conn = db::connect(&f.app.database).unwrap();
    let mut draft = draft(&f);
    draft.data.items[0].description.clear();
    let invalid = f
        .app
        .save_invoice(&mut conn, &serde_json::to_value(&draft).unwrap())
        .unwrap();
    error(
        f.app
            .issue_invoice(&mut conn, invalid.id.unwrap(), invalid.version),
        422,
    );
    assert_eq!(
        conn.query_row(
            "SELECT last_number FROM sequences WHERE year=2026",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        13
    );
    let mut new = f.app.blank(&conn).unwrap();
    new.data.customer_id = f.customer["id"].as_i64();
    new.data.items = draft.data.items;
    new.data.items[0].description = "Next year".into();
    new.data.issue_date = "2027-01-01".into();
    new.data.delivery_date = "2027-01-01".into();
    new.data.due_date = "2027-01-15".into();
    let new = f
        .app
        .save_invoice(&mut conn, &serde_json::to_value(new).unwrap())
        .unwrap();
    assert_eq!(issue(&f, &new).number.as_deref(), Some("20270001"));
}
#[test]
fn concurrent_issuance_is_idempotent_and_numbers_are_unique() {
    let f = fixture();
    let first = draft(&f);
    let second = draft(&f);
    let threads = [first.clone(), second, first]
        .into_iter()
        .map(|invoice| {
            let app = f.app.clone();
            std::thread::spawn(move || {
                let mut conn = db::connect(&app.database).unwrap();
                app.issue_invoice(&mut conn, invoice.id.unwrap(), invoice.version)
                    .unwrap()
                    .number
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();
    let numbers = threads
        .into_iter()
        .map(|t| t.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(numbers[0], numbers[2]);
    assert_ne!(numbers[0], numbers[1]);
}
#[test]
fn imported_binary_pdf_with_text_storage_class_is_preserved() {
    let f = fixture();
    let draft = draft(&f);
    let conn = db::connect(&f.app.database).unwrap();
    let bytes = b"%PDF-1.4\n\xff\0legacy";
    conn.execute(
        "UPDATE invoices SET status='issued',number='20260014',pdf=CAST(?1 AS TEXT) WHERE id=?2",
        params![bytes.as_slice(), draft.id],
    )
    .unwrap();
    assert_eq!(f.app.invoice_pdf(&conn, draft.id.unwrap()).unwrap(), bytes);
}
#[test]
fn payment_qr_payloads_decode_independently() {
    let f = fixture();
    let mut invoice = draft(&f);
    invoice.number = Some("20260014".into());
    assert!(
        qr::payload(&invoice, "cz")
            .unwrap()
            .contains("AM:3542.40*CC:EUR")
    );
    assert_eq!(
        qr::payload(&invoice, "epc").unwrap().lines().nth(7),
        Some("EUR3542.40")
    );
    let encoded = qr::payload(&invoice, "sk").unwrap();
    let mut data = Vec::new();
    let mut buffer = 0_u32;
    let mut bits = 0;
    for c in encoded.bytes() {
        let value = if c.is_ascii_digit() {
            c - b'0'
        } else {
            c - b'A' + 10
        };
        buffer = (buffer << 5) | u32::from(value);
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            data.push((buffer >> bits) as u8);
        }
    }
    assert_eq!(&data[..2], &[0, 0]);
    let length = u16::from_le_bytes([data[2], data[3]]) as u64;
    let mut lzma = vec![0x5d, 0, 0, 2, 0];
    lzma.extend_from_slice(&length.to_le_bytes());
    lzma.extend_from_slice(&data[4..]);
    let mut decoded = Vec::new();
    use std::io::Read;
    xz2::read::XzDecoder::new_stream(
        &lzma[..],
        xz2::stream::Stream::new_lzma_decoder(1 << 24).unwrap(),
    )
    .read_to_end(&mut decoded)
    .unwrap();
    assert_eq!(decoded.len(), length as usize);
    assert_eq!(
        u32::from_le_bytes(decoded[..4].try_into().unwrap()),
        crc32fast::hash(&decoded[4..])
    );
    let text = String::from_utf8(decoded[4..].to_vec()).unwrap();
    let fields = text.split('\t').collect::<Vec<_>>();
    assert_eq!(fields.len(), 19);
    assert_eq!(fields[3], "3542.40");
    assert_eq!(fields[5], "");
    assert_eq!(fields[6], "20260014");
    assert_eq!(fields[12], "SK5000000000000000000001");
    assert_eq!(fields[16], "Test dodávateľ s. r. o.");
}
#[test]
fn smtp_password_lifecycle_and_validation() {
    let f = fixture();
    let mut conn = db::connect(&f.app.database).unwrap();
    let settings = smtp::settings(&conn).unwrap();
    assert_eq!(settings["version"], 0);
    let mut input = json!({"host":"mailpit","port":1025,"encryption":"none","username":"mailer","password":"secret-fixture","fromEmail":"billing@example.test","fromName":"Test","replyTo":"reply@example.test","version":0});
    let first = smtp::save_settings(&mut conn, &input).unwrap();
    assert!(first.get("password").is_none());
    assert_eq!(first["passwordSet"], true);
    error(smtp::save_settings(&mut conn, &input), 409);
    input["version"] = first["version"].clone();
    input["password"] = "".into();
    let second = smtp::save_settings(&mut conn, &input).unwrap();
    assert_eq!(
        smtp::connection(&conn).unwrap()["password"],
        "secret-fixture"
    );
    input["version"] = second["version"].clone();
    input["host"] = "new-host".into();
    error(smtp::save_settings(&mut conn, &input), 422);
    input["host"] = "https://bad.example/path".into();
    error(smtp::save_settings(&mut conn, &input), 422);
    input["host"] = "mailpit".into();
    input["fromName"] = "Injected\r\nBCC: victim@example.test".into();
    error(smtp::save_settings(&mut conn, &input), 422);
}

#[test]
fn php_email_request_hash_survives_migration_without_resending() {
    let f = fixture();
    let mut conn = db::connect(&f.app.database).unwrap();
    // Generated by the original PHP JSON_UNESCAPED_UNICODE|JSON_UNESCAPED_SLASHES contract.
    let hash = "125d576b5a8022a1d2c2c3a1854e13f7f901f0fb47f1be689652c81d6c183313";
    let key = "abababababababababababababababab";
    conn.execute("INSERT INTO email_deliveries(request_key,payload_hash,invoice_id,invoice_version,recipient,subject,status,created_at) VALUES(?1,?2,99,2,'recipient@example.test','Faktúra č. 20260014','sent','2026-10-15T10:00:00+02:00')",params![key,hash]).unwrap();
    let input = json!({"id":99,"version":2,"smtpVersion":1,"requestKey":key,"to":"recipient@example.test","subject":"Faktúra č. 20260014","message":"Dobrý deň,\nÚhrada & test."});
    let result = f.app.send_email(&mut conn, &input, false).unwrap();
    assert_eq!(result["status"], "sent");
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM email_deliveries", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    let mut changed = input;
    changed["to"] = "other@example.test".into();
    error(f.app.send_email(&mut conn, &changed, false), 409);
}

#[test]
fn long_invoice_pdf_preserves_every_row_and_metadata() {
    let f = fixture();
    let mut conn = db::connect(&f.app.database).unwrap();
    let mut invoice = draft(&f);
    invoice.data.note = "Začiatok poznámky\nSlovenské znaky čšťžýáíé.\nKoniec poznámky.".into();
    invoice.data.items = (1..=60)
        .map(|i| Item {
            description: format!(
                "Položka {i:02} — Vývoj a testovanie slovenských znakov čšťžýáíé. {}",
                "Podrobný opis prác a výsledkov. ".repeat(5)
            ),
            quantity: "2.125".into(),
            unit: "hod".into(),
            unit_price: "15.50".into(),
            vat_rate: if i % 2 == 0 { 19 } else { 23 },
        })
        .collect();
    invoice.data.qr_profiles = vec!["sk".into(), "cz".into()];
    let saved = f
        .app
        .save_invoice(&mut conn, &serde_json::to_value(&invoice).unwrap())
        .unwrap();
    let issued = issue(&f, &saved);
    let pdf = f.app.invoice_pdf(&conn, issued.id.unwrap()).unwrap();
    let document = lopdf::Document::load_mem(&pdf).unwrap();
    let pages = document.get_pages().keys().copied().collect::<Vec<_>>();
    assert!(pages.len() > 1);
    let text = document.extract_text(&pages).unwrap();
    for i in 1..=60 {
        assert!(text.contains(&format!("Položka {i:02}")), "missing row {i}");
    }
    assert!(text.contains("Koniec poznámky."));
    let info = document
        .trailer
        .get(b"Info")
        .unwrap()
        .as_reference()
        .unwrap();
    let info = document.get_object(info).unwrap().as_dict().unwrap();
    assert_eq!(
        lopdf::decode_text_string(info.get(b"Author").unwrap()).unwrap(),
        "Test dodávateľ s. r. o."
    );
    if let Ok(path) = std::env::var("FAKTURACIE_ARTIFACTS") {
        let path = PathBuf::from(path);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("invoice-multipage.pdf"), pdf).unwrap();
        let mut single = issued;
        single.data.items = vec![Item {
            description: "Služby — slovenské znaky čšťžýáíé".into(),
            quantity: "1.000".into(),
            unit: "ks".into(),
            unit_price: "2880.00".into(),
            vat_rate: 23,
        }];
        single.data.totals = rules::totals(&single.data.items).unwrap();
        single.data.qr_profiles = vec!["sk".into(), "epc".into()];
        std::fs::write(
            path.join("invoice-sk-epc.pdf"),
            f.app.render_pdf(&single, false).unwrap(),
        )
        .unwrap();
        let mut payloads = json!({});
        for profile in ["sk", "cz", "epc"] {
            single.data.qr_profiles = vec![profile.into()];
            let image = qr::images(&single).unwrap().remove(0);
            use base64::Engine;
            let png = base64::engine::general_purpose::STANDARD
                .decode(image.data_uri.split_once(',').unwrap().1)
                .unwrap();
            std::fs::write(path.join(format!("qr-{profile}.png")), png).unwrap();
            payloads[profile] = qr::payload(&single, profile).unwrap().into();
        }
        std::fs::write(path.join("qr-payloads.json"), payloads.to_string()).unwrap();
    }
}
