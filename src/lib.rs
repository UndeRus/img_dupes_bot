use image_hasher::ImageHash;

use rusqlite::{
    functions::{Context, FunctionFlags},
    Connection, Result,
};

pub mod db;
mod file_storage;
pub mod hasher;

fn hamming_distance(hash1: &str, hash2: &str) -> Result<u32, ()> {
    let hash1: ImageHash<Box<[u8]>> = ImageHash::from_base64(hash1).map_err(|_| ())?;
    let hash2 = ImageHash::from_base64(hash2).map_err(|_| ())?;
    return Ok(hash1.dist(&hash2));
}

#[derive(Debug)]
pub struct HashRecord {
    pub id: i32,
    filename: String,
    hash: String,
    file_id: String,
    pub chat_id: i64,    // group chat id
    pub message_id: i64, // single message id
}

#[inline]
fn hamming_sqlite_func(ctx: &Context) -> Result<i64, rusqlite::Error> {
    let hash1: String = ctx.get(0)?;
    let hash2: String = ctx.get(1)?;
    let dist = hamming_distance(&hash1, &hash2);
    dist.map_err(|_| rusqlite::Error::InvalidQuery)
        .map(|x| x as i64)
}

pub fn find_image_by_unique_file_id(conn: &Connection, unique_file_id: &str) -> Option<HashRecord> {
    let mut stmt = conn.prepare(
        "SELECT id, filename, base64_hash, file_id, chat_id, message_id FROM hashes file_id = ?",
    ).ok()?;

    let mut result = stmt.query(rusqlite::params![unique_file_id]).map_err(|e|{
        eprint!("Select error {}",e);
        e
    }).ok()?;
    let row = result.next().ok()??;

    Some(HashRecord {
        id: row.get(0).unwrap_or_default(),
        filename: row.get(1).unwrap_or_default(),
        hash: row.get(2).unwrap_or_default(),
        file_id: row.get(3).unwrap_or_default(),
        chat_id: row.get(4).unwrap_or_default(),
        message_id: row.get(5).unwrap_or_default(),
    })
}

pub fn find_similar_hashes(
    conn: &Connection,
    input_hash: &str,
    max_distance: usize,
    chat_id: i64,
    from_timestamp: u64,
) -> Result<Vec<HashRecord>> {
    // Decode input hash to binary
    //let input_bytes = decode_base64_hash(input_hash);

    // Register custom Hamming distance function
    conn.create_scalar_function(
        "hamming_distance",
        2,
        FunctionFlags::all(),
        move |ctx: &Context| {
            hamming_sqlite_func(ctx)
            //Ok(dist as i64)
        },
    ).map_err(|e|{
        eprintln!("Failed to register function {}", e);
        e
    })?;
    let mut stmt = conn.prepare(
        "SELECT id, filename, base64_hash, file_id, chat_id, message_id, hamming_distance(base64_hash, ?) as dist FROM hashes WHERE chat_id  = ? AND dist < ? AND created_at > ? ORDER by dist ASC",
    ).map_err(|e|{
        eprint!("Failed to execute query to search similar {}", e);
        e
    })?;

    let mut rows = stmt.query(rusqlite::params![input_hash, chat_id, max_distance, from_timestamp])?;

    // Collect results
    let mut similar_hashes = Vec::new();
    while let Some(row) = rows.next()? {
        similar_hashes.push(HashRecord {
            id: row.get(0).unwrap_or_default(),
            filename: row.get(1).unwrap_or_default(),
            hash: row.get(2).unwrap_or_default(),
            file_id: row.get(3).unwrap_or_default(),
            chat_id: row.get(4).unwrap_or_default(),
            message_id: row.get(5).unwrap_or_default(),
        });
    }

    Ok(similar_hashes)
}


pub fn delete_old_hash( conn: &Connection, hash_id: i32) -> Result<(), rusqlite::Error> {
    let mut stmt = conn.prepare(
        "DELETE FROM hashes WHERE id = ?",
    )?;

    let result = stmt.execute(rusqlite::params![hash_id]).map_err(|e|{
        tracing::error!("Delete error {}",e);
        e
    })?;
    tracing::info!("Hash records deleted {}", result);
    Ok(())
}

pub fn move_old_hash_to_new(conn: &Connection, hash_id: i32, chat_id: i64, message_id: i64) -> Result<(), rusqlite::Error>  {
    let mut stmt = conn.prepare(
        "UPDATE hashes SET message_id = ? WHERE id = ? AND chat_id = ?",
    )?;

    let result = stmt.execute(rusqlite::params![message_id, hash_id, chat_id]).map_err(|e|{
        tracing::error!("Update error {}",e);
        e
    })?;
    tracing::info!("Hash records updated {}", result);
    Ok(())
}