use std::{ffi::OsStr, ops::Deref, str::FromStr, sync::Arc};

use dotenvy::dotenv;
use frankenstein::{
    client_reqwest::Bot,
    methods::{AnswerCallbackQueryParams, GetFileParams, GetUpdatesParams, SendMessageParams},
    response::MethodResponse,
    types::{CallbackQuery, File, MaybeInaccessibleMessage, Message, ReplyParameters, User},
    updates::UpdateContent,
    AsyncTelegramApi,
};

use img_hashing_bot::{
    data::{CallbackQueryCommand, CallbackQueryData},
    hasher::Indexer,
    keyboards::build_keyboard,
    metrics,
    storage::{s3_storage::S3FileStorage, FileStorage},
    tg_callbacks::{
        process_contra_callback, process_ignore_callback, process_pro_callback,
        process_wrong_callback,
    },
    tracing_setup::init_tracing,
};
use migration::sea_orm::{
    sqlx::{sqlite::SqliteConnectOptions, SqlitePool},
    SqlxSqliteConnector,
};
use tokio::{signal, sync::Mutex};

const MESSAGE_FOUND_MSG: &str = "Эту картинку уже постили тут:";
const REPLY_NOT_FOUND_ERROR: &str = "Bad Request: message to be replied not found";

async fn apply_migrations(db_path: &str) {
    use migration::{Migrator, MigratorTrait};
    let opts = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(opts)
        .await
        .expect("Failed to connect to apply migrations");
    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool);
    Migrator::up(&db, None)
        .await
        .expect("Failed to apply transactions");
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    dotenv().ok();

    let otlp_endpoint =
        &dotenvy::var("OTLP_ENDPOINT").expect("Failed to find OTLP_ENDPOINT env var");
    let otlp_token = &dotenvy::var("OTLP_TOKEN").expect("Failed to find OTLP_TOKEN env var");

    let s3_endpoint = &dotenvy::var("S3_ENDPOINT").expect("Failed to find S3_ENDPOINT env var");
    let s3_bucket = &dotenvy::var("S3_BUCKET").expect("Failed to find S3_BUCKET env var");
    let s3_access_key =
        &dotenvy::var("S3_ACCESS_KEY").expect("Failed to find S3_ACCESS_KEY env var");
    let s3_secret_key =
        &dotenvy::var("S3_SECRET_KEY").expect("Failed to find S3_SECRET_KEY env var");
    let bot_api_token = &dotenvy::var("TELEGRAM_BOT_API_TOKEN")
        .expect("Failed to find TELEGRAM_BOT_API_TOKEN env var");


    let db_path = "./hashes.db";

    apply_migrations(db_path).await;



    let finisher = init_tracing(&otlp_endpoint, &otlp_token);
    let indexer = Arc::new(Mutex::new(Indexer::new(db_path)));

    let storage = Arc::new(Mutex::new(S3FileStorage::new(
        s3_endpoint,
        s3_bucket,
        s3_access_key,
        s3_secret_key,
    )));

    let api = Bot::new(bot_api_token);
    let files_endpoint = format!("https://api.telegram.org/file/bot{bot_api_token}/");

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
                                        let storage = storage.clone();
                                        tokio::spawn(async move {
                                            if message.photo.is_none() {
                                                return;
                                            }
                                            if let Err(e) = process_message(&message, api_clone, &files_endpoint, indexer, storage).await {
                                                tracing::error!("Failed to start message processing: {e}");
                                            }
                                        });

                                }
                                UpdateContent::CallbackQuery(callback_message) => {
                                    let api_clone = api.clone();
                                    let indexer = indexer.clone();
                                    tokio::spawn(async move {
                                        let result = process_callback(&api_clone, &callback_message, indexer).await;
                                        if let Err(err) = result {
                                            tracing::warn!("Failed to process buttons: {err}");
                                        }
                                    });
                                }
                                _ => {
                                    tracing::info!("Other {:?}", update.content);
                                }
                            }
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

