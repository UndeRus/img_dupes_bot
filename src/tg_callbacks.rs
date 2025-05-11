use std::sync::Arc;

use frankenstein::{
    client_reqwest::Bot,
    methods::EditMessageTextParams,
    response::{MessageOrBool, MethodResponse},
    AsyncTelegramApi,
};
use tokio::sync::Mutex;

use crate::{hasher::Indexer, keyboards::build_vote_keyboard, models::{VoteType, VotingType}};

#[tracing::instrument(name = "Process wrong dupe callback", skip(api, indexer))]
pub async fn process_wrong_callback(
    api: &Bot,
    chat_id: i64,
    message_id: i32,
    bot_message_id: i32,
    indexer: Arc<Mutex<Indexer>>,
) -> Result<MethodResponse<MessageOrBool>, anyhow::Error> {
    // User BLABLABLA started voting about wrong duplicate
    // start voting
    //TODO: create voting

    let mut indexer = indexer.lock().await;
    let voting_id = indexer
        .create_voting(
            chat_id,
            bot_message_id.try_into().unwrap(),
            message_id.try_into().unwrap(),
            VotingType::NOTDUPE,
        )
        .await
        .map_err(|_| anyhow::format_err!("Failed to create voting"))?;

    api.edit_message_text(
        &EditMessageTextParams::builder()
            .chat_id(chat_id)
            .message_id(bot_message_id)
            .text("Я думаю это дубликат, голосуем за то что это не дубликат")
            .reply_markup(build_vote_keyboard(voting_id))
            .build(),
    )
    .await
    .map_err(|_| anyhow::format_err!("Failed to update message"))
}

#[tracing::instrument(name = "Process ignore dupe callback", skip(api, indexer))]
pub async fn process_ignore_callback(
    api: &Bot,
    chat_id: i64,
    message_id: i32,
    bot_message_id: i32,
    indexer: Arc<Mutex<Indexer>>,
) -> Result<MethodResponse<MessageOrBool>, anyhow::Error> {
    // User BLABLABLA started voting about remove notification
    // start voting
    let mut indexer = indexer.lock().await;
    let voting_id = indexer
        .create_voting(
            chat_id,
            bot_message_id.try_into().unwrap(),
            message_id.try_into().unwrap(),
            VotingType::IGNORE,
        )
        .await
        .map_err(|_| anyhow::format_err!("Failed to create voting"))?;

    api.edit_message_text(
        &EditMessageTextParams::builder()
            .chat_id(chat_id)
            .message_id(bot_message_id)
            .text("Голосуем за игнор")
            .reply_markup(build_vote_keyboard(voting_id))
            .build(),
    )
    .await
    .map_err(|_| anyhow::format_err!("Failed to update message"))
}

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

#[tracing::instrument(name = "Process voting contra callback", skip(api, indexer))]
pub async fn process_contra_callback(
    voting_id: i64,
    user_id: u64,
    username: &str,
    api: &Bot,
    indexer: Arc<Mutex<Indexer>>,
) -> Result<(), anyhow::Error> {
    //TODO: search voting, check votes count, add new vote, update message with voters

    let mut indexer = indexer.lock().await;

    let vote_result = indexer
        .vote(voting_id, user_id, username, VoteType::CON)
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
