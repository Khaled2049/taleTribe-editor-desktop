use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::error::AppResult;

/// Ordered and append-only: a shipped migration is never edited, only followed.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_init.sql"),
    include_str!("../migrations/0002_settings.sql"),
    include_str!("../migrations/0003_publish.sql"),
];

pub fn open(path: &Path) -> AppResult<Connection> {
    let mut conn = Connection::open(path)?;
    // WAL keeps the last committed version intact if the app dies mid-write.
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "FULL")?;
    configure(&mut conn)?;
    Ok(conn)
}

#[cfg(test)]
pub fn open_in_memory() -> AppResult<Connection> {
    let mut conn = Connection::open_in_memory()?;
    configure(&mut conn)?;
    Ok(conn)
}

fn configure(conn: &mut Connection) -> AppResult<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(conn)
}

/// Applies each pending migration in its own transaction, tracked by `user_version`.
fn migrate(conn: &mut Connection) -> AppResult<()> {
    let applied: usize = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(applied) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", index + 1)?;
        tx.commit()?;
    }
    Ok(())
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_once_and_reopen_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        drop(open(&path).unwrap());
        let conn = open(&path).unwrap();
        let version: usize = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len());
    }
}
