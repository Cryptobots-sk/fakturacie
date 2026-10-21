use crate::{
    Backend,
    error::{Problem, Result},
    model::Invoice,
    qr, rules,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use minijinja::{Environment, Value as TemplateValue};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use tungstenite::{Message, WebSocket, stream::MaybeTlsStream};

pub struct Views {
    environment: Environment<'static>,
}
impl Views {
    pub fn new(root: &Path) -> Result<Self> {
        let mut env = Environment::new();
        env.set_loader(minijinja::path_loader(root.join("views")));
        env.add_filter("money", rules::money);
        env.add_filter("date", |date: String| {
            if date.is_empty() {
                return "—".to_owned();
            }
            chrono::DateTime::parse_from_rfc3339(&date)
                .map(|d| {
                    d.with_timezone(&chrono_tz::Europe::Bratislava)
                        .format("%d.%m.%Y %H:%M")
                        .to_string()
                })
                .or_else(|_| {
                    chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
                        .map(|d| d.format("%d.%m.%Y").to_string())
                })
                .unwrap_or(date)
        });
        env.add_filter("quantity", |quantity: String| {
            quantity
                .trim_end_matches('0')
                .trim_end_matches('.')
                .replace('.', ",")
        });
        env.add_filter("price", |price: String| {
            rules::decimal(&json!(price), 2, "Cena", 9_999_999_999)
                .map_or_else(|_| String::new(), |(_, v)| rules::money(v))
        });
        env.add_filter("iban", |iban: String| {
            iban.as_bytes()
                .chunks(4)
                .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
                .collect::<Vec<_>>()
                .join(" ")
        });
        env.add_filter("lines", |value: String| {
            TemplateValue::from_safe_string(html_escape(&value).replace('\n', "<br>"))
        });
        // Fail at startup for an incomplete template port, not on the first invoice.
        for template in [
            "layout.html",
            "home.html",
            "invoice.html",
            "customers.html",
            "settings.html",
            "pdf.html",
            "email.html",
            "document/invoice.html",
            "document/item.html",
            "document/field.html",
            "partials/icons.html",
            "partials/party-fields.html",
            "partials/customer-dialog.html",
            "partials/email-dialog.html",
        ] {
            env.get_template(template).map_err(|e| {
                eprintln!("Šablóna {template}: {e}");
                Problem::internal()
            })?;
        }
        Ok(Self { environment: env })
    }
    pub fn render(&self, name: &str, context: &Value) -> Result<String> {
        self.environment
            .get_template(name)?
            .render(context)
            .map_err(|e| {
                eprintln!("Šablóna {name}: {e}");
                Problem::internal()
            })
    }
}
pub fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
/// JSON is embedded in a non-executable script element. Escape HTML delimiters as PHP did.
pub fn boot_json(value: &Value) -> String {
    value
        .to_string()
        .replace('<', "\\u003C")
        .replace('>', "\\u003E")
        .replace('&', "\\u0026")
        .replace('\'', "\\u0027")
}

struct PdfProcess(Child);
impl Drop for PdfProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Cdp {
    socket: WebSocket<MaybeTlsStream<std::net::TcpStream>>,
    next_id: u64,
    session: Option<String>,
    loaded: bool,
    deadline: Instant,
}
impl Cdp {
    fn receive(&mut self) -> Result<Value> {
        loop {
            let remaining = self
                .deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(Problem::internal)?;
            if let MaybeTlsStream::Plain(stream) = self.socket.get_ref() {
                stream.set_read_timeout(Some(remaining))?;
            }
            match self.socket.read().map_err(|_| Problem::internal())? {
                Message::Text(text) => {
                    let value: Value = serde_json::from_str(&text)?;
                    if value["method"] == "Page.loadEventFired"
                        && value["sessionId"].as_str() == self.session.as_deref()
                    {
                        self.loaded = true;
                    }
                    return Ok(value);
                }
                Message::Close(_) => return Err(Problem::internal()),
                _ => {}
            }
        }
    }
    fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        self.next_id += 1;
        let id = self.next_id;
        let mut message = json!({"id":id,"method":method,"params":params});
        if let Some(session) = &self.session {
            message["sessionId"] = session.clone().into();
        }
        self.socket
            .send(Message::Text(message.to_string().into()))
            .map_err(|_| Problem::internal())?;
        loop {
            let response = self.receive()?;
            if response["id"].as_u64() == Some(id) {
                if response.get("error").is_some() {
                    eprintln!("PDF: zlyhalo {method}");
                    return Err(Problem::internal());
                }
                return Ok(response["result"].clone());
            }
        }
    }
}

