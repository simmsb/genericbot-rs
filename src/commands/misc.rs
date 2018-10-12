use chrono::{NaiveDateTime, Utc};
use diesel;
use diesel::prelude::*;
use procinfo;
use rand;
use rand::Rng;
use serenity::{
    framework::standard::StandardFramework,
    prelude::*,
    utils::{with_cache, MessageBuilder},
};
use std::num::Wrapping;
use std::time;
use utils::{and_comma_split, say, send_message, try_resolve_user};
use whirlpool::{Digest, Whirlpool};
use PgConnectionManager;

fn process_usage() -> f64 {
    use std::thread;
    let start_measure = procinfo::pid::stat_self().unwrap().utime;
    thread::sleep(time::Duration::from_millis(100));
    let end_measure = procinfo::pid::stat_self().unwrap().utime;

    let diff = end_measure - start_measure;
    diff as f64 / 0.1 // util seconds / 100ms per second
}

command!(status_cmd(ctx, msg) {
    use ::{StartTime, CmdCounter};

    let mem_usage = procinfo::pid::statm_self().ok().map_or(0, |p| p.resident) / 1000;
    let cpu_usage = process_usage();
    let uptime = {
        let &start = ctx.data.lock().get::<StartTime>().unwrap();
        let now = Utc::now().naive_utc();

        now.signed_duration_since(start)
    };

    let cmd_count = {
        let lock = ctx.data.lock();
        let count = *lock.get::<CmdCounter>().unwrap().read();
        count
    };

    let (g_c, c_c, u_c, s_c) = log_time!(with_cache(
        |c| {
            let g_c = c.all_guilds().len();
            let c_c = c.channels.len();
            let u_c = c.users.len();
            let s_c = c.shard_count;
            (g_c, c_c, u_c, s_c)
        }), "with_cache: get bot stats");

    let (u_days, u_hours, u_min, u_sec) = (
        uptime.num_days(),
        uptime.num_hours() % 24,
        uptime.num_minutes() % 60,
        uptime.num_seconds() % 60,
    );

    let uptime_str = format!("{}d, {}h, {}m, {}s", u_days, u_hours, u_min, u_sec);

    send_message(msg.channel_id,
        |m| m.embed(
            |e| e
                .title("genericbot stats")
                .colour(0x2C78C8)
                .field("Uptime", uptime_str, true)
                .field("Guild count", g_c, true)
                .field("Channel count", c_c, true)
                .field("User count", u_c, true)
                .field("Commands executed", cmd_count, true)
                .field("Shard count", s_c, true)
                .field("Cpu usage", format!("{:.1}%", cpu_usage), true)
                .field("Mem usage", format!("{:.2}MB", mem_usage), true)
        ))?;
});

command!(q(_ctx, msg) {
    void!(say(msg.channel_id, rand::thread_rng()
                             .choose(&["Yes", "No"])
                             .unwrap()));
});

command!(message_owner(ctx, msg, args) {
    use ::OwnerId;
    let text = args.full();

    if text.is_empty() {
        return Err("Empty message".into());
    }

    let message = MessageBuilder::new()
        .push("Private message: ")
        .push_mono_line_safe(text)
        .push("From: ")
        .push(&msg.author.tag())
        .push(format!("({})", msg.author.id));

    let message = if let Some(guild) = msg.guild_id {
        message.push(" in guild: ")
               .push(guild)
    } else {
        message
    };

    let lock = ctx.data.lock();
    let user = &lock.get::<OwnerId>().unwrap();
    user.direct_message(|m| m.content(message))?;
});

macro_rules! x_someone {
    ( $name:ident, $send_msg:expr, $err:expr ) => (
        command!($name(_ctx, msg, args) {
            let users: Vec<_> = args.multiple_quoted::<String>()
                .map(|u| u.into_iter()
                     .filter_map(|s| try_resolve_user(&s, msg.guild_id.unwrap()).ok())
                     .collect())
                .unwrap_or_else(|_| Vec::new());

            let res = if !users.is_empty() {
                let mentions: Vec<_> = users.into_iter().map(|u| u.mention()).collect();
                let result = and_comma_split(&mentions);
                format!($send_msg, msg.author.mention(), result)
            } else {
                $err.to_string()
            };

            say(msg.channel_id, res)?;
        });
    )
}

x_someone!(hug, "{} hugs {}!", "You can't hug nobody!");
x_someone!(slap, "{} slaps {}! B..Baka!!!", "Go slap yourself you baka");
x_someone!(
    kiss,
    "{} Kisses {}! Chuuuu!",
    "DW anon you'll find someone to love some day!"
);

command!(rate(_ctx, msg, args) {
    let asked = args.full().trim();
    let result = Whirlpool::digest(&asked.as_bytes());
    let sum: Wrapping<u8> = result.into_iter().map(Wrapping).sum();

    let modulus = sum % Wrapping(12);

    void!(say(msg.channel_id, format!("I rate {}: {}/10", asked, modulus)));
});

