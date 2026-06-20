use crate::models::{VoteType, VotingType};

mod ignore_dupes;
mod vote_contra;
mod vote_pro;
mod wrong_dupes;
pub use ignore_dupes::process_ignore_callback;
pub use vote_contra::process_contra_callback;
pub use vote_pro::process_pro_callback;
pub use wrong_dupes::process_wrong_callback;

fn get_vote_type_text(voting_type: &VotingType) -> String {
    match voting_type {
        VotingType::NOTDUPE => "кривой дубликат",
        VotingType::IGNORE => "игнор",
    }
    .to_owned()
}

fn get_vote_result_text(vote_result: &VoteType) -> String {
    match vote_result {
        VoteType::PRO => "ЗА",
        VoteType::CON => "ПРОТИВ",
    }
    .to_owned()
}
