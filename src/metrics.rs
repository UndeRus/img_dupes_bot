use std::u64;

use opentelemetry::{global, metrics::Meter, KeyValue};
use quanta::Clock;

fn meter() -> Meter {
    
    global::meter("img dupes tgbot")
}

fn mtr_exec_time(name: &'static str) -> impl Fn() {
    let clock = Clock::new();
    let now = clock.now();
    let time_metric = meter().u64_gauge(name).build();
    move || {
        let duration = clock.now().duration_since(now);
        time_metric.record(
            duration.as_millis().try_into().unwrap_or(u64::MAX),
            &[],
        );
    }
}

fn mtr_count(name: &'static str, count: u64) {
    let count_metric = meter().u64_counter(name).build();
    count_metric.add(count, &[]);
}

fn mtr_value(name: &'static str, value: u64) {
    let count_metric = meter().u64_gauge(name).build();
    count_metric.record(value, &[]);
}

pub fn mtr_find_similar_hashes_time() -> impl Fn() {
    mtr_exec_time("find_similar_hashes_time")
}

pub fn mtr_images_count(count: u64, user_id: i64) {
    let count_metric = meter().u64_counter("images_count").build();
    count_metric.add(count, &[KeyValue::new("user_id", user_id)]);
}

pub fn mtr_samefiles_count(count: u64) {
    mtr_count("images_samefiles_count", count);
}

pub fn mtr_removed_originals_count(count: u64) {
    mtr_count("removed_originals_count", count);
}

pub fn mtr_image_size(size: u64, chat_id: i64) {
    let count_metric = meter().u64_gauge("image_size").build();
    count_metric.record(size, &[KeyValue::new("chat_id", chat_id)]);
}

pub fn mtr_message_hashing_time() -> impl Fn() {
    mtr_exec_time("message_hashing_time")
}

pub fn mtr_is_file_processed_info_query_time() -> impl Fn() {
    mtr_exec_time("is_file_processed_info_query_time")
}

pub fn mtr_duplicate_count(count: u64, chat_id: i64, user_id: i64) {
    let count_metric = meter().u64_counter("duplicate_count").build();
    count_metric.add(
        count,
        &[
            KeyValue::new("chat_id", chat_id),
            KeyValue::new("user_id", user_id),
        ],
    );
}