#[tracing::instrument(name = "Process new message", skip(api, storage, indexer))]
async fn process_message<T: FileStorage>(
    message: &Message,
    api: Bot,
    files_endpoint: &str,
    indexer: Arc<Mutex<Indexer>>,
    storage: Arc<Mutex<T>>,
) -> Result<(), anyhow::Error> {
    // Skip all replies
    if message.reply_to_message.is_some() {
        tracing::info!("This is reply, ignore");
        return Ok(());
    }

    if let Some(response) = get_image_from_message(&message, &api).await {
        let file_processed_info = {
            let indexer = indexer.lock().await;
            // check by unique id
            indexer
                .is_file_processed_info(&response.file_unique_id, message.chat.id)
                .await
        };

        if let Some(Ok(user_id)) = message.from.clone().map(|f| f.id.try_into()) {
            metrics::mtr_images_count(1, user_id);
        }

        // check image by file_id
        if let Some(file_processed_info) = file_processed_info {
            let mut indexer = indexer.lock().await;

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
                if is_message_removed(&e) {
                    tracing::warn!("Reply not found, update existing record");
                    metrics::mtr_removed_originals_count(1);
                    let hash_record = file_processed_info;
                    // Update hash record of removed message
                    indexer
                        .update_old_hash(hash_record.id, message.chat.id, message.message_id as i64)
                        .await;
                } else {
                    tracing::error!("Failed to send message about same file id {e}");
                }
                return Ok(());
            }
        } else {
            //Existing file not found, process fully

            // Download file
            let file_path = response
                .file_path
                .ok_or(anyhow::format_err!("File path not found in message"))?;

            let storage = storage.clone();
            let storage = storage.lock().await;
            if let Ok(file_uri) = download_file_from_tg(
                &file_path,
                &response.file_unique_id,
                files_endpoint,
                storage.deref(),
            )
            .await
            {
                if let Some(size) = response.file_size {
                    metrics::mtr_image_size(size, message.chat.id);
                }

                let image = &storage
                    .load_file(&file_uri)
                    .await
                    .map_err(|e| anyhow::format_err!("Failed to load image from s3: {e}"))?;

                let mut indexer = indexer.lock().await;

                // Generate hashes
                let (hash_landscape, hash_portrait, hash_square) = indexer.hash_image(image);

                // Search hash in db
                let result = indexer
                    .find_similar_hashes(
                        (&hash_landscape, &hash_portrait, &hash_square),
                        message.chat.id,
                    )
                    .await;

                // Hash found
                if !result.is_empty() {
                    log::info!("Found similar images images {result:?}");
                    let found_result_in_chat = result
                        .first()
                        .ok_or(anyhow::format_err!("Failed to find first image in found"))?;

                    //Check if have same media group - check if same like in found
                    if message.media_group_id.is_some()
                        && message.media_group_id == found_result_in_chat.media_group_id
                    {
                        return Ok(());
                    }

                    if let Some(Ok(user_id)) = message.from.clone().map(|f| f.id.try_into()) {
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
                        if is_message_removed(&e) {
                            tracing::warn!("Reply not found, update existing record");
                            metrics::mtr_removed_originals_count(1);
                            let hash_record = found_result_in_chat;
                            indexer
                                .update_old_hash(
                                    hash_record.id,
                                    message.chat.id,
                                    message.message_id as i64,
                                )
                                .await;

                            //remove hashed image if original removed
                            storage.remove_file(&file_uri).await?;
                        } else {
                            tracing::error!("Failed to send message about same file id {e}");
                        }
                    }
                } else {
                    // Hash not found, save to index
                    if let Err(e) = indexer
                        .save_to_index(
                            &file_uri,
                            message.chat.id,
                            message.message_id as i64,
                            &response.file_unique_id,
                            message.media_group_id.as_deref(),
                            (&hash_landscape, &hash_portrait, &hash_square),
                        )
                        .await
                    {
                        tracing::error!("Failed to index image {e:?}");
                    }
                }
            } else {
                tracing::error!("Failed to upload image to S3");
            }
        }
    } else {
        let text = message.text.clone().unwrap_or("".to_string());
        if text == "/start" {
            return Ok(());
        } else if text == "/help" {
            return Ok(());
        } else {
            return Ok(());
        }
    }

    Ok(())
}

