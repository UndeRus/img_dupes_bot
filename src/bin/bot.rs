use std::{
    ffi::OsStr,
    io::Cursor,
    path::PathBuf,
    sync::Arc,
};


use dotenvy::dotenv;
use dotenvy_macro::dotenv;
use frankenstein::{
    AsyncApi, AsyncTelegramApi, File, GetFileParams, GetUpdatesParams, Message, MethodResponse,
    ReplyParameters, SendMessageParams, UpdateContent,
};

use img_hashing_bot::{hasher::Indexer, tracing_setup::init_tracing};
use reqwest::Response;
use tokio::{fs, signal, sync::Mutex};

const MESSAGE_FOUND_MSG: &str = "Эту картинку уже постили тут:";
const REPLY_NOT_FOUND_ERROR: &str = "Bad Request: message to be replied not found";

#[tokio::main]
async fn main() -> Result<(), ()> {
    dotenv().ok();

    let finisher = init_tracing(dotenv!("OTLP_ENDPOINT"), dotenv!("OTLP_TOKEN"));
    let indexer = Arc::new(Mutex::new(Indexer::new()));

    let bot_api_token = dotenv!("TELEGRAM_BOT_API_TOKEN");
    let api = AsyncApi::new(bot_api_token);
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
    api: AsyncApi,
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

                //TODO: check this file already exists
                if let Some(file_processed_info) = file_processed_info {
                    //TODO: return existing file info to chat

                    tracing::info!("Found same file in db");
                    let reply_params = ReplyParameters::builder()
                        .message_id(file_processed_info.message_id as i32) // original message id
                        .build();

                    let send_message_params = SendMessageParams::builder()
                        .chat_id(message.chat.id)
                        .text(MESSAGE_FOUND_MSG)
                        .reply_parameters(reply_params)
                        .build();
                    tracing::info!("Message sent to chat");

                    if let Err(e) = api.send_message(&send_message_params).await {
                        //TODO: remove image from db if cannot find original
                        if let frankenstein::Error::Api(e) = e {
                            if e.description == REPLY_NOT_FOUND_ERROR {
                                tracing::warn!("Reply not found, update existing record");
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

                // Download file
                if let Some(file_path) = response.result.file_path.clone() {
                    let file_response =
                        reqwest::get(format!("{}/{}", files_endpoint, file_path)).await;
                    if let Ok(file_response) = file_response {
                        let destination_path =
                            save_file(&file_path, &response, file_response).await?;

                        let (hash_landscape, hash_portrait, hash_square) =
                            indexer.hash_image(&image::open(&destination_path).unwrap());

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
                            let reply_params = ReplyParameters::builder()
                                .message_id(found_result_in_chat.message_id as i32) // original message id
                                .build();

                            let send_message_params = SendMessageParams::builder()
                                .chat_id(message.chat.id)
                                .text(MESSAGE_FOUND_MSG)
                                .reply_parameters(reply_params)
                                .build();

                            if let Err(e) = api.send_message(&send_message_params).await {
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
                            if let Err(e) = fs::remove_file(destination_path).await {
                                tracing::error!("Failed to remove file {}", e);
                                return Err(());
                            }
                            // Extract to FileStorage

                            return Ok(());
                        }

                        // Save to index
                        if let Err(e) = indexer
                            .save_to_index(
                                destination_path.to_str().unwrap_or(""),
                                message.chat.id,
                                message.message_id as i64,
                                &response.result.file_unique_id,
                                (&hash_landscape, &hash_portrait, &hash_square),
                            )
                            .await
                        {
                            tracing::error!("Failed to index image {:?}", e);
                        }
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
