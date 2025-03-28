use std::{ffi::OsStr, io::Cursor, path::PathBuf, str::FromStr, sync::Arc};

use dotenvy::dotenv;
use frankenstein::{
    client_reqwest::Bot, AnswerCallbackQueryParams, AsyncTelegramApi, CallbackQuery,
    EditMessageTextParams, File, GetFileParams, GetUpdatesParams, InlineKeyboardButton,
    InlineKeyboardMarkup, Message, MethodResponse, ReplyMarkup, ReplyParameters, SendMessageParams,
    UpdateContent, User,
};

use img_hashing_bot::{
    data::{CallbackQueryCommand, CallbackQueryData},
    file_storage::{FileStorage, S3FileStorage},
    hasher::Indexer,
    metrics,
    tracing_setup::init_tracing,
};
use reqwest::Response;
use tokio::{fs, signal, sync::Mutex};

const MESSAGE_FOUND_MSG: &str = "Эту картинку уже постили тут:";
const REPLY_NOT_FOUND_ERROR: &str = "Bad Request: message to be replied not found";

#[tokio::main]
async fn main() -> Result<(), ()> {
    dotenv().ok();

    let finisher = init_tracing(
        &dotenvy::var("OTLP_ENDPOINT").unwrap(),
        &dotenvy::var("OTLP_TOKEN").unwrap(),
    );
    let indexer = Arc::new(Mutex::new(Indexer::new()));

    let bot_api_token = &dotenvy::var("TELEGRAM_BOT_API_TOKEN").unwrap();
    let api = Bot::new(bot_api_token);
    let files_endpoint = format!(
        "https://api.telegram.org/file/bot{bot_api_token}/",
        bot_api_token = bot_api_token
    );

    let update_params_builder = GetUpdatesParams::builder();
    let mut update_params = update_params_builder.build();

    loop {
        tokio::select! {
            result = api.get_updates(&update_params) => {
                match result {
                    Ok(response) => {
                        for update in response.result {
                            match update.content {
                                UpdateContent::Message(message) => {

                                        let api_clone = api.clone();
                                        let files_endpoint = files_endpoint.clone();

                                        let indexer = indexer.clone();
                                        tokio::spawn(async move {
                                            if message.photo.is_none() {
                                                return;
                                            }
                                            if let Err(_) = process_message(message, api_clone, &files_endpoint, indexer).await {
                                                tracing::error!("Failed to start message processing");
                                            }
                                        });

                                }
                                UpdateContent::CallbackQuery(callback_message) => {
                                    let api_clone = api.clone();
                                    tokio::spawn(async move {
                                        let result = process_callback(&api_clone, callback_message).await;
                                        if let Err(err) = result {
                                            tracing::warn!("Failed to process buttons: {}", err);
                                        }
                                    });
                                }
                                _ => {
                                    tracing::info!("Other {:?}", update.content);
                                }
                            };
                            update_params.offset = Some(i64::from(update.update_id) + 1);
                        }
                    }
                    Err(error) => {
                        tracing::error!("Failed to get updates: {error:?}");
                    }
                }
            }


            _ = signal::ctrl_c() => {
                tracing::info!("Bot finished");
                break;
            }
        }
    }
    finisher();
    Ok(())
}

#[tracing::instrument(
    name = "Process new message subscriber",
    skip(api, files_endpoint, indexer)
)]
async fn process_message(
    message: Message,
    api: Bot,
    files_endpoint: &str,
    indexer: Arc<Mutex<Indexer>>,
) -> Result<(), ()> {
    if let Some(_) = message.reply_to_message {
        tracing::info!("This is reply, ignore");
        return Ok(());
    }
    if let Some(pics) = message.photo {
        tracing::info!("Got picture {} {}", message.message_id, pics.len());
        let best_quality = pics.last();
        if let Some(best_quality) = best_quality {
            let params = GetFileParams::builder()
                .file_id(&best_quality.file_id)
                .build();
            let response = api.get_file(&params).await;
            if let Ok(response) = response {
                let mut indexer = indexer.lock().await;
                // check by unique id
                let file_processed_info = indexer
                    .is_file_processed_info(&response.result.file_unique_id)
                    .await;

                if let Some(Ok(user_id)) = message.from.clone().map(|f| f.id.try_into()) {
                    metrics::mtr_images_count(1, user_id);
                }

                //TODO: check this file already exists
                if let Some(file_processed_info) = file_processed_info {
                    //TODO: return existing file info to chat

                    metrics::mtr_samefiles_count(1);
                    tracing::info!("Found same file in db");

                    if let Err(e) = send_message(
                        &api,
                        message.chat.id,
                        file_processed_info
                            .message_id
                            .try_into()
                            .expect("Failed to cast message id"),
                    )
                    .await
                    {
                        //TODO: remove image from db if cannot find original
                        if let frankenstein::Error::Api(e) = e {
                            if e.description == REPLY_NOT_FOUND_ERROR {
                                tracing::warn!("Reply not found, update existing record");
                                metrics::mtr_removed_originals_count(1);
                                let hash_record = file_processed_info;
                                indexer
                                    .update_old_hash(
                                        hash_record.id,
                                        message.chat.id,
                                        message.message_id as i64,
                                    )
                                    .await;
                            }
                        } else {
                            tracing::error!("Failed to send message about same file id {}", e);
                        }
                    }

                    return Ok(());
                }

                fn get_filename(file_path: &str, file_unique_id: &str) -> String {
                    let original_path = std::path::Path::new(file_path);
                    let extension = original_path
                        .extension()
                        .and_then(OsStr::to_str)
                        .unwrap_or("");

                    // TODO: extract ot file storage implementation
                    let destination_path_str = format!(
                        "{path}.{extension}",
                        path = file_unique_id,
                        extension = extension
                    );
                    return destination_path_str;
                }

                // Download file
                if let Some(file_path) = response.result.file_path.clone() {
                    //TODO: extract logic to storage
                    let storage = S3FileStorage::new(
                        "http://localhost:9000",
                        "imgdupes-bot",
                        "XMAA9SeEDHmk0SOBt1Km",
                        "oi1oufCEl3xHCZEKP4EO0mczKtE1eGsKOa1JH7bL",
                    );
                    let destination_path_str =
                        get_filename(&file_path, &response.result.file_unique_id);

                    let file_uri = storage
                        .save_file(
                            &format!("{}/{}", files_endpoint, file_path),
                            &destination_path_str,
                        )
                        .await;

                    /*
                    let file_response =
                        reqwest::get(format!("{}/{}", files_endpoint, file_path)).await;
                        */
                    if let Ok(file_uri) = file_uri {
                        //if let Ok(file_response) = file_response {
                        if let Some(size) = response.result.file_size {
                            metrics::mtr_image_size(size, message.chat.id);
                        }

                        /*
                        let destination_path =
                            save_file(&file_path, &response, file_response).await?;
                            */

                        let (hash_landscape, hash_portrait, hash_square) =
                            //indexer.hash_image(&image::open(&destination_path).unwrap());
                            indexer.hash_image(&storage.load_file(&file_uri).await.unwrap());

                        let result = indexer
                            .find_similar_hashes(
                                (&hash_landscape, &hash_portrait, &hash_square),
                                message.chat.id,
                            )
                            .await;

                        // Similar hash exists
                        if !result.is_empty() {
                            log::info!("Found images {:?}", result);

                            // notify to chat
                            let found_result_in_chat = result.first().ok_or(())?;

                            if let Some(Ok(user_id)) = message.from.map(|f| f.id.try_into()) {
                                metrics::mtr_duplicate_count(1, message.chat.id, user_id);
                            }

                            if let Err(e) = send_message(
                                &api,
                                message.chat.id,
                                found_result_in_chat
                                    .message_id
                                    .try_into()
                                    .expect("Failed to convert message id"),
                            )
                            .await
                            {
                                if let frankenstein::Error::Api(e) = e {
                                    if e.description == REPLY_NOT_FOUND_ERROR {
                                        // remove old record
                                        tracing::error!("Reply not found after indexing, update record in db, old message_id: {}, new message_id: {}", found_result_in_chat.message_id, message.message_id);
                                        indexer
                                            .update_old_hash(
                                                found_result_in_chat.id,
                                                message.chat.id,
                                                message.message_id as i64,
                                            )
                                            .await;
                                        return Ok(());
                                    }
                                } else {
                                    tracing::error!("Failed to send message about file with almost same hash {}", e);
                                    return Err(());
                                }
                            } else {
                                tracing::info!("Message sent to chat");
                            }

                            //remove file
                            if let Err(e) = storage.remove_file(&file_uri).await {
                            //if let Err(e) = fs::remove_file(destination_path).await {
                                tracing::error!("Failed to remove file {}", e);
                                return Err(());
                            }
                            // Extract to FileStorage

                            return Ok(());
                        }

                        // Save to index
                        if let Err(e) = indexer
                            .save_to_index(
                                &file_uri,
                                //destination_path.to_str().unwrap_or(""),
                                message.chat.id,
                                message.message_id as i64,
                                &response.result.file_unique_id,
                                (&hash_landscape, &hash_portrait, &hash_square),
                            )
                            .await
                        {
                            tracing::error!("Failed to index image {:?}", e);
                        }
                    } else {
                        tracing::error!("Failed to upload file");
                    }
                }
            }
        }
        Ok(())
    } else {
        let text = message.text.clone().unwrap_or("".to_string());
        if text == "/start" {
            return Ok(());
        } else if text == "/help" {
            return Ok(());
        } else {
            Ok(())
        }
    }
}

