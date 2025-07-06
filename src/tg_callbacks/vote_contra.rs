use std::{sync::Arc, time::Duration};

use frankenstein::{
    client_reqwest::Bot,
    methods::{DeleteMessageParams, EditMessageTextParams},
    AsyncTelegramApi,
};
use tokio::sync::Mutex;

use crate::{
    hasher::Indexer,
    keyboards::build_vote_keyboard,
    models::VoteType,
    VoteResult,
};

use super::{get_vote_result_text, get_vote_type_text};

#[tracing::instrument(name = "Process voting contra callback", skip(api, indexer))]
pub async fn process_contra_callback(
    voting_id: i64,
    user_id: u64,
    username: &str,
    api: &Bot,
    indexer: Arc<Mutex<Indexer>>,
) -> Result<(), anyhow::Error> {
    let mut indexer = indexer.lock().await;

    let vote_result = indexer
        .vote(voting_id, user_id, username, VoteType::CON)
        .await?;

    match vote_result {
        VoteResult::InProgress(voter_names) => {
            let voting_info = indexer.get_voting_info(voting_id).await?;
            let message_id = voting_info.message_id.try_into()?;

            let message_text = format!(
                "Голосуем за {}\nПроголосовали: {}",
                get_vote_type_text(&voting_info.voting_type),
                voter_names
                    .iter()
                    .map(|s| s.0.as_str())
                    .collect::<Vec<&str>>()
                    .join(","),
            );

            api.edit_message_text(
                &EditMessageTextParams::builder()
                    .chat_id(voting_info.chat_id)
                    .message_id(message_id)
                    .reply_markup(build_vote_keyboard(voting_id, &voting_info.voting_type))
                    .text(message_text)
                    .build(),
            )
            .await?;
        }
        VoteResult::Finished(voter_names, voting_result) => {
            let voting_info = indexer.get_voting_info(voting_id).await?;
            let message_id = voting_info.message_id.try_into()?;

            let vote_result = get_vote_result_text(&voting_result);

            let message_text = format!(
                "Голосование за {} завершено\nОкончательный голос {}\nПроголосовали: {}",
                get_vote_type_text(&voting_info.voting_type),
                vote_result,
                voter_names
                    .iter()
                    .map(|s| s.0.as_str())
                    .collect::<Vec<&str>>()
                    .join(", "),
            );

            api.edit_message_text(
                &EditMessageTextParams::builder()
                    .chat_id(voting_info.chat_id)
                    .message_id(message_id)
                    .text(message_text)
                    .build(),
            )
            .await?;

            tracing::error!("Voting result: {:?}", &voting_result);

            if voting_result == VoteType::PRO {
                tokio::time::sleep(Duration::from_secs(5)).await;
                api.delete_message(
                    &DeleteMessageParams::builder()
                        .chat_id(voting_info.chat_id)
                        .message_id(message_id)
                        .build(),
                )
                .await?;
            }
        }
        VoteResult::AlreadyVoted => {}
    }
    Ok(())
}