#[tracing::instrument(name = "Download file from tg", skip(storage))]
async fn download_file_from_tg<T: FileStorage>(
    file_path: &str,
    file_id: &str,
    files_endpoint: &str,
    storage: &T,
) -> Result<String, anyhow::Error> {
    let destination_path_str = get_filename(file_path, file_id);
    let tg_file_url = format!("{files_endpoint}/{file_path}");
    let file_uri = storage
        .save_file(&tg_file_url, &destination_path_str)
        .await?;

    Ok(file_uri)
}

#[tracing::instrument(name = "Extract image from message", skip(api))]
async fn get_image_from_message(message: &Message, api: &Bot) -> Option<File> {
    let pics = message.photo.clone()?;
    let best_quality = pics.last()?;
    let params = GetFileParams::builder()
        .file_id(&best_quality.file_id)
        .build();
    let response = api.get_file(&params).await.ok()?;
    Some(response.result)
}

fn is_message_removed(error: &frankenstein::Error) -> bool {
    if let frankenstein::Error::Api(e) = error {
        if e.description == REPLY_NOT_FOUND_ERROR {
            return true;
        }
    }
    false
}

fn get_filename(file_path: &str, file_unique_id: &str) -> String {
    let original_path = std::path::Path::new(file_path);
    let extension = original_path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or("");

    let destination_path_str = format!(
        "{path}.{extension}",
        path = file_unique_id,
        extension = extension
    );
    destination_path_str
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

#[tracing::instrument(name = "Process inline query", skip(api, indexer))]
async fn process_callback(
    api: &Bot,
    query: &CallbackQuery,
    indexer: Arc<Mutex<Indexer>>,
) -> Result<(), anyhow::Error> {
    let result = api
        .answer_callback_query(
            &AnswerCallbackQueryParams::builder()
                .callback_query_id(query.id.clone())
                .build(),
        )
        .await;

    if let Err(err) = result {
        tracing::warn!("Failed to answer callback query");
        return Err(anyhow::format_err!("{}", err));
    }

    let data = query
        .data
        .clone()
        .ok_or(anyhow::format_err!("No inline data"))?;

    let callback_data = CallbackQueryData::from_str(&data)?;

    let username = get_username(&query.from);

    let user_id = query.from.id;

    println!("callback data: {callback_data:?}");

    let maybe_message = query
        .message
        .clone()
        .ok_or(anyhow::format_err!("Failed to find message"))?;
    let message_id = match maybe_message {
        MaybeInaccessibleMessage::Message(message) => message.message_id,
        MaybeInaccessibleMessage::InaccessibleMessage(_) => {
            return Err(anyhow::format_err!("Message is inaccessible"));
        }
    };

    match callback_data.command {
        CallbackQueryCommand::WRONG => {
            match process_wrong_callback(
                api,
                callback_data.args[0],
                i32::try_from(callback_data.args[1]).expect("Failed to cast chat id"),
                message_id,
                indexer.clone(),
            )
            .await
            {
                Ok(_) => tracing::info!("Message update sent"),
                Err(e) => tracing::error!("Failed to update message: {e}"),
            }
        }
        CallbackQueryCommand::IGNORE => {
            match process_ignore_callback(
                api,
                callback_data.args[0],
                i32::try_from(callback_data.args[1]).expect("Failed to cast chat id"),
                message_id,
                indexer.clone(),
            )
            .await
            {
                Ok(_) => tracing::info!("Message update sent"),
                Err(e) => tracing::error!("Failed to update message: {e}"),
            }
        }
        CallbackQueryCommand::PRO => {
            //match process_vote_pro_callback

            // pass: chat_id, message_id, original_message_id???, user_id, username
            match process_pro_callback(callback_data.args[0], user_id, &username, &api, indexer)
                .await
            {
                Ok(_) => tracing::info!("Message update sent"),
                Err(e) => tracing::error!("Failed to update message {e}"),
            }
        }
        CallbackQueryCommand::CON => {
            //match process_vote_con_callback

            // pass: chat_id, message_id, original_message_id???, user_id, username
            match process_contra_callback(callback_data.args[0], user_id, &username, &api, indexer)
                .await
            {
                Ok(_) => tracing::info!("Message update sent"),
                Err(e) => tracing::error!("Failed to update message {e}"),
            }
        }
    }

    Ok(())
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
