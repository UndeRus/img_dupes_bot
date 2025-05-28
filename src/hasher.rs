use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use image::DynamicImage;
use image_hasher::{HashAlg, Hasher, HasherConfig};

use tokio::sync::Mutex;

use crate::{
    create_vote, create_voting, db, delete_old_hash, find_image_by_unique_file_id, find_similar_hashes, get_voting_info, metrics, move_old_hash_to_new, HashRecord, VoteResult, VoteType, VotingRecord, VotingType
};

const PERCEPTIVE_HASH_TOLERANCE: usize = 5;
const SEARCH_DISTANCE_IN_SECONDS: u64 = 7 * 24 * 60 * 60;
pub const MIN_VOTES_COUNT: i64 = 5;

pub struct Indexer {
    hasher_landscape: Hasher,
    hasher_portrait: Hasher,
    hasher_square: Hasher,
    db: Arc<Mutex<rusqlite::Connection>>,
}

impl Default for Indexer {
    fn default() -> Self {
        Self::new("hashes.db")
    }
}

impl Indexer {
    pub fn new(db_path: &str) -> Self {
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

        let db = Arc::new(Mutex::new(db::create_db(db_path).expect("Failed to open db")));

        Self {
            hasher_landscape,
            hasher_portrait,
            hasher_square,
            db,
        }
    }

    pub async fn is_file_processed_info(&self, file_id: &str, chat_id: i64) -> Option<HashRecord> {
        let db = self.db.lock().await;
        let send_metric = metrics::mtr_is_file_processed_info_query_time();

        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let from_timestamp = current_timestamp - SEARCH_DISTANCE_IN_SECONDS;
        let result = find_image_by_unique_file_id(&db, file_id, chat_id, from_timestamp);
        send_metric();
        result
    }

    #[tracing::instrument("Calculate image hashes", skip(self, img))]
    pub fn hash_image(&self, img: &DynamicImage) -> (String, String, String) {
        let send_metric = metrics::mtr_message_hashing_time();

        let hash_landscape = self.hasher_landscape.hash_image(img).to_base64();
        let hash_portrait = self.hasher_portrait.hash_image(img).to_base64();
        let hash_square = self.hasher_square.hash_image(img).to_base64();

        send_metric();

        (hash_landscape, hash_portrait, hash_square)
    }

    pub async fn find_similar_hashes(
        &self,
        (hash_landscape, hash_portrait, hash_square): (&str, &str, &str),
        chat_id: i64,
    ) -> Vec<HashRecord> {
        let db = self.db.lock().await;
        let send_mtr = metrics::mtr_find_similar_hashes_time();

        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let from_timestamp = current_timestamp - SEARCH_DISTANCE_IN_SECONDS;

        let results: Vec<HashRecord> = [hash_landscape, hash_portrait, hash_square]
            .iter()
            .filter_map(|hash_str| {
                let result = find_similar_hashes(
                    &db,
                    hash_str,
                    PERCEPTIVE_HASH_TOLERANCE,
                    chat_id,
                    from_timestamp,
                );
                result.ok()
            })
            .flatten()
            .collect();

        // Send metrics
        send_mtr();

        results
    }

    #[tracing::instrument("Save image hashes to db", skip(self))]
    pub async fn save_to_index(
        &mut self,
        filename: &str,
        chat_id: i64,
        message_id: i64,
        file_id: &str,
        media_group_id: Option<String>,
        (hash_landscape, hash_portrait, hash_square): (&str, &str, &str),
    ) -> Result<(), ()> {
        let mut db = self.db.lock().await;

        let tx = db.transaction().map_err(|e| {
            tracing::error!("Transaction error {}", e);
            
        })?;
        {
            let mut prepared_st = tx
                .prepare(
                    r#"INSERT INTO hashes(filename, orientation, base64_hash, chat_id, message_id, file_id, created_at, media_group_id) VALUES(?, ?, ?, ?, ?, ?, ?, ?)"#,
                )
                .map_err(|e| {
                    tracing::error!("Compile statement error {}", e);
                    
                })?;

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs();

            let media_group_id = media_group_id.unwrap_or("".to_owned());

            prepared_st
                .execute(rusqlite::params![
                    filename,
                    "landscape",
                    hash_landscape,
                    chat_id,
                    message_id,
                    file_id,
                    now,
                    media_group_id,
                ])
                .map_err(|e| {
                    tracing::error!("Insert landscape error {}", e);
                })?;

            prepared_st
                .execute(rusqlite::params![
                    filename,
                    "portrait",
                    hash_portrait,
                    chat_id,
                    message_id,
                    file_id,
                    now,
                    media_group_id,
                ])
                .map_err(|e| {
                    tracing::error!("Insert portrait error {}", e);
                })?;

            prepared_st
                .execute(rusqlite::params![
                    filename,
                    "square",
                    hash_square,
                    chat_id,
                    message_id,
                    file_id,
                    now,
                    media_group_id,
                ])
                .map_err(|e| {
                    tracing::error!("Transaction error {}", e);
                })?;
        }

        tx.commit().map_err(|e| {
            tracing::error!("Transaction error {}", e);
        })?;
        Ok(())
    }

    pub async fn delete_old_hash(&mut self, hash_id: i32) {
        let db = self.db.lock().await;
        let _ = delete_old_hash(&db, hash_id);
    }

    #[tracing::instrument(name = "Update existing hash", skip(self))]
    pub async fn update_old_hash(&mut self, hash_id: i32, chat_id: i64, message_id: i64) {
        let db = self.db.lock().await;
        if let Err(e) = move_old_hash_to_new(&db, hash_id, chat_id, message_id) {
            tracing::error!("Failed to update old hash: {}", e);
        } else {
            tracing::info!("Old hash updated");
        }
    }

    #[tracing::instrument(name = "Create voting", skip(self))]
    pub async fn create_voting(&mut self, chat_id: i64, message_id: i64, original_message_id: i64, voting_type: VotingType) -> Result<i64, ()> {
        let db = self.db.lock().await;
        match create_voting(&db, chat_id, message_id, original_message_id, voting_type) {
            Ok(result) => {
                tracing::info!("Voting created");
                Ok(result)
            },
            Err(e) => {
                tracing::error!("Voting create failed: {e}");
                Err(())
            },
        }
    }

    #[tracing::instrument(name = "Get voting info", skip(self))]
    pub async fn get_voting_info(&mut self, voting_id: i64) -> Result<VotingRecord, anyhow::Error> {
        let db = self.db.lock().await;
        return get_voting_info(&db, voting_id);
    }


    #[tracing::instrument(name = "Create new vote", skip(self))]
    pub async fn vote(&mut self, voting_id: i64, user_id: u64, username: &str, vote_type: VoteType) -> Result<VoteResult, anyhow::Error> {
        let mut db = self.db.lock().await;
        return create_vote(&mut db, voting_id, user_id, username, vote_type);
    }
}
