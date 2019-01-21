use chrono::{Duration, Utc};
// use dotenv;
use models::Reminder;
// use reqwest;
use serenity::{
    model::id::{ChannelId, UserId},
    prelude::*,
    utils,
    utils::{shard_id, MessageBuilder},
};
use std::{
    collections::HashMap,
    sync::{Once, ONCE_INIT},
    thread, time,
};

use PgConnectionManager;

static BOTLIST_UPDATE_START: Once = ONCE_INIT;
static REMINDER_START: Once = ONCE_INIT;

pub fn background_task(ctx: &Context) {
    BOTLIST_UPDATE_START.call_once(|| {
        thread::spawn(move || {
            info!(target: "bot", "Starting botlist updater process");

            let botlist_key = match dotenv::var("DISCORD_BOT_LIST_TOKEN") {
                Ok(x) => x.to_owned(),
                _ => {
                    warn!(target: "bot", "No botlist token set");
                    return;
                }
            };

            let bot_id = log_time!(utils::with_cache(|c| c.user.id), "with_cache_lock: get bot id");

            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(reqwest::header::AUTHORIZATION, botlist_key.parse().unwrap());

            let client = reqwest::Client::builder()
                .default_headers(headers)
                .build()
                .unwrap();

            loop {
                thread::sleep(time::Duration::from_secs(60 * 60)); // every hour
                let (guild_counts, shard_count): (HashMap<_, u32>, _) = log_time!(utils::with_cache(|c| {
                    let mut counts = HashMap::new();

                    for g_id in c.guilds.keys() {
                        *counts.entry(shard_id(g_id.0, c.shard_count))
                               .or_default() += 1;
                    }

                    (counts, c.shard_count)
                }), "with_cache_lock: get guild len");

                for (shard_id, guild_count) in guild_counts {

                    info!(target: "bot", "Sent update to botlist for shard: {}, with count: {}", shard_id, guild_count);

                    let resp = client
                        .post(&format!(
                            "https://discord.bots.gg/api/v1/bots/{}/stats",
                            bot_id
                        ))
                        .json(&json!({ "guildCount": guild_count,
                                       "shardCount": shard_count,
                                       "shardId":    shard_id
                        }))
                        .send();

                    if let Ok(mut resp) = resp {
                        info!(target: "bot", "Response from botlist for shard {}. status: {}, body: {:?}", shard_id, resp.status(), resp.text());
                    }
                }
            }
        });
    });

    REMINDER_START.call_once(|| {
        use diesel;
        use diesel::prelude::*;
        use schema::reminder;

        let delay_period = Duration::seconds(10);
        let zero_duration = time::Duration::new(0, 0);

        let data = ctx.data.clone();

        thread::spawn(move || loop {
            debug!(target: "bot", "Reminder loop");

            let time_limit = Utc::now().naive_utc() + delay_period;

            let pool = &*data
                .lock()
                .get::<PgConnectionManager>()
                .unwrap()
                .get()
                .unwrap();

            if let Ok(reminders) = reminder::dsl::reminder
                .filter(reminder::dsl::when.lt(time_limit))
                .order(reminder::dsl::when)
                .load::<Reminder>(pool)
            {
                if !reminders.is_empty() {
                    info!(target: "bot", "Collected {} reminders.", reminders.len());
                }

                for rem in reminders {
                    let diff = rem
                        .when
                        .signed_duration_since(Utc::now().naive_utc())
                        .to_std()
                        .unwrap_or(zero_duration);

                    thread::sleep(diff);

                    send_reminder_msg(&rem);

                    diesel::delete(reminder::dsl::reminder.find(rem.id))
                        .execute(pool)
                        .unwrap();
                }
            }

            thread::sleep(delay_period.to_std().unwrap());
        });
    });
}

fn send_reminder_msg(rem: &Reminder) {
    use commands::reminders::human_timedelta;

    let diff = rem.when.signed_duration_since(rem.started);

    let content = MessageBuilder::new()
        .user(rem.user_id as u64)
        .push(", ")
        .push(human_timedelta(&diff))
        .push(" ago, you asked me to remind you about: ")
        .push_safe(&rem.text);

    let chan = ChannelId::from(rem.channel_id as u64);
    if chan.say(&content).is_ok() {
        return;
    }

    let user = UserId::from(rem.user_id as u64);
    if let Ok(chan) = user.create_dm_channel() {
        void!(chan.say(&content));
    }
}
