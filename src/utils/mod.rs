use serenity::{
    model::{
        id::{GuildId, UserId, MessageId, ChannelId},
        channel::Message,
        guild::Member,
    },
    framework::standard::{
        Args,
        CommandOptions,
    },
    builder::CreateMessage,
};
use serenity::prelude::*;
use serenity;
use std::fmt::Display;
use serde_json;
use itertools::Itertools;


#[macro_use]
pub mod macros;
pub mod markov;


pub fn names_for_members<U, G>(u_ids: &[U], g_id: G) -> Vec<String>
    where U: Into<UserId> + Copy,
          G: Into<GuildId> + Copy,
{
    use serenity::{
        utils::with_cache,
    };

    fn backup_getter<U>(u_id: U) -> String
        where U: Into<UserId> + Copy,
    {
        match u_id.into().get() {
            Ok(u) => u.name,
            _     => u_id.into().to_string(),
        }
    }

    with_cache(
        |cache| cache.guild(g_id).map(|g| {
            let members = &g.read().members;
            u_ids.iter().map(
                |&id| members.get(&id.into()).map_or_else(
                    || backup_getter(id),
                    |m| m.display_name().to_string()))
                           .collect()
        })).unwrap_or_else(|| u_ids.iter().map(|&id| backup_getter(id)).collect())
}


pub fn and_comma_split<T: AsRef<str>>(m: &[T]) -> String {
    let len = m.len();

    let res = match m {
        [] => "".to_owned(),
        [a] => a.as_ref().to_owned(),
        _ => {
            let mut res = String::new();
            let mut iter = m.into_iter();
            res.push_str(&iter.take(len - 1).map(|s| s.as_ref()).join(", "));
            res.push_str(" and ");
            res.push_str(m[len - 1].as_ref());
            res
        },
    };

    return res;
}


pub fn insert_missing_guilds(ctx: &Context) {
    use diesel;
    use diesel::prelude::*;
    use models::NewGuild;
    use schema::guild;
    use ::PgConnectionManager;
    use serenity::utils::with_cache;

    let pool = extract_pool!(&ctx);

    let guilds: Vec<_> = with_cache(|c| c.all_guilds().iter().map(
        |&g| NewGuild { id: g.0 as i64 }
    ).collect());

    diesel::insert_into(guild::table)
        .values(&guilds)
        .on_conflict_do_nothing()
        .execute(pool)
        .expect("Error building any missing guilds.");
}


pub struct HistoryIterator {
    last_id: Option<MessageId>,
    channel: ChannelId,
    message_vec: Vec<Message>,
}


/// An iterator over discord messages, runs forever through all the messages in a channel's history
impl HistoryIterator {
    pub fn new(c_id: ChannelId) -> Self {
        HistoryIterator { last_id: None, channel: c_id, message_vec: Vec::new() }
    }
}


impl Iterator for HistoryIterator {
    type Item = Message;
    fn next(&mut self) -> Option<Message> {
        // no messages, get some more
        if self.message_vec.is_empty() {
            match self.channel.messages(
                |g| match self.last_id {
                    Some(id) => g.before(id),
                    None     => g
                }) {
                Ok(messages) => {
                    if messages.is_empty() {
                        // no more messages to get, end iterator here
                        return None;
                    }
                    self.message_vec.extend(messages);
                    self.last_id = self.message_vec.last().map(|m| m.id);
                },
                Err(why) => panic!(format!("Couldn't get messages: {}, aborting.", why)),
            }
        }

        let m = self.message_vec.pop();
        if m.is_none() {
            panic!("Messages didn't exist? aborting.");
        }
        return m;
    }
}


pub fn try_resolve_user(s: &str, g_id: GuildId) -> Result<Member, ()> {
    if let Some(g) = g_id.find() {
        let guild = g.read();

        if let Ok(u) = s.parse::<UserId>() {
            return guild.member(u).map_err(|_| ());
        }

        return guild.member_named(s).map(|m| m.clone()).ok_or(());
    } else {
        return Err(());
    }
}


pub fn nsfw_check(_: &mut Context, msg: &Message, _: &mut Args, _: &CommandOptions) -> bool {
    msg.channel_id.find().map_or(false, |c| c.is_nsfw())
}


pub fn send_message<F>(chan_id: ChannelId, f: F) -> serenity::Result<()>
    where F: FnOnce(CreateMessage) -> CreateMessage {
    use ::{MESSENGER_SOCKET, connect_socket};
    use serde::Serialize;
    use rmp_serde::Serializer;
    use std::io::Write;

    let msg = f(CreateMessage::default());
    let map = serenity::utils::vecmap_to_json_map(msg.0);

    Message::check_content_length(&map)?;
    Message::check_embed_length(&map)?;

    let object = &serde_json::Value::Object(map);
    let content = serde_json::to_string(&object)?;

    let mut buf = Vec::new();

    (chan_id, content).serialize(&mut Serializer::new(&mut buf)).unwrap();

    let mut socket = MESSENGER_SOCKET.lock();

    if socket.is_none() {
        *socket = connect_socket();
    } // try to connect once

    let use_fallback = match socket.as_mut() {
        Some(skt) => skt.write_all(buf.as_slice()).is_err(),
        _         => true,
    };

    if use_fallback {
        serenity::http::send_message(chan_id.0, &object)?;
    }

    Ok(())
}


pub fn say<D: Display>(chan_id: ChannelId, content: D) -> serenity::Result<()> {
    send_message(chan_id, |m| m.content(content))
}
