use frankenstein::types::{InlineKeyboardButton, InlineKeyboardMarkup, ReplyMarkup};

use crate::models::VotingType;

pub fn build_keyboard(chat_id: i64, message_id: i32) -> ReplyMarkup {
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = Vec::new();

    let mut row = vec![];

    row.push(
        InlineKeyboardButton::builder()
            .text("😡 не дубль")
            .callback_data(format!("wr {chat_id} {message_id}"))
            .build(),
    );
    row.push(
        InlineKeyboardButton::builder()
            .text("😑 забей")
            .callback_data(format!("ig {chat_id} {message_id}"))
            .build(),
    );

    keyboard.push(row);

    let inline_keyboard = InlineKeyboardMarkup::builder()
        .inline_keyboard(keyboard)
        .build();

    ReplyMarkup::InlineKeyboardMarkup(inline_keyboard)
}


pub fn build_vote_keyboard(voting_id: i64, voting_type: &VotingType) -> InlineKeyboardMarkup {
    let (pro_text, contra_text) = match voting_type {
        VotingType::NOTDUPE => ("не баян", "баян"),
        VotingType::IGNORE => ("ПОХУЙ", "похуй"),
    };


    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = Vec::new();

    let mut row = vec![];

    row.push(
        InlineKeyboardButton::builder()
            .text(format!("👍 {pro_text}"))
            .callback_data(format!("pro {voting_id}"))
            .build(),
    );
    row.push(
        InlineKeyboardButton::builder()
            .text(format!("👎 {contra_text}"))
            .callback_data(format!("con {voting_id}"))
            .build(),
    );

    keyboard.push(row);

    InlineKeyboardMarkup::builder()
        .inline_keyboard(keyboard)
        .build()
}
