use rusqlite::types::{FromSql, FromSqlResult, ValueRef};



#[derive(Debug)]
pub struct HashRecord {
    pub id: i32,
    pub filename: String,
    pub hash: String,
    pub file_id: String,
    pub chat_id: i64,    // group chat id
    pub message_id: i64, // single message id
    pub media_group_id: Option<String>,
}

#[derive(Debug)]
pub struct VotingRecord {
    pub id: i32,
    pub chat_id: i64,
    pub message_id: i64,
    pub voting_type: VotingType,
}


#[derive(Debug, PartialEq)]
pub enum VotingType {
    NOTDUPE,
    IGNORE,
}

impl ToString for VotingType {
    fn to_string(&self) -> String {
        match self {
            VotingType::NOTDUPE => "nondupes",
            VotingType::IGNORE => "ignore",
        }
        .to_owned()
    }
}

impl FromSql for VotingType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_str().and_then(|as_str| match as_str {
            "nondupes" => Ok(VotingType::NOTDUPE),
            "ignore" => Ok(VotingType::IGNORE),
            _ => Err(rusqlite::types::FromSqlError::InvalidType),
        })
    }
}

#[derive(Debug, PartialEq)]
pub enum VoteType {
    PRO,
    CON,
}

impl Into<i64> for VoteType {
    fn into(self) -> i64 {
        match self {
            VoteType::PRO => 1,
            VoteType::CON => -1,
        }
    }
}

impl TryFrom<i64> for VoteType {
    type Error = anyhow::Error;

    fn try_from(value: i64) -> std::result::Result<Self, Self::Error> {
        if value == 1 {
            return Ok(VoteType::PRO);
        } else if value == -1 {
            return Ok(VoteType::CON);
        }
        return Err(anyhow::format_err!("Failed convert vote to enum"));
    }
}

pub struct VoterName(pub String);

impl FromSql for VoterName {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        String::column_result(value).and_then(|r| Ok(Self(r)))
    }
}

pub enum VoteResult {
    InProgress(Vec<VoterName>),
    Finished(Vec<VoterName>, VoteType),
    AlreadyVoted,
}

#[derive(Debug)]
pub struct VoteRecord {
    pub id: i32,
    pub voting_id: i64,
    pub vote_type: VoteType,
    pub user_id: i64,
    pub username: String,
}