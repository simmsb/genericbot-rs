use diesel::pg::data_types::{PgTimestamp};
use ::schema::*;

#[table_name="guild"]
#[derive(Queryable, Insertable)]
pub struct Guild {
    pub id: i64,
    pub markov_on: bool,
    pub tag_prefix_on: bool,
    pub commands_from: i64,
}

#[table_name="message"]
#[derive(Insertable)]
pub struct NewStoredMessage<'a> {
    pub id: i64,
    pub guild_id: i64,
    pub user_id: i64,
    pub msg: &'a str,
    pub created_at: PgTimestamp,
}

#[table_name="prefix"]
#[derive(Insertable)]
pub struct NewPrefix<'a> {
    pub guild_id: i64,
    pub pre: &'a str,
}

#[table_name="reminder"]
#[derive(Insertable)]
pub struct NewReminder<'a> {
    pub user_id: i64,
    pub channel_id: i64,
    pub text: &'a str,
    pub started: PgTimestamp,
    pub when: PgTimestamp,
}

#[table_name="tag"]
#[derive(Insertable)]
pub struct NewTag<'a> {
    pub author_id: i64,
    pub guild_id: i64,
    pub key: &'a str,
    pub text: &'a str,
}

#[derive(Queryable)]
pub struct StoredMessage {
    pub id: i64,
    pub guild_id: i64,
    pub user_id: i64,
    pub message: String,
    pub created_at: PgTimestamp,
}

#[derive(Queryable)]
pub struct Prefix {
    pub id: i64,
    pub guild_id: i64,
    pub pre: String,
}

#[derive(Queryable)]
pub struct Reminder {
    pub id: i64,
    pub user_id: i64,
    pub channel_id: i64,
    pub text: String,
    pub started: PgTimestamp,
    pub when: PgTimestamp,
}

#[derive(Queryable)]
pub struct Tag {
    pub id: i64,
    pub author_id: i64,
    pub guild_id: i64,
    pub key: String,
    pub text: String,
}
