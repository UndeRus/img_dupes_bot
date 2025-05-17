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