fn id_to_ts(id: u64) -> NaiveDateTime {
    let offset_sec = (id >> 22) / 1000;
    let ns = (id >> 22) % 1000;

    let secs = offset_sec + 1_420_070_400;

    NaiveDateTime::from_timestamp(secs as i64, ns as u32 * 1_000_000)
}

command!(ping_cmd(_ctx, msg) {
    let recvd = Utc::now().naive_utc();
    let created = id_to_ts(msg.id.0);

    if let Ok(mut tmp) = msg.channel_id.say("...") {
        let send_to_recv = recvd.signed_duration_since(created);
        let send_to_repl = id_to_ts(tmp.id.0).signed_duration_since(created);

        let reply = MessageBuilder::new()
            .push("Send to recv: ")
            .push(send_to_recv.num_milliseconds())
            .push_line("ms")
            .push("Send to reply: ")
            .push(send_to_repl.num_milliseconds())
            .push_line("ms");

        void!(tmp.edit(|m| m.content(reply)));
    }
});

command!(stando(_ctx, msg) {
    let menacing = format!("***{}***", "ゴ".repeat(200));
    let out = MessageBuilder::new()
        .push_line(&menacing)
        .push("ＴＨＩＳ 　ＭＵＳＴ 　ＢＥ 　ＴＨＥ 　ＷＯＲＫ 　ＯＦ 　ＡＮ 　ＥＮＥＭＹ 「")
        .mention(&msg.author)
        .push_line("」*！！*")
        .push(&menacing);

    void!(say(msg.channel_id, out));
});

fn increment_tea_count(ctx: &Context, u_id: i64) -> i32 {
    use models::NewTeaCount;
    use schema::tea_count::dsl::*;

    let pool = extract_pool!(&ctx);

    let to_insert = NewTeaCount {
        user_id: u_id,
        count: 1,
    };

    diesel::insert_into(tea_count)
        .values(&to_insert)
        .on_conflict(user_id)
        .do_update()
        .set(count.eq(count + 1))
        .returning(count)
        .get_result(pool)
        .expect("Failed to update tea count.")
}

fn get_tea_count(ctx: &Context, u_id: i64) -> i32 {
    use schema::tea_count::dsl::*;

    let pool = extract_pool!(&ctx);

    tea_count
        .find(u_id)
        .select(count)
        .first(pool)
        .unwrap_or(0)
}

command!(tea_plus_plus_cmd(ctx, msg) {
    let count = increment_tea_count(&ctx, msg.author.id.0 as i64);

    void!(say(msg.channel_id, format!("Your tea count is now: {}", count)));
});

command!(tea_count_cmd(ctx, msg) {
    let count = get_tea_count(&ctx, msg.author.id.0 as i64);

    void!(say(msg.channel_id, format!("Your tea count is: {}", count)));
});

command!(count_generics_cmd(_ctx, msg) {
    let count = with_cache(
        |c| c.guilds
             .values()
             .filter(|g| g.read().name.contains("generic"))
             .count());

    void!(say(msg.channel_id, format!("There are {} guilds that have generic in their name", count)));
});

pub fn setup_misc(_client: &mut Client, frame: StandardFramework) -> StandardFramework {
    frame
        .group("Misc",
               |g| g
               .command("stats", |c| c
                        .cmd(status_cmd)
                        .desc("Bot stats")
                        .batch_known_as(&["status"])
               )
               .command("q", |c| c
                        .cmd(q)
                        .desc("Ask a question")
               )
               .command("message_owner", |c| c
                        .cmd(message_owner)
                        .desc("Send a message to the bot owner.")
               )
               .command("hug", |c| c
                        .cmd(hug)
                        .desc("Hug someone")
                        .guild_only(true)
               )
               .command("slap", |c| c
                        .cmd(slap)
                        .desc("Slap a bitch")
                        .guild_only(true)
               )
               .command("kiss", |c| c
                        .cmd(kiss)
                        .desc("Kiss someone")
                        .guild_only(true)
               )
               .command("rate", |c| c
                        .cmd(rate)
                        .desc("Rate something.")
               )
               .command("ping", |c| c
                        .cmd(ping_cmd)
                        .desc("Ping discord")
               )
        )
        .group("Hidden",
               |g| g
               .help_available(false)
               .command("stando", |c| c
                        .cmd(stando)
                        .desc("An enemy stand!")
               )
               .command("tea++", |c| c
                        .cmd(tea_plus_plus_cmd)
                        .desc("tea += 1")
               )
               .command("tea_count", |c| c
                        .cmd(tea_count_cmd)
                        .desc("get your tea count")
               )
               .command("generic_count", |c| c
                        .cmd(count_generics_cmd)
                        .desc("what is this bot useful for anyway?")
               )
        )
}
