use image_hasher::ImageHash;
use rusqlite::{functions::{Context, FunctionFlags}, Connection};

pub fn create_db(path: &str) -> Result<Connection, ()> {
    // Connect to the SQLite database
    let conn = Connection::open(path).map_err(|_| ())?;

    // Register custom Hamming distance function
    conn.create_scalar_function(
        "hamming_distance",
        2,
        FunctionFlags::all(),
        move |ctx: &Context| {
            hamming_sqlite_func(ctx)
            //Ok(dist as i64)
        },
    )
    .map_err(|e| {
        eprintln!("Failed to register function {}", e);
        ()
    })?;
    /*
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

    /*
    CREATE TABLE IF NOT EXISTS votings (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        chat_id INTEGER NOT NULL,
        message_id INTEGER NOT NULL
        original_message_id INTEGER NOT NULL,
        voting_type TEXT CHECK(voting_type IN ('nondupes', 'ignore')) NOT NULL,
        UNIQUE(id)
    );

    CREATE TABLE IF NOT EXISTS votes (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        voting_id INTEGER NOT NULL,
        vote_type INTEGER NOT NULL,
        user_id INTEGER NOT NULL,
        username TEXT NOT NULL
    );
    */
        "#,
        [],
    )
    .map_err(|e| {
        eprintln!("Create db error {}", e);
        
    })?;
    */
    Ok(conn)
}


#[inline]
fn hamming_sqlite_func(ctx: &Context) -> Result<i64, rusqlite::Error> {
    let hash1: String = ctx.get(0)?;
    let hash2: String = ctx.get(1)?;
    let dist = hamming_distance(&hash1, &hash2);
    dist.map_err(|_| rusqlite::Error::InvalidQuery)
        .map(|x| x as i64)
}


fn hamming_distance(hash1: &str, hash2: &str) -> Result<u32, ()> {
    let hash1: ImageHash<Box<[u8]>> = ImageHash::from_base64(hash1).map_err(|_| ())?;
    let hash2 = ImageHash::from_base64(hash2).map_err(|_| ())?;
    Ok(hash1.dist(&hash2))
}
