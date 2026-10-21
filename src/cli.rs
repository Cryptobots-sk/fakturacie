use crate::{
    db,
    error::{Problem, Result},
    random_key,
};
use serde_json::json;
use std::time::Duration;
fn inspect(path: &std::path::Path) -> Result<rusqlite::Connection> {
    let db =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let integrity: String = db.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if integrity != "ok" {
        return Err(Problem::new("Databáza neprešla kontrolou integrity."));
    }
    for table in ["settings", "customers", "invoices", "sequences"] {
        db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| {
            r.get::<_, i64>(0)
        })?;
    }
    // The original importer accepted backups made before SMTP was added.
    // Startup creates missing optional tables after import, without changing the core data.
    db::settings(&db)?;
    Ok(db)
}
pub fn database_command(path: &std::path::Path, command: &str) -> Result<()> {
    use std::io::Write;
    match command {
        "export" => {
            let db = inspect(path)?;
            let directory = tempfile::tempdir()?;
            // VACUUM INTO creates a new file inside a private directory, also when run via exec.
            let snapshot = directory.path().join("snapshot.sqlite");
            db.execute("VACUUM INTO ?1", [snapshot.to_string_lossy().as_ref()])?;
            std::io::copy(
                &mut std::fs::File::open(&snapshot)?,
                &mut std::io::stdout().lock(),
            )?;
            Ok(())
        }
        "import" => {
            if path.exists() {
                return Err(Problem::new(
                    "Úložisko už obsahuje databázu; import ju neprepíše.",
                ));
            }
            let parent = path.parent().ok_or_else(Problem::internal)?;
            std::fs::create_dir_all(parent)?;
            let mut file = tempfile::NamedTempFile::new_in(parent)?;
            std::io::copy(&mut std::io::stdin().lock(), file.as_file_mut())?;
            file.as_file_mut().flush()?;
            drop(inspect(file.path())?);
            file.persist_noclobber(path)
                .map_err(|_| Problem::internal())?;
            println!("Databáza importovaná a overená.");
            Ok(())
        }
        "manifest" => {
            use sha2::{Digest, Sha256};
            let db = inspect(path)?;
            let mut result = json!({});
            for table in [
                "settings",
                "customers",
                "invoices",
                "sequences",
                "smtp_settings",
                "email_deliveries",
            ] {
                let mut statement = db.prepare(&format!("SELECT * FROM {table} ORDER BY 1"))?;
                let width = statement.column_count();
                let mut rows = statement.query([])?;
                let mut digest = Sha256::new();
                let mut count = 0;
                while let Some(row) = rows.next()? {
                    for index in 0..width {
                        let value = row.get_ref(index)?;
                        let (tag, bytes) = match value {
                            rusqlite::types::ValueRef::Null => (0, vec![]),
                            rusqlite::types::ValueRef::Integer(v) => (1, v.to_le_bytes().to_vec()),
                            rusqlite::types::ValueRef::Real(v) => (2, v.to_le_bytes().to_vec()),
                            rusqlite::types::ValueRef::Text(v) => (3, v.to_vec()),
                            rusqlite::types::ValueRef::Blob(v) => (4, v.to_vec()),
                        };
                        digest.update([tag]);
                        digest.update((bytes.len() as u64).to_le_bytes());
                        digest.update(bytes);
                    }
                    count += 1;
                }
                result[table] = json!({"count":count,"sha256":hex::encode(digest.finalize())});
            }
            println!("{}", serde_json::to_string_pretty(&result)?);
            Ok(())
        }
        _ => Err(Problem::new(
            "Použitie: fakturacie database import|export|manifest",
        )),
    }
}
pub fn backup(path: &std::path::Path) -> Result<()> {
    let db = inspect(path)?;
    let directory = path.parent().ok_or_else(Problem::internal)?.join("backups");
    std::fs::create_dir_all(&directory)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
    }
    let output = directory.join(format!(
        "fakturacie-{}-{}.sqlite",
        chrono::Utc::now().format("%Y%m%d-%H%M%S"),
        random_key(4)
    ));
    db.execute("VACUUM INTO ?1", [output.to_string_lossy().as_ref()])?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&output, std::fs::Permissions::from_mode(0o600))?;
    }
    println!("{}", output.display());
    Ok(())
}
pub fn healthcheck() -> Result<()> {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect_timeout(
        &"127.0.0.1:8080".parse().map_err(|_| Problem::internal())?,
        Duration::from_secs(3),
    )?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    if !response.starts_with("HTTP/1.1 200 ") {
        return Err(Problem::internal());
    }
    Ok(())
}
