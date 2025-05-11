use std::sync::Arc;

use frankenstein::{
    client_reqwest::Bot,
    methods::EditMessageTextParams,
    response::{MessageOrBool, MethodResponse},
    AsyncTelegramApi,
};
use tokio::sync::Mutex;

use crate::{hasher::Indexer, keyboards::build_vote_keyboard, models::VotingType};

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
