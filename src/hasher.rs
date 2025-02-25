use std::{sync::Arc, time::{SystemTime, UNIX_EPOCH}};

use image::DynamicImage;
use image_hasher::{HashAlg, Hasher, HasherConfig};

use glob::glob;
use tokio::sync::Mutex;

use crate::{
    db, delete_old_hash, file_storage::{FileStorage, LocalFileStorage}, find_image_by_unique_file_id, find_similar_hashes, HashRecord
};

const PERCEPTIVE_HASH_TOLERANCE: usize = 10;

pub struct Indexer {
    hasher_landscape: Hasher,
    hasher_portrait: Hasher,
    hasher_square: Hasher,
    db: Arc<Mutex<rusqlite::Connection>>,
    file_storage: LocalFileStorage,
}

impl Indexer {
    pub fn new() -> Self {
        let hash_landscape_config = HasherConfig::new()
            .hash_size(15, 10)
            .hash_alg(HashAlg::Blockhash);
        let hasher_landscape = hash_landscape_config.to_hasher();

        let hash_portrait_config = HasherConfig::new()
            .hash_size(10, 15)
            .hash_alg(HashAlg::Blockhash);
        let hasher_portrait = hash_portrait_config.to_hasher();

        let hash_square_config = HasherConfig::new()
            .hash_size(15, 15)
            .hash_alg(HashAlg::Blockhash);
        let hasher_square = hash_square_config.to_hasher();

        let db = Arc::new(Mutex::new(db::create_db().unwrap()));

        let file_storage = LocalFileStorage {};

        Self {
            hasher_landscape,
            hasher_portrait,
            hasher_square,
            db,
            file_storage,
        }
    }

    pub async fn is_file_processed_info(&self, file_id: &str) -> Option<HashRecord> {
        let db = self.db.lock().await;
        find_image_by_unique_file_id(&db, file_id)
    }

    pub fn hash_image(&self, img: &DynamicImage) -> (String, String, String) {
        let hash_landscape = self.hasher_landscape.hash_image(img).to_base64();
        let hash_portrait = self.hasher_portrait.hash_image(img).to_base64();
        let hash_square = self.hasher_square.hash_image(img).to_base64();
        (hash_landscape, hash_portrait, hash_square)
    }

    pub async fn find_similar_hashes(
        &self,
        (hash_landscape, hash_portrait, hash_square): (&str, &str, &str),
        chat_id: i64,
    ) -> Vec<HashRecord> {
        let db = self.db.lock().await;
        let results: Vec<HashRecord> = [hash_landscape, hash_portrait, hash_square]
            .iter()
            .filter_map(|hash_str| {
                let result = find_similar_hashes(&db, &hash_str, PERCEPTIVE_HASH_TOLERANCE, chat_id);
                result.ok()
            })
            .flatten()
            .collect();
        results
    }

    //pub async fn find_hash()
    //pub async fn save_to_index()

    // TODO: extract this to indexer
    #[tracing::instrument(
        skip(self)
    )]
    pub async fn save_to_index(
        &mut self,
        filename: &str,
        chat_id: i64,
        message_id: i64,
        file_id: &str,
        (hash_landscape, hash_portrait, hash_square): (&str, &str, &str),
    ) -> Result<(), ()> {
        //let (hash_landscape, hash_portrait, hash_square) = self.hash_image(img);

        //println!("Hash of {} is {}", file, hash);
        let mut db = self.db.lock().await;

        let tx = db.transaction().map_err(|e| {
            tracing::error!("Transaction error {}", e);
            ()
        })?;
        {
            let mut prepared_st = tx
                .prepare(
                    r#"INSERT INTO hashes(filename, orientation, base64_hash, chat_id, message_id, file_id, created_at) VALUES(?, ?, ?, ?, ?, ?, ?)"#,
                )
                .map_err(|e| {
                    tracing::error!("Complile statement error {}", e);
                    ()
                })?;


            let now =  SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_secs();

            prepared_st
                .execute(rusqlite::params![filename, "landscape", hash_landscape, chat_id, message_id, file_id, now])
                .map_err(|e| {
                    tracing::error!("Insert landscape error {}", e);
                    ()
                })?;

            prepared_st
                .execute(rusqlite::params![filename, "portrait", hash_portrait, chat_id, message_id, file_id, now])
                .map_err(|e| {
                    tracing::error!("Insert portrait error {}", e);
                    ()
                })?;

            prepared_st
                .execute(rusqlite::params![filename, "square", hash_square, chat_id, message_id, file_id, now])
                .map_err(|e| {
                    tracing::error!("Transaction error {}", e);
                    ()
                })?;
        }

        tx.commit().map_err(|e| {
            tracing::error!("Transaction error {}", e);
            ()
        })?;
        Ok(())
    }


    pub async fn delete_old_hash(&mut self, hash_id: i32) {
        let mut db = self.db.lock().await;
        delete_old_hash(&db, hash_id);
    }
}

async fn main2() {
    let db = Arc::new(Mutex::new(db::create_db().unwrap()));

    let files = read_files();

    //dbg!(files);
    let hash_landscape_config = HasherConfig::new()
        .hash_size(15, 10)
        .hash_alg(HashAlg::Blockhash);
    let hasher_landscape = hash_landscape_config.to_hasher();

    let hash_portrait_config = HasherConfig::new()
        .hash_size(10, 15)
        .hash_alg(HashAlg::Blockhash);
    let hasher_portrait = hash_portrait_config.to_hasher();

    let hash_square_config = HasherConfig::new()
        .hash_size(15, 15)
        .hash_alg(HashAlg::Blockhash);
    let hasher_square = hash_square_config.to_hasher();

    /*
    files.par_iter().for_each(|file| match image::open(file) {
        Ok(img) => {
            let hash_landscape = hasher_landscape.hash_image(&img).to_base64();
            let hash_portrait = hasher_portrait.hash_image(&img).to_base64();
            let hash_square = hasher_square.hash_image(&img).to_base64();
            //println!("Hash of {} is {}", file, hash);
            let db = db.lock().await;
            db.execute(
                r#"INSERT INTO hashes(filename, orientation, base64_hash) VALUES(?, ?, ?)"#,
                rusqlite::params![file, "landscape", hash_landscape],
            );
            db.execute(
                r#"INSERT INTO hashes(filename, orientation, base64_hash) VALUES(?, ?, ?)"#,
                rusqlite::params![file, "portrait", hash_portrait],
            );
            db.execute(
                r#"INSERT INTO hashes(filename, orientation, base64_hash) VALUES(?, ?, ?)"#,
                rusqlite::params![file, "square", hash_square],
            );
        }
        Err(_) => {
            println!("Failed to load {}", file);
        }
    });
    */
}

fn read_files() -> Vec<String> {
    let mut result = vec![];
    for entry in glob("demo_data/**/*.jpg").expect("Failed to read glob pattern") {
        match entry {
            Ok(path) => {
                //println!("{:?}", path.display());
                result.push(path.display().to_string());
            }
            Err(e) => {
                //println!("{:?}", e)
            }
        }
    }
    result
}
