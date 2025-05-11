use std::sync::Arc;

use frankenstein::{
    client_reqwest::Bot,
    methods::EditMessageTextParams,
    AsyncTelegramApi,
};
use tokio::sync::Mutex;

use crate::{hasher::Indexer, models::VoteType};

use super::{get_vote_result_text, get_vote_type_text};


#[tracing::instrument(name = "Process voting pro callback", skip(api, indexer))]
pub async fn process_pro_callback(
    voting_id: i64,
    user_id: u64,
    username: &str,
    api: &Bot,
    indexer: Arc<Mutex<Indexer>>,
) -> Result<(), anyhow::Error> {
    //TODO: search voting, check votes count, add new vote, update message with voters

    let mut indexer = indexer.lock().await;

    let vote_result = indexer
        .vote(voting_id, user_id, username, VoteType::PRO)
        .await?;

    match vote_result {
        crate::VoteResult::InProgress(voter_names) => {
            let voting_info = indexer.get_voting_info(voting_id).await?;
            //TODO: send message "Голосуем за ошибочный дубликат"
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
                    .text(message_text)
                    .build(),
            )
            .await?;
        }
        crate::VoteResult::Finished(voter_names, voting_result) => {
            //TODO: send message "Голосуем за ошибочный дубликат"
            let voting_info = indexer.get_voting_info(voting_id).await?;
            //TODO: send message "Голосуем за ошибочный дубликат"
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
                    .join(","),
            );

            api.edit_message_text(
                &EditMessageTextParams::builder()
                    .chat_id(voting_info.chat_id)
                    .message_id(message_id)
                    .text(message_text)
                    .build(),
            )
            .await?;


            if voting_result == VoteType::CON {
                //TODO удаляем сообщение
                //TODO сохраняем инфу если голосование за кривой дубликат
            }
        }
    }
    Ok(())
}