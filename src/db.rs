use rusqlite::Connection;

pub fn create_db() -> Result<Connection, ()> {
    // Connect to the SQLite database
    let conn = Connection::open("hashes.db").map_err(|_| ())?;

    conn.execute(
        r#"
    CREATE TABLE IF NOT EXISTS hashes (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        chat_id INTEGER NOT NULL,
        message_id INTEGER NOT NULL,
        filename TEXT NOT NULL,
        file_id TEXT NOT NULL,
        orientation TEXT CHECK(orientation IN ('portrait', 'landscape', 'square')) NOT NULL,
        base64_hash TEXT NOT NULL,   -- The original base64 encoded hash for reference
        created_at INTEGER NOT NULL, -- unixtime timestamp
        UNIQUE(id, orientation) -- Ensure one hash per orientation per image
    );

    -- Index on hash_data to speed up searches (optional but recommended for large datasets)
    CREATE INDEX idx_hash_data ON hashes(hash_data);
        "#,
        [],
    )
    .map_err(|e| {
        eprintln!("Create error {}", e);
        ()
    })?;
    Ok(conn)
}
