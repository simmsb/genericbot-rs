use serenity::{
    prelude::*,
    model::{
        channel::Message,
        permissions::Permissions,
    },
    framework::standard::{
        StandardFramework,
        CommandError,
    },
    utils::{
        with_cache,
        MessageBuilder,
    },
};
use diesel::prelude::*;
use diesel;
use ::PgConnectionManager;
use models::Tag;
use regex::Regex;
use chrono::NaiveDateTime;


fn recognise_date(date: &str) -> NaiveDateTime {

    // parse out jan(uary) ... stuff etc
    panic!("NotImplemented");

}
