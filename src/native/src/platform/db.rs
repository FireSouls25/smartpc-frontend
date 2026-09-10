//! Shared SQLite setup: open the file and enforce foreign keys.
//! Schema lives with each domain (e.g. auth/schema.sql), applied by its store.
use rusqlite::Connection;

pub fn connect(db_path: &str) -> rusqlite::Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    Ok(conn)
}