impl Backend {
    pub fn render_pdf(&self, invoice: &Invoice, draft: bool) -> Result<Vec<u8>> {
        let directory = tempfile::Builder::new()
            .prefix("fakturacie-pdf-")
            .tempdir()?;
        let images = if draft { vec![] } else { qr::images(invoice)? };
        let css = std::fs::read_to_string(self.root.join("assets/invoice.css"))?;
        let fonts = self.root.join("public/assets/fonts");
        let title = if draft {
            "Koncept faktúry".into()
        } else {
            format!("Faktúra {}", invoice.number.as_deref().unwrap_or(""))
        };
        let labels: Value = rules::QR_LABELS
            .into_iter()
            .map(|(k, v)| (k.to_owned(), Value::from(v)))
            .collect();
        let html=self.views.render("pdf.html",&json!({"invoice":invoice,"interactive":false,"draft":draft,"qrImages":images,"qrPlaceholder":if invoice.number.is_none() {qr::placeholder(&self.root,"QR po vystavení")?} else {String::new()},"qrLabels":labels,"documentCss":css,"fontPath":fonts.to_string_lossy(),"title":title}))?;
        let path = directory.path().join("invoice.html");
        std::fs::File::create(&path)?.write_all(html.as_bytes())?;
        let profile = directory.path().join("profile");
        let child = Command::new(
            std::env::var("CHROMIUM_BIN").unwrap_or_else(|_| "/usr/bin/chromium".into()),
        )
        .args([
            "--headless",
            "--no-sandbox",
            "--disable-dev-shm-usage",
            "--disable-gpu",
            "--disable-background-networking",
            "--disable-component-update",
            "--disable-extensions",
            "--no-first-run",
            "--no-default-browser-check",
            "--remote-debugging-port=0",
            "--remote-debugging-address=127.0.0.1",
        ])
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg("about:blank")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
        let mut process = PdfProcess(child);
        let started = Instant::now();
        let endpoint = loop {
            if let Ok(active) = std::fs::read_to_string(profile.join("DevToolsActivePort")) {
                let mut lines = active.lines();
                if let (Some(port), Some(path)) = (lines.next(), lines.next()) {
                    break format!("ws://127.0.0.1:{port}{path}");
                }
            }
            if started.elapsed() > Duration::from_secs(15) || process.0.try_wait()?.is_some() {
                return Err(Problem::internal());
            }
            std::thread::sleep(Duration::from_millis(30));
        };
        let (socket, _) = tungstenite::connect(&endpoint).map_err(|_| Problem::internal())?;
        if let MaybeTlsStream::Plain(stream) = socket.get_ref() {
            stream.set_read_timeout(Some(Duration::from_secs(30)))?;
            stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        }
        let mut cdp = Cdp {
            socket,
            next_id: 0,
            session: None,
            loaded: false,
            deadline: Instant::now() + Duration::from_secs(30),
        };
        let target = cdp.call("Target.createTarget", json!({"url":"about:blank"}))?;
        let attached = cdp.call(
            "Target.attachToTarget",
            json!({"targetId":target["targetId"],"flatten":true}),
        )?;
        cdp.session = attached["sessionId"].as_str().map(str::to_owned);
        cdp.call("Network.enable", json!({}))?;
        // Render only our escaped HTML, data images and local bundled fonts; no outbound invoice data.
        cdp.call(
            "Network.setBlockedURLs",
            json!({"urls":["http://*","https://*","ftp://*"]}),
        )?;
        cdp.call("Page.enable", json!({}))?;
        cdp.loaded = false;
        cdp.call(
            "Page.navigate",
            json!({"url":format!("file://{}",path.display())}),
        )?;
        while !cdp.loaded {
            cdp.receive()?;
        }
        let ready=cdp.call("Runtime.evaluate",json!({"expression":"(async () => { if (!document.querySelector('.invoice-document')) throw new Error('Missing document'); await document.fonts.ready; await Promise.all([...document.images].map(i => i.decode())); return true; })()","awaitPromise":true,"returnByValue":true}))?;
        if ready.get("exceptionDetails").is_some() {
            return Err(Problem::internal());
        }
        let footer = format!(
            "<div style=\"font-family:DejaVu Sans,sans-serif;font-size:9.333px;color:#526779;width:100%;text-align:center\">{} · {} | <span class=\"pageNumber\"></span>/<span class=\"totalPages\"></span></div>",
            html_escape(crate::model::string(&invoice.data.supplier, "name")),
            if draft {
                "KONCEPT".into()
            } else {
                format!("Faktúra {}", invoice.number.as_deref().unwrap_or(""))
            }
        );
        let pdf=cdp.call("Page.printToPDF",json!({"printBackground":true,"paperWidth":210.0/25.4,"paperHeight":297.0/25.4,"marginLeft":17.0/25.4,"marginRight":17.0/25.4,"marginTop":18.0/25.4,"marginBottom":18.0/25.4,"displayHeaderFooter":true,"headerTemplate":"<span></span>","footerTemplate":footer,"preferCSSPageSize":true}))?;
        let bytes = STANDARD
            .decode(pdf["data"].as_str().ok_or_else(Problem::internal)?)
            .map_err(|_| Problem::internal())?;
        if !bytes.starts_with(b"%PDF") {
            return Err(Problem::internal());
        }
        // Close the browser normally; the child guard remains a fallback for failed renders.
        cdp.session = None;
        let _ = cdp.socket.send(Message::Text(
            json!({"id":cdp.next_id+1,"method":"Browser.close"})
                .to_string()
                .into(),
        ));
        for _ in 0..10 {
            if process.0.try_wait()?.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let mut document = lopdf::Document::load_mem(&bytes).map_err(|_| Problem::internal())?;
        let mut info = document
            .trailer
            .get(b"Info")
            .and_then(lopdf::Object::as_reference)
            .ok()
            .and_then(|id| document.get_object(id).ok())
            .and_then(|object| object.as_dict().ok())
            .cloned()
            .unwrap_or_default();
        info.set(
            "Author",
            lopdf::text_string(crate::model::string(&invoice.data.supplier, "name")),
        );
        info.set("Title", lopdf::text_string(&title));
        let info_id = document.add_object(info);
        document.trailer.set("Info", info_id);
        let mut output = Vec::new();
        document.save_to(&mut output)?;
        Ok(output)
    }
}
