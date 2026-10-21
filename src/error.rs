use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

#[derive(Debug)]
pub struct Problem {
    pub status: u16,
    pub message: String,
}

pub type Result<T> = std::result::Result<T, Problem>;

impl Problem {
    pub fn new(message: impl Into<String>) -> Self {
        Self::status(422, message)
    }
    pub fn status(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
    pub fn internal() -> Self {
        Self::status(
            500,
            "Operáciu sa nepodarilo dokončiť. Údaje v editore zostali zachované; skús to znova.",
        )
    }
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Problem {}
impl From<rusqlite::Error> for Problem {
    fn from(_: rusqlite::Error) -> Self {
        Self::internal()
    }
}
impl From<serde_json::Error> for Problem {
    fn from(_: serde_json::Error) -> Self {
        Self::internal()
    }
}
impl From<std::io::Error> for Problem {
    fn from(_: std::io::Error) -> Self {
        Self::internal()
    }
}
impl From<minijinja::Error> for Problem {
    fn from(_: minijinja::Error) -> Self {
        Self::internal()
    }
}
impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        (
            StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(json!({"ok":false,"error":self.message,"fields":{}})),
        )
            .into_response()
    }
}
