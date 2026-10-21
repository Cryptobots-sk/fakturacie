pub mod cli;
pub mod db;
pub mod error;
pub mod invoices;
pub mod model;
pub mod qr;
pub mod render;
pub mod rules;
pub mod smtp;
pub mod web;

use crate::{error::Result, render::Views};
use std::path::{Path, PathBuf};

pub struct Backend {
    pub database: PathBuf,
    pub root: PathBuf,
    pub views: Views,
}

impl Backend {
    pub fn new(root: &Path, database: &Path) -> Result<Self> {
        Ok(Self {
            database: database.to_owned(),
            root: root.to_owned(),
            views: Views::new(root)?,
        })
    }
}

pub fn random_key(bytes: usize) -> String {
    use rand::RngCore;
    let mut key = vec![0; bytes];
    rand::rng().fill_bytes(&mut key);
    hex::encode(key)
}
pub fn now() -> String {
    chrono::Utc::now()
        .with_timezone(&chrono_tz::Europe::Bratislava)
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}
