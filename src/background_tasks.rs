use serenity::{
    prelude::*,
    utils,
};
use serenity;
use std::{
    sync::{
        Once,
        ONCE_INIT
    },
    thread,
    time,
};
use dotenv;
use ::PgConnectionManager;
use reqwest;
use models::Reminder;
use chrono::{Utc, Duration};


static BOTLIST_UPDATE_START: Once = ONCE_INIT;
static REMINDER_START: Once = ONCE_INIT;


pub fn background_task(ctx: &Context) {
    BOTLIST_UPDATE_START.call_once(|| {
        thread::spawn(
            move || {
                info!(target: "bot", "Starting botlist updater process");
                let botlist_key = match dotenv::var("DISCORD_BOT_LIST_TOKEN") {
                    Ok(x) => x.to_owned(),
                    _     => {
                        warn!(target: "bot", "No botlist token set");
                        return;
                    },
                };

                let bot_id = utils::with_cache(|c| c.user.id);
                let mut headers = reqwest::header::Headers::new();
                headers.set(reqwest::header::Authorization(botlist_key.to_owned()));
                let client = reqwest::Client::builder()
                    .default_headers(headers)
                    .build()
                    .unwrap();

                loop {
                    {
                        let guild_count = utils::with_cache(|c| c.all_guilds().len());

                        info!(target: "bot", "Sent update to botlist, with count: {}", guild_count);

                        let resp = client.post(&format!("https://bots.discord.pw/api/bots/{}/stats", bot_id))
                                         .body(format!(r#"{{"server_count": {}}}"#, guild_count))
                                         .send();

                        if let Ok(mut resp) = resp {
                            info!(target: "bot", "Response from botlist. status: {}, body: {:?}", resp.status(), resp.text());
                        }
                    }

                    thread::sleep(time::Duration::from_secs(60 * 60));  // every hour
                }});
    });

    let pool = ctx.data.lock().get::<PgConnectionManager>().unwrap().clone();
    let delay_period = Duration::seconds(10);
    REMINDER_START.call_once(|| {
        use schema::reminder;
        use diesel::prelude::*;
        use diesel;

        thread::spawn(
            move || loop {

                let time_limit = Utc::now().naive_utc() + delay_period;

                let pool = &*pool.get().unwrap();

                if let Ok(reminders) = reminder::dsl::reminder
                    .filter(reminder::dsl::when.lt(time_limit))
                    .order(reminder::dsl::when)
                    .load::<Reminder>(pool)
                {
                    if !reminders.is_empty() {
                        info!(target: "bot", "Collected {} reminders.", reminders.len());
                    }

                    for rem in reminders {
                        let diff = rem.when.signed_duration_since(Utc::now().naive_utc());
                        let diff = match diff.to_std() {
                            Ok(diff) => diff,
                            _        => time::Duration::new(0, 0),
                        };

                        thread::sleep(diff);

                        send_reminder_msg(&rem);

                        diesel::delete(reminder::dsl::reminder.find(rem.id))
                            .execute(pool).unwrap();
                    }

                }

                thread::sleep(delay_period.to_std().unwrap());
            }
        );
    });
}


fn send_reminder_msg(rem: &Reminder) {
    use commands::reminders::human_timedelta;
    use serenity::utils::MessageBuilder;

    let diff = rem.when.signed_duration_since(rem.started);

    let content = MessageBuilder::new()
        .user(rem.user_id as u64)
        .push(", ")
        .push(human_timedelta(&diff))
        .push(" ago, you asked me to remind you about: ")
        .push_safe(&rem.text)
        .build();

    let chan = serenity::model::id::ChannelId::from(rem.channel_id as u64);
    if chan.say(&content).is_ok() {
        return;
    }

    let user = serenity::model::id::UserId::from(rem.user_id as u64);
    if let Ok(chan) = user.create_dm_channel() {
        let _ = chan.say(&content);
    }
}
