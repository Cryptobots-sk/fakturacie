use crate::{
    Backend, db,
    error::{Problem, Result},
    qr, random_key,
    render::boot_json,
    rules, smtp,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{ConnectInfo, Path, Query, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{any, get},
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
use tower_http::services::ServeDir;

struct Session {
    token: String,
    expires: Instant,
}
#[derive(Clone)]
pub struct WebState {
    pub backend: Arc<Backend>,
    sessions: Arc<Mutex<HashMap<String, Session>>>,
}
impl WebState {
    pub fn new(backend: Backend) -> Self {
        Self {
            backend: Arc::new(backend),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    fn token(&self, headers: &HeaderMap, create: bool) -> Result<(String, Option<String>)> {
        let id = headers
            .get("cookie")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| {
                h.split(';')
                    .find_map(|p| p.trim().strip_prefix("fakturacie_session="))
            });
        let mut sessions = self.sessions.lock().map_err(|_| Problem::internal())?;
        if let Some(session) = id
            .and_then(|id| sessions.get(id))
            .filter(|s| s.expires > Instant::now())
        {
            return Ok((session.token.clone(), None));
        }
        if !create {
            return Err(Problem::status(
                403,
                "Platnosť stránky vypršala. Obnov ju a skús znovu.",
            ));
        }
        sessions.retain(|_, s| s.expires > Instant::now());
        let id = random_key(32);
        let token = random_key(32);
        sessions.insert(
            id.clone(),
            Session {
                token: token.clone(),
                expires: Instant::now() + Duration::from_secs(14 * 86400),
            },
        );
        Ok((
            token,
            Some(format!(
                "fakturacie_session={id}; Path=/; HttpOnly; SameSite=Strict; Max-Age=1209600"
            )),
        ))
    }
}
pub fn router(state: WebState) -> Router {
    let assets = state.backend.root.join("public/assets");
    Router::new()
        .route("/", get(home))
        .route("/zakaznici", get(customers))
        .route("/nastavenia", get(settings))
        .route("/faktury/nova", get(new_invoice))
        .route("/faktury/{id}", get(invoice))
        .route("/faktury/{id}/pdf", get(pdf))
        .route("/api/{action}", any(api))
        .route("/healthz", get(health))
        .nest_service(
            "/assets",
            ServeDir::new(assets).append_index_html_on_directories(false),
        )
        .fallback(|| async { Problem::status(404, "Stránka sa nenašla.") })
        .layer(middleware::from_fn_with_state(state.clone(), boundary))
        .with_state(state)
}
async fn boundary(State(_state): State<WebState>, request: Request, next: Next) -> Response {
    let host = request
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.parse::<axum::http::uri::Authority>().ok())
        .map(|h| h.host().to_ascii_lowercase());
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|p| p.0.ip());
    let docker = std::env::var("FAKTURACIE_RUNTIME").as_deref() == Ok("docker");
    let trusted_host = host
        .as_deref()
        .is_some_and(|h| ["127.0.0.1", "localhost", "[::1]", "::1", "faktura.local"].contains(&h));
    let mut response = if !trusted_host || (!docker && !peer.is_some_and(|p| p.is_loopback())) {
        (
            StatusCode::FORBIDDEN,
            "Aplikácia je dostupná iba na tomto počítači.",
        )
            .into_response()
    } else {
        next.run(request).await
    };
    for (name, value) in [
        (
            "content-security-policy",
            "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
        ),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        ("cache-control", "no-store"),
    ] {
        response.headers_mut().insert(
            axum::http::HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    response
}
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| Problem::internal())?
}
fn labels() -> Value {
    rules::QR_LABELS
        .into_iter()
        .map(|(k, v)| (k.to_owned(), Value::from(v)))
        .collect()
}
fn base_context(csrf: &str) -> Value {
    json!({"csrf":csrf,"title":"Faktúry","page":"invoices","basePath":"","qrLabels":labels()})
}
fn page_response(html: String, cookie: Option<String>) -> Result<Response> {
    let mut response = Html(html).into_response();
    if let Some(cookie) = cookie {
        response.headers_mut().insert(
            "set-cookie",
            cookie.parse().map_err(|_| Problem::internal())?,
        );
    }
    Ok(response)
}
async fn home(State(state): State<WebState>, headers: HeaderMap) -> Result<Response> {
    let (token, cookie) = state.token(&headers, true)?;
    let html = blocking(move || {
        let db = db::connect(&state.backend.database)?;
        let mut context = base_context(&token);
        context["invoices"] = serde_json::to_value(db::invoices(&db)?)?;
        context["bootJson"] = boot_json(&json!({"page":"invoices"})).into();
        state.backend.views.render("home.html", &context)
    })
    .await?;
    page_response(html, cookie)
}
async fn customers(
    State(state): State<WebState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response> {
    let (token, cookie) = state.token(&headers, true)?;
    let archived = query
        .get("archived")
        .is_some_and(|v| v == "1" || v == "true");
    let html = blocking(move || {
        let db = db::connect(&state.backend.database)?;
        let customers = db::customers(&db, archived)?;
        let mut context = base_context(&token);
        context["title"] = "Zákazníci".into();
        context["page"] = "customers".into();
        context["archived"] = archived.into();
        context["customers"] = serde_json::to_value(&customers)?;
        context["bootJson"] =
            boot_json(&json!({"page":"customers","customers":customers,"archived":archived}))
                .into();
        state.backend.views.render("customers.html", &context)
    })
    .await?;
    page_response(html, cookie)
}
async fn settings(State(state): State<WebState>, headers: HeaderMap) -> Result<Response> {
    let (token, cookie) = state.token(&headers, true)?;
    let html=blocking(move || {let db=db::connect(&state.backend.database)?;let mut context=base_context(&token);
        context["title"]="Moja firma".into();context["page"]="settings".into();
        context["bootJson"]=boot_json(&json!({"page":"settings","supplier":db::settings(&db)?,"smtp":smtp::settings(&db)?,"smtpTestKey":random_key(16)})).into();state.backend.views.render("settings.html",&context)}).await?;
    page_response(html, cookie)
}
async fn new_invoice(State(state): State<WebState>, headers: HeaderMap) -> Result<Response> {
    invoice_page(state, headers, None, false).await
}
async fn invoice(
    State(state): State<WebState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response> {
    let id = id
        .parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| Problem::status(404, "Faktúra sa nenašla."))?;
    invoice_page(
        state,
        headers,
        Some(id),
        query.get("edit").is_some_and(|v| v == "1" || v == "true"),
    )
    .await
}
async fn invoice_page(
    state: WebState,
    headers: HeaderMap,
    id: Option<i64>,
    edit: bool,
) -> Result<Response> {
    let (token, cookie) = state.token(&headers, true)?;
    let html=blocking(move || {
        let db=db::connect(&state.backend.database)?;
        let invoice=match id {Some(id)=>db::invoice(&db,id)?,None=>state.backend.blank(&db)?};
        let editing=invoice.status=="draft" || edit;let customers=db::customers(&db,false)?;
        let images=if invoice.status=="issued" {qr::images(&invoice)?} else {vec![]};
        let placeholder=qr::placeholder(&state.backend.root,if invoice.status=="issued" {"QR po uložení"} else {"QR po vystavení"})?;
        let mut context=base_context(&token);
        context["title"]=invoice.number.as_ref().map_or_else(||"Nová faktúra".into(),|n|format!("Faktúra {n}")).into();
        context["invoice"]=serde_json::to_value(&invoice)?;context["editing"]=editing.into();context["customers"]=serde_json::to_value(&customers)?;
        context["qrImages"]=serde_json::to_value(&images)?;context["qrPlaceholder"]=placeholder.clone().into();
        context["deliveries"]=serde_json::to_value(id.map_or_else(||Ok(vec![]),|id|smtp::history(&db,id))?)?;
        context["bootJson"]=boot_json(&json!({"page":"editor","invoice":invoice,"editing":editing,"customers":customers,"qrImages":images,"qrPlaceholder":placeholder})).into();
        state.backend.views.render("invoice.html",&context)
    }).await?;
    page_response(html, cookie)
}
async fn pdf(
    State(state): State<WebState>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response> {
    let id = id
        .parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| Problem::status(404, "Faktúra sa nenašla."))?;
    let (invoice, pdf) = blocking(move || {
        let db = db::connect(&state.backend.database)?;
        Ok((db::invoice(&db, id)?, state.backend.invoice_pdf(&db, id)?))
    })
    .await?;
    let name = invoice
        .number
        .map_or_else(|| format!("Koncept_{id}"), |n| format!("Faktura_{n}"));
    let disposition = format!(
        "{}; filename=\"{name}.pdf\"",
        if query.get("download").is_some_and(|v| v == "1") {
            "attachment"
        } else {
            "inline"
        }
    );
    let length = pdf.len();
    let mut response = Body::from(pdf).into_response();
    response
        .headers_mut()
        .insert("content-type", HeaderValue::from_static("application/pdf"));
    response.headers_mut().insert(
        "content-disposition",
        disposition.parse().map_err(|_| Problem::internal())?,
    );
    response.headers_mut().insert(
        "content-length",
        length
            .to_string()
            .parse()
            .map_err(|_| Problem::internal())?,
    );
    Ok(response)
}
async fn api(
    State(state): State<WebState>,
    Path(action): Path<String>,
    request: Request,
) -> Result<Response> {
    if request.method() != axum::http::Method::POST {
        return Err(Problem::status(405, "Táto operácia vyžaduje POST."));
    }
    let headers = request.headers();
    let host = headers
        .get("host")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(Problem::internal)?;
    if headers
        .get("origin")
        .is_some_and(|h| h.to_str().ok() != Some(format!("http://{host}").as_str()))
        || headers
            .get("sec-fetch-site")
            .is_some_and(|h| h == "cross-site")
    {
        return Err(Problem::status(
            403,
            "Požiadavka z inej stránky bola odmietnutá.",
        ));
    }
    let (token, _) = state.token(headers, false)?;
    let provided = headers
        .get("x-csrf-token")
        .map_or(&[][..], |h| h.as_bytes());
    if !bool::from(token.as_bytes().ct_eq(provided)) {
        return Err(Problem::status(
            403,
            "Platnosť stránky vypršala. Obnov ju a skús znovu.",
        ));
    }
    if !headers
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| h.to_ascii_lowercase().starts_with("application/json"))
    {
        return Err(Problem::status(415, "Očakávam údaje vo formáte JSON."));
    }
    let bytes = to_bytes(request.into_body(), 300000)
        .await
        .map_err(|_| Problem::status(413, "Požiadavka je príliš veľká."))?;
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| Problem::new("Údaje sa nepodarilo načítať."))?;
    if !within_json_depth(&value, 1) {
        return Err(Problem::new("Údaje sa nepodarilo načítať."));
    }
    if !value.is_object() || value.as_object().is_some_and(|object| object.is_empty()) {
        return Err(Problem::new("Neplatná požiadavka."));
    }
    let output = blocking(move || state.backend.dispatch(&action, &value)).await?;
    Ok(axum::Json(json!({"ok":true,"data":output})).into_response())
}
fn within_json_depth(value: &Value, depth: usize) -> bool {
    if depth > 32 {
        return false;
    }
    match value {
        Value::Object(object) => object.values().all(|v| within_json_depth(v, depth + 1)),
        Value::Array(array) => array.iter().all(|v| within_json_depth(v, depth + 1)),
        _ => true,
    }
}
impl Backend {
    pub fn dispatch(&self, action: &str, input: &Value) -> Result<Value> {
        let mut db = db::connect(&self.database)?;
        match action {
            "customer-save" => db::save_customer(&mut db, input),
            "customer-archive" => db::archive_customer(&db, input),
            "settings-save" => db::save_settings(&db, input),
            "smtp-save" => smtp::save_settings(&mut db, input),
            "smtp-test" => self.send_email(&mut db, input, true),
            "invoice-email-preview" => self.email_preview(
                &db,
                rules::positive_id(input, "id")?,
                rules::positive_id(input, "version")?,
            ),
            "invoice-email-send" => self.send_email(&mut db, input, false),
            "invoice-save" => Ok(serde_json::to_value(self.save_invoice(&mut db, input)?)?),
            "invoice-issue" => Ok(serde_json::to_value(self.issue_invoice(
                &mut db,
                rules::positive_id(input, "id")?,
                rules::positive_id(input, "version")?,
            )?)?),
            "invoice-copy" => Ok(serde_json::to_value(
                self.copy_invoice(&mut db, rules::positive_id(input, "id")?)?,
            )?),
            "invoice-delete" => self.delete_invoice(
                &mut db,
                rules::positive_id(input, "id")?,
                rules::positive_id(input, "version")?,
            ),
            _ => Err(Problem::status(404, "Neznáma operácia.")),
        }
    }
}
async fn health(State(state): State<WebState>) -> Result<&'static str> {
    blocking(move || {
        let db = db::connect(&state.backend.database)?;
        db::settings(&db)?;
        Ok("ok")
    })
    .await
}
pub async fn run() -> Result<()> {
    let root = PathBuf::from(std::env::var("FAKTURACIE_ROOT").unwrap_or_else(|_| "/app".into()));
    let path = PathBuf::from(
        std::env::var("FAKTURACIE_DB")
            .unwrap_or_else(|_| "/var/www/html/data/fakturacie.sqlite".into()),
    );
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|a| a == "--version") {
        println!("Fakturácie {} · Rust backend", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "--healthcheck") {
        return crate::cli::healthcheck();
    }
    if args.first().is_some_and(|a| a == "database") {
        return crate::cli::database_command(&path, args.get(1).map(String::as_str).unwrap_or(""));
    }
    if args.first().is_some_and(|a| a == "backup") {
        return crate::cli::backup(&path);
    }
    db::initialize(&path)?;
    if args.first().is_some_and(|a| a == "setup") {
        println!("Pripravené. Pri prvom spustení vyplň údaje v časti Moja firma.");
        return Ok(());
    }
    let backend = Backend::new(&root, &path)?;
    let listen = std::env::var("FAKTURACIE_LISTEN").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let listener = tokio::net::TcpListener::bind(&listen).await?;
    println!("Fakturácie Rust {} · {listen}", env!("CARGO_PKG_VERSION"));
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    axum::serve(
        listener,
        router(WebState::new(backend)).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = terminate.recv() => {},
        }
    })
    .await?;
    Ok(())
}