#[tracing::instrument(name = "Save file", skip(file_response))]
async fn save_file(
    file_path: &str,
    response: &MethodResponse<File>,
    file_response: Response,
) -> Result<PathBuf, ()> {
    let original_path = std::path::Path::new(file_path);
    let extension = original_path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or("");

    // TODO: extract ot file storage implementation
    let destination_path_str = format!(
        "./files/{path}.{extension}",
        path = response.result.file_unique_id,
        extension = extension
    );
    let destination_path = std::path::Path::new(&destination_path_str);
    let prefix = destination_path.parent().unwrap();
    std::fs::create_dir_all(prefix).unwrap();
    let mut file = std::fs::File::create(destination_path).map_err(|_| ())?;
    let mut content = Cursor::new(file_response.bytes().await.map_err(|_| ())?);
    std::io::copy(&mut content, &mut file).map_err(|_| ())?;
    Ok(destination_path.to_path_buf())
}

#[tracing::instrument(name = "Send message to chat", skip(api))]
async fn send_message(
    api: &Bot,
    chat_id: i64,
    message_id: i32,
) -> Result<MethodResponse<Message>, frankenstein::Error> {
    let reply_params = ReplyParameters::builder()
        .message_id(message_id) // original message id
        .build();

    let send_message_params = SendMessageParams::builder()
        .chat_id(chat_id)
        .text(MESSAGE_FOUND_MSG)
        .reply_parameters(reply_params)
        .reply_markup(build_keyboard(chat_id, message_id))
        .build();
    api.send_message(&send_message_params).await
}

