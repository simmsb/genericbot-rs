use chrono::NaiveDateTime;
use ::schema::*;


#[table_name="guild"]
#[derive(Insertable)]
pub struct NewGuild {
    pub id: i64,
}

#[table_name="message"]
#[derive(Insertable)]
pub struct NewStoredMessage<'a> {
    pub id: i64,
    pub guild_id: i64,
    pub user_id: i64,
    pub msg: &'a str,
    pub created_at: &'a NaiveDateTime,
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
    pub started: &'a NaiveDateTime,
    pub when: &'a NaiveDateTime,
}

#[table_name="tag"]
#[derive(Insertable)]
pub struct NewTag<'a> {
    pub author_id: i64,
    pub guild_id: i64,
    pub key: &'a str,
    pub text: &'a str,
}

#[table_name="command_alias"]
#[derive(Insertable)]
pub struct NewCommandAlias<'a> {
    pub owner_id: i64,
    pub alias_name: &'a str,
    pub alias_value: &'a str,
}

#[table_name="tea_count"]
#[derive(Insertable)]
pub struct NewTeaCount {
    pub user_id: i64,
    pub count: i32,
}

#[derive(Queryable)]
pub struct Guild {
    pub id: i64,
    pub markov_on: bool,
    pub tag_prefix_on: bool,
    pub commands_from: i64,
}

#[derive(Queryable)]
pub struct StoredMessage {
    pub id: i64,
    pub guild_id: i64,
    pub user_id: i64,
    pub message: String,
    pub created_at: NaiveDateTime,
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
    pub started: NaiveDateTime,
    pub when: NaiveDateTime,
}

#[derive(Queryable)]
pub struct Tag {
    pub id: i64,
    pub author_id: i64,
    pub guild_id: i64,
    pub key: String,
    pub text: String,
}

#[derive(Queryable)]
pub struct CommandAlias {
    pub id: i64,
    pub owner_id: i64,
    pub alias_name: String,
    pub alias_value: String,
}
