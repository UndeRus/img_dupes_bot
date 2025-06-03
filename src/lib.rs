use hasher::MIN_VOTES_COUNT;
use models::{HashRecord, VoteResult, VoteType, VoterName, VotingRecord, VotingType};
use rusqlite::{Connection, Result};

pub mod data;
pub mod db;
pub mod hasher;
pub mod keyboards;
pub mod metrics;
mod models;
pub mod storage;
pub mod tg_callbacks;
pub mod tracing_setup;

pub fn find_image_by_unique_file_id(
    conn: &Connection,
    unique_file_id: &str,
    chat_id: i64,
    from_timestamp: u64,
) -> Option<HashRecord> {
    let mut stmt = conn.prepare(
        "SELECT id, filename, base64_hash, file_id, chat_id, message_id FROM hashes WHERE file_id = ? AND chat_id = ? AND created_at > ?",
    ).map_err(|e|{
        eprintln!("Failed to prepare statement {e}");
        e
    }).ok()?;

    let mut result = stmt
        .query(rusqlite::params![unique_file_id, chat_id, from_timestamp])
        .map_err(|e| {
            eprint!("Select error {e}");
            e
        })
        .ok()?;
    let row = result.next().ok()??;

    Some(HashRecord {
        id: row.get(0).unwrap_or_default(),
        filename: row.get(1).unwrap_or_default(),
        hash: row.get(2).unwrap_or_default(),
        file_id: row.get(3).unwrap_or_default(),
        chat_id: row.get(4).unwrap_or_default(),
        message_id: row.get(5).unwrap_or_default(),
        media_group_id: None,
    })
}

pub fn find_similar_hashes(
    conn: &Connection,
    input_hash: &str,
    max_distance: usize,
    chat_id: i64,
    from_timestamp: u64,
) -> Result<Vec<HashRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, filename, base64_hash, file_id, chat_id, message_id, media_group_id, hamming_distance(base64_hash, ?) as dist FROM hashes WHERE chat_id  = ? AND dist < ? AND created_at > ? ORDER by dist ASC",
    ).map_err(|e|{
        eprint!("Failed to execute query to search similar {e}");
        e
    })?;

    let mut rows = stmt.query(rusqlite::params![
        input_hash,
        chat_id,
        max_distance,
        from_timestamp
    ])?;

    // Collect results
    let mut similar_hashes = Vec::new();
    while let Some(row) = rows.next()? {
        let media_group_id: Option<String> = row.get(6).unwrap_or(None);
        let media_group_id =
            media_group_id.filter(|media_group_id| !media_group_id.trim().is_empty());

        similar_hashes.push(HashRecord {
            id: row.get(0).unwrap_or_default(),
            filename: row.get(1).unwrap_or_default(),
            hash: row.get(2).unwrap_or_default(),
            file_id: row.get(3).unwrap_or_default(),
            chat_id: row.get(4).unwrap_or_default(),
            message_id: row.get(5).unwrap_or_default(),
            media_group_id,
        });
    }

    Ok(similar_hashes)
}

pub fn delete_old_hash(conn: &Connection, hash_id: i32) -> Result<(), rusqlite::Error> {
    let mut stmt = conn.prepare("DELETE FROM hashes WHERE id = ?")?;

    let result = stmt.execute(rusqlite::params![hash_id]).map_err(|e| {
        tracing::error!("Delete error {}", e);
        e
    })?;
    tracing::info!("Hash records deleted {}", result);
    Ok(())
}

pub fn move_old_hash_to_new(
    conn: &Connection,
    hash_id: i32,
    chat_id: i64,
    message_id: i64,
) -> Result<(), rusqlite::Error> {
    let mut stmt = conn.prepare("UPDATE hashes SET message_id = ? WHERE id = ? AND chat_id = ?")?;

    let result = stmt
        .execute(rusqlite::params![message_id, hash_id, chat_id])
        .map_err(|e| {
            tracing::error!("Update error {}", e);
            e
        })?;
    tracing::info!("Hash records updated {}", result);
    Ok(())
}

pub fn create_voting(
    conn: &Connection,
    chat_id: i64,
    message_id: i64,
    original_message_id: i64,
    voting_type: VotingType,
) -> Result<i64, rusqlite::Error> {
    let mut stmt = conn.prepare("INSERT INTO votings(chat_id, message_id, original_message_id, voting_type) VALUES(?, ?, ?, ?)")?;

    stmt.execute(rusqlite::params![
        chat_id,
        message_id,
        original_message_id,
        voting_type.to_string()
    ])
    .map_err(|e| {
        tracing::error!("Create voting error {e}");
        e
    })?;

    tracing::info!("New voting created");
    Ok(conn.last_insert_rowid())
}