fn build_keyboard(chat_id: i64, message_id: i32) -> ReplyMarkup {
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = Vec::new();

    let mut row = vec![];

    row.push(
        InlineKeyboardButton::builder()
            .text("😡 not dupe")
            .callback_data(format!("wr {} {}", chat_id, message_id))
            .build(),
    );
    row.push(
        InlineKeyboardButton::builder()
            .text("😑 ignore")
            .callback_data(format!("ig {} {}", chat_id, message_id))
            .build(),
    );

    keyboard.push(row);

    let inline_keyboard = InlineKeyboardMarkup::builder()
        .inline_keyboard(keyboard)
        .build();

    let keyboard_markup = ReplyMarkup::InlineKeyboardMarkup(inline_keyboard);

    keyboard_markup
}

#[tracing::instrument(name = "Process inline query", skip(api))]
async fn process_callback(api: &Bot, query: CallbackQuery) -> Result<(), anyhow::Error> {
    let result = api
        .answer_callback_query(
            &AnswerCallbackQueryParams::builder()
                .callback_query_id(query.id)
                .build(),
        )
        .await;

    if let Err(err) = result {
        tracing::warn!("Failed to answer callback query");
        return Err(anyhow::format_err!("{}", err));
    }

    let data = query.data.ok_or(anyhow::format_err!("No inline data"))?;

    let callback_data = CallbackQueryData::from_str(&data)?;

    let username = get_username(&query.from);

    println!("callback data: {:?}", callback_data);

    let maybe_message = query
        .message
        .ok_or(anyhow::format_err!("Failed to find message"))?;
    let message_id = match maybe_message {
        frankenstein::MaybeInaccessibleMessage::Message(message) => message.message_id,
        frankenstein::MaybeInaccessibleMessage::InaccessibleMessage(_) => {
            return Err(anyhow::format_err!("Message is inaccessible"));
        }
    };

    match callback_data.command {
        CallbackQueryCommand::WRONG => {
            if let Err(e) = process_wrong_callback(
                &api,
                callback_data.args[0],
                callback_data.args[1] as i32,
                message_id,
            )
            .await
            {
                tracing::error!("Failed to update message: {}", e);
            } else {
                tracing::info!("Message update sent")
            }
        }
        CallbackQueryCommand::IGNORE => {
            process_ignore_callback().await;
        }
        CallbackQueryCommand::PRO => {}
        CallbackQueryCommand::CON => {}
    }

    Ok(())
}

#[tracing::instrument(name = "Process wrong dupe callback", skip(api))]
async fn process_wrong_callback(
    api: &Bot,
    chat_id: i64,
    message_id: i32,
    bot_message_id: i32,
) -> Result<MethodResponse<frankenstein::MessageOrBool>, frankenstein::Error> {
    // User BLABLABLA started voting about wrong duplicate
    // start voting
    api.edit_message_text(
        &EditMessageTextParams::builder()
            .chat_id(chat_id)
            .message_id(bot_message_id)
            .text("Я нашел здесь дубликат, голосуем за то что это не дубликат")
            .reply_markup(build_vote_keyboard(chat_id, message_id))
            .build(),
    )
    .await
}

fn build_vote_keyboard(chat_id: i64, message_id: i32) -> InlineKeyboardMarkup {
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = Vec::new();

    let mut row = vec![];

    row.push(
        InlineKeyboardButton::builder()
            .text("👍")
            .callback_data(format!("pro {} {}", chat_id, message_id))
            .build(),
    );
    row.push(
        InlineKeyboardButton::builder()
            .text("👎")
            .callback_data(format!("con {} {}", chat_id, message_id))
            .build(),
    );

    keyboard.push(row);

    let inline_keyboard = InlineKeyboardMarkup::builder()
        .inline_keyboard(keyboard)
        .build();

    inline_keyboard
}

async fn process_ignore_callback() {
    // User BLABLABLA started voting about remove notification
    // start voting
}

fn get_username(user: &User) -> String {
    if let Some(username) = &user.username {
        return username.clone();
    }

    let mut name_parts = vec![];

    name_parts.push(user.first_name.clone());

    if let Some(last_name) = user.last_name.clone() {
        name_parts.push(last_name.clone());
    }

    name_parts.join(" ")
}
