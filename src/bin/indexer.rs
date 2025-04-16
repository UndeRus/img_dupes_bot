use std::sync::{Arc, Mutex};

use image_hasher::{HashAlg, HasherConfig};

use glob::glob;
use img_hashing_bot::db;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

fn main() {
    let db = Arc::new(Mutex::new(db::create_db("hashes.db").expect("Failed to open db")));

    let files = read_files();

    //dbg!(files);
    let hash_landscape_config = HasherConfig::new()
        .hash_size(30, 20)
        .hash_alg(HashAlg::Blockhash);
    let hasher_landscape = hash_landscape_config.to_hasher();

    let hash_portrait_config = HasherConfig::new()
        .hash_size(20, 30)
        .hash_alg(HashAlg::Blockhash);
    let hasher_portrait = hash_portrait_config.to_hasher();

    let hash_square_config = HasherConfig::new()
        .hash_size(30, 30)
        .hash_alg(HashAlg::Blockhash);
    let hasher_square = hash_square_config.to_hasher();

    files.par_iter().for_each(|file| match image::open(file) {
        Ok(img) => {
            let hash_landscape = hasher_landscape.hash_image(&img).to_base64();
            let hash_portrait = hasher_portrait.hash_image(&img).to_base64();
            let hash_square = hasher_square.hash_image(&img).to_base64();
            let db = db.lock().unwrap();
            let _ = db.execute(
                r#"INSERT INTO hashes(filename, orientation, base64_hash) VALUES(?, ?, ?)"#,
                rusqlite::params![file, "landscape", hash_landscape],
            );
            let _ = db.execute(
                r#"INSERT INTO hashes(filename, orientation, base64_hash) VALUES(?, ?, ?)"#,
                rusqlite::params![file, "portrait", hash_portrait],
            );
            let _ = db.execute(
                r#"INSERT INTO hashes(filename, orientation, base64_hash) VALUES(?, ?, ?)"#,
                rusqlite::params![file, "square", hash_square],
            );
        }
        Err(_) => {
            println!("Failed to load {}", file);
        }
    });
}

fn read_files() -> Vec<String> {
    let mut result = vec![];
    for entry in glob("demo_data/**/*.jpg").expect("Failed to read glob pattern") {
        match entry {
            Ok(path) => {
                //println!("{:?}", path.display());
                result.push(path.display().to_string());
            }
            Err(_e) => {
                //println!("{:?}", e)
            }
        }
    }
    result
}