pub fn create_vote(
    db: &mut Connection,
    voting_id: i64,
    user_id: u64,
    username: &str,
    vote_type: VoteType,
) -> Result<VoteResult, anyhow::Error> {
    if is_already_voted(&db, voting_id, user_id)? {
        return Ok(VoteResult::AlreadyVoted);
    }
    {
        let mut insert_vote_stmt = db
            .prepare(
                r"INSERT INTO votes(voting_id, user_id, username, vote_type) VALUES(?, ?, ?, ?)",
            )
            .map_err(|e| {
                tracing::error!("Compile statement error {}", e);
                anyhow::format_err!("Compile statement error {e}")
            })?;

        insert_vote_stmt
            .execute(rusqlite::params![
                voting_id,
                user_id,
                username,
                Into::<i64>::into(vote_type)
            ])
            .map_err(|e| {
                tracing::error!("Vote insert query error {}", e);
                anyhow::format_err!("Vote insert query error {e}")
            })?;
    }

    let votes_count = get_votes_count(voting_id, &db)?;

    let (voting_result, score) = get_voting_result(&db, voting_id)?;
    if votes_count >= MIN_VOTES_COUNT || score.abs() >= MIN_VOTES_COUNT / 2 {
        let voters = get_voting_names(&db, voting_id)?;

        return Ok(VoteResult::Finished(voters, voting_result));
    }

    let voters = get_voting_names(&db, voting_id)?;
    Ok(VoteResult::InProgress(voters))
}

fn get_votes_count(voting_id: i64, tx: &Connection) -> Result<i64, anyhow::Error> {
    let mut vote_count_query = tx.prepare(
        r"SELECT COUNT(vot.id) FROM votings vti JOIN votes vot ON vti.id = vot.voting_id WHERE vti.id = ?"
    ).map_err(|e| {
        tracing::error!("Compile statement error {}", e);
        anyhow::format_err!("Compile statement error {e}")
    })?;
    let votes_count = vote_count_query
        .query_row(rusqlite::params![voting_id], |row| row.get::<_, i64>(0))
        .map_err(|e| {
            tracing::error!("Vote count query error {}", e);
            anyhow::format_err!("Vote count query error {e}")
        })?;
    Ok(votes_count)
}

fn get_voting_names(conn: &Connection, voting_id: i64) -> Result<Vec<VoterName>, anyhow::Error> {
    let mut voters_query = conn
        .prepare(r"SELECT username FROM votes WHERE voting_id = ?")
        .map_err(|e| {
            tracing::error!("Compile statement error {}", e);
            anyhow::format_err!("Compile statement error {e}")
        })?;
    let mut voters_result = voters_query
        .query(rusqlite::params![voting_id])
        .map_err(|e| {
            tracing::error!("Voters names query error {}", e);
            anyhow::format_err!("Voters names query error {e}")
        })?;
    let mut voters = vec![];
    while let Ok(Some(voter_username_row)) = voters_result.next() {
        if let Ok(username) = voter_username_row.get(0) {
            voters.push(username);
        }
    }
    Ok(voters)
}

fn is_already_voted(
    conn: &Connection,
    voting_id: i64,
    user_id: u64,
) -> Result<bool, anyhow::Error> {
    let mut vote_query = conn
        .prepare(r"SELECT id FROM votes WHERE voting_id = ? AND user_id = ?")
        .map_err(|e| {
            tracing::error!("Compile statement error {}", e);
            anyhow::format_err!("Compile statement error {e}")
        })?;
    let mut result = vote_query
        .query(rusqlite::params![voting_id, user_id])
        .map_err(|e| {
            tracing::error!("Voter is already voted query error {}", e);
            anyhow::format_err!("Voter is already voted query error {e}")
        })?;
    let result = result.next().map_err(|e| {
        tracing::error!("Voter is already voted query unwrap error {}", e);
        anyhow::format_err!("Voter is already voted query unwrap error {e}")
    })?;
    if result.is_none() {
        return Ok(false);
    } else {
        return Ok(true);
    }
}

fn get_voting_info(conn: &Connection, voting_id: i64) -> Result<VotingRecord, anyhow::Error> {
    let mut voting_query = conn
        .prepare(r"SELECT id, chat_id, message_id, voting_type FROM votings WHERE id = ?")
        .map_err(|e| {
            tracing::error!("Compile statement error {}", e);
            anyhow::format_err!("Compile statement error {e}")
        })?;
    let mut voting_result = voting_query
        .query(rusqlite::params![voting_id])
        .map_err(|e| {
            tracing::error!("Voting info query error {}", e);
            anyhow::format_err!("Voting info query error {e}")
        })?;
    if let Ok(Some(row)) = voting_result.next() {
        let id = row.get(0)?;
        let chat_id = row.get(1)?;
        let message_id = row.get(2)?;
        let voting_type: VotingType = row.get(3)?;
        Ok(VotingRecord {
            id,
            chat_id,
            message_id,
            voting_type,
        })
    } else {
        Err(anyhow::format_err!("Failed to fetch row for voting info"))
    }
}

fn get_voting_result(conn: &Connection, voting_id: i64) -> Result<(VoteType, i64), anyhow::Error> {
    let mut voting_query = conn
        .prepare(r"SELECT SUM(vote_type) FROM votes WHERE voting_id = ?")
        .map_err(|e| {
            tracing::error!("Compile statement error {}", e);
            anyhow::format_err!("Compile statement error {e}")
        })?;
    let mut voting_result = voting_query
        .query(rusqlite::params![voting_id])
        .map_err(|e| {
            tracing::error!("Voters names query error {}", e);
            anyhow::format_err!("Voters names query error {e}")
        })?;

    if let Ok(Some(result)) = voting_result.next() {
        let final_vote_result: i64 = result.get(0).map_err(|e| {
            tracing::error!("Failed to query final vote result");
            anyhow::format_err!("Failed to query final vote result: {e}")
        })?;

        return if final_vote_result > 0 {
            Ok((VoteType::PRO, final_vote_result))
        } else {
            Ok((VoteType::CON, final_vote_result))
        };
    }

    Err(anyhow::format_err!("Failed to query final vote result"))
}
