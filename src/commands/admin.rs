use base64;
use serenity::{
    prelude::*,
    framework::standard::{
        StandardFramework,
        CommandError,
    },
    utils::{
        MessageBuilder,
        with_cache,
    },
    model::{
        channel::Channel,
        id::{GuildId, ChannelId}},
    http,
};
use diesel;
use std::{
    iter,
    collections::HashSet,
};
use diesel::prelude::*;
use ::PgConnectionManager;
use utils::say;
use itertools::Itertools;


command!(set_avatar(_ctx, msg) {
    for att in &msg.attachments {
        let ext = if att.filename.ends_with("png") {
            "png"
        } else if att.filename.ends_with("jpg") || att.filename.ends_with("jpeg") {
            "jpg"
        } else {
            continue;
        };

        let content = att.download()?;
        let b64 = base64::encode(&content);
        let data = format!("data:image/{};base64,{}", ext, b64);

        let profile_iter = iter::once(("avatar".to_owned(), data.into()));

        http::edit_profile(&profile_iter.collect())?;

        void!(say(msg.channel_id, "Set avatar!"));
        return Ok(());
    }
});


fn empty_guilds(ctx: &Context) -> QueryResult<Vec<i64>> {
    use schema::guild::dsl::*;

    let pool = extract_pool!(&ctx);

    guild.filter(commands_from.eq(0))
         .select(id)
         .load(pool)
}


fn drop_guilds(ctx: &Context, guilds: &[i64]) -> usize {
    use schema::guild::dsl::*;

    let pool = extract_pool!(&ctx);

    diesel::delete(guild.filter(id.eq_any(guilds)))
        .execute(pool)
        .unwrap()
}


fn clean_dead_guilds(ctx: &Context, guilds: &[i64]) -> usize {
    use schema::guild::dsl::*;

    let pool = extract_pool!(&ctx);

    diesel::delete(guild.filter(diesel::dsl::not(id.eq_any(guilds))))
        .execute(pool)
        .unwrap()
}

fn find_bot_collection_guilds() -> Vec<GuildId> {
    with_cache(
        |c| c.guilds
             .values()
             .filter_map(
                 |g| {
                     let guild = g.read();
                     let (bot_count, people_count) =
                         guild.members
                              .values()
                              .fold((0, 0), |(b, p), m| if m.user.read().bot {
                                  (b + 1, p)
                              } else {
                                  (b, p + 1)
                              });

                     // only leave a guild if they have more bots than people
                     // and also atleast 10 people, so we don't end up leaving small guilds
                     if bot_count >= people_count && people_count >= 10 {
                         Some(guild.id)
                     } else {
                         None
                     }
                 })
             .collect()
    )
}

command!(clean_guilds(ctx, msg, args) {
    let dry_run = get_arg!(args, single, bool, dry_run, true);

    let ignored_guilds: [i64; 1] = [110373943822540800];

    let mut guilds_to_leave: HashSet<_> = empty_guilds(&ctx)?.into_iter().collect();

    guilds_to_leave.extend(find_bot_collection_guilds()
                           .into_iter()
                           .map(|g_id| g_id.0 as i64));

    for guild in &ignored_guilds {
        guilds_to_leave.remove(&guild);
    }

    let guilds_to_leave_vec: Vec<_> = guilds_to_leave.into_iter().collect();

    if dry_run {
        void!(say(msg.channel_id, format!("Would leave: {} guilds.", guilds_to_leave_vec.len())));
    } else {
        void!(say(msg.channel_id, format!("Leaving: {} guilds.", guilds_to_leave_vec.len())));

        for &guild in &guilds_to_leave_vec {
            let guild_id = GuildId::from(guild as u64);
            void!(guild_id.leave());
        }

        drop_guilds(&ctx, &guilds_to_leave_vec);
        void!(say(msg.channel_id, format!("Left: {} guilds", guilds_to_leave_vec.len())));
    }
});

command!(clean_dead_guilds_cmd(ctx, msg) {
    let guild_ids: Vec<_> = with_cache(
        |c| c.all_guilds()
             .into_iter()
             .map(|g| g.0 as i64)
             .collect());

    let deleted = clean_dead_guilds(&ctx, &guild_ids);

    void!(say(msg.channel_id, format!("Dropped {} dead guilds", deleted)));
});

command!(stop_bot(ctx, msg) {
    use ::ShardManagerContainer;

    void!(say(msg.channel_id, "🤖🔫"));
    let lock = ctx.data.lock();
    let mut manager = lock.get::<ShardManagerContainer>().unwrap().lock();
    manager.shutdown_all();
});


command!(reboot_shard(ctx, msg, args) {
    use ::ShardManagerContainer;
    use serenity::client::bridge::gateway::ShardId;

    let shard = ShardId(get_arg!(args, single, u64, shard));

    void!(say(msg.channel_id, format!("Rebooting shard: {}", shard)));

    let lock = ctx.data.lock();
    let mut manager = lock.get::<ShardManagerContainer>().unwrap().lock();

    manager.restart(shard);
});


command!(admin_stats(ctx, msg) {
    use ::{ThreadPoolCache, ShardManagerContainer};

    let data = ctx.data.lock();
    let dpool = &*data.get::<PgConnectionManager>().unwrap();
    let tpool = &*data.get::<ThreadPoolCache>().unwrap().lock();
    let smanager = data.get::<ShardManagerContainer>().unwrap().lock();

    let inner = MessageBuilder::new()
        .push("Active threads: ")
        .push_line(tpool.active_count())
        .push("Queued threads: ")
        .push_line(tpool.queued_count())
        .push("DB Connections: ")
        .push_line(dpool.state().connections)
        .push_line(format!("Shards: {:?}", smanager.shards_instantiated()));

    let resp = MessageBuilder::new()
        .push_codeblock(inner, None);

    void!(say(msg.channel_id, resp));
});

command!(leave_guild_cmd(_ctx, _msg, args) {
    let guild_id = GuildId::from(get_arg!(args, single_quoted, u64, guild_id));

    if let Some(guild) = guild_id.to_guild_cached() {
        let guild = guild.read();
        info!(target: "bot", "Leaving guild: {}", guild.name);
        return Ok(());
    }

    void!(guild_id.leave());

    warn!(target: "bot", "Couldn't leave guild: {}", guild_id);
});

command!(leave_guild_from_channel_cmd(_ctx, _msg, args) {
    let channel_id = ChannelId::from(get_arg!(args, single_quoted, u64, guild_id));

    if let Some(Channel::Guild(channel)) = channel_id.to_channel_cached() {
        let channel = channel.read();
        if let Some(guild) = channel.guild_id.to_guild_cached() {
            let guild = guild.read();
            info!(target: "bot", "Leaving guild: {}", guild.name);
        }
        void!(channel.guild_id.leave());
        return Ok(());
    }
    warn!(target: "bot", "Couldn't leave guild by chan: {}", channel_id);
});

fn insert_block(ctx: &Context, g_id: Option<i64>, c_id: Option<i64>) {
    use models::NewBlock;
    use schema::blocked_guilds_channels;

    let pool = extract_pool!(&ctx);

    let block = NewBlock {
        guild_id: g_id,
        channel_id: c_id,
    };

    diesel::insert_into(blocked_guilds_channels::table)
        .values(&block)
        .execute(pool)
        .expect("Failed to insert block");
}

command!(block_guild_cmd(ctx, _msg, args) {
    let guild_id = get_arg!(args, single_quoted, u64, guild_id) as i64;

    insert_block(&ctx, Some(guild_id), None);
});

command!(block_chan_cmd(ctx, _msg, args) {
    let chan_id = get_arg!(args, single_quoted, u64, chan_id) as i64;

    insert_block(&ctx, None, Some(chan_id));
});

command!(guild_info_cmd(_ctx, msg, args) {
    let guild_id = get_arg!(args, single_quoted, u64, guild_id);
    let guild_lock = GuildId::from(guild_id).to_guild_cached().ok_or("Could not read guild")?;
    let guild = guild_lock.read();

    let channel_list: Vec<_> = guild.channels.values().map(|c| c.read()).collect();
    let channel_name_list = channel_list.iter().map(|c| &c.name).join(", ");

    let message = MessageBuilder::new()
        .push_bold("Name: ")
        .push_line(&guild.name)
        .push_bold("Channels: ")
        .push_line(channel_name_list)
        .push_bold("Member count: ")
        .push_line(guild.member_count);

    void!(say(msg.channel_id, message));
});

pub fn setup_admin(_client: &mut Client, frame: StandardFramework) -> StandardFramework {
    frame.group("Admin",
                |g| g
                .owners_only(true)
                .help_available(false)
                .command(
                    "set_avatar", |c| c
                        .cmd(set_avatar)
                        .desc("Set the bot's avatar.")
                )
                .command(
                    "clean_guilds", |c| c
                        .cmd(clean_guilds)
                        .desc("Cleanup guilds from bot db. Does a dry calculation by default.")
                        .usage("{non-dry}")
                )
                .command(
                    "stop_bot", |c| c
                        .cmd(stop_bot)
                        .desc("Bye")
                )
                .command(
                    "reboot_shard", |c| c
                        .cmd(reboot_shard)
                        .desc("Restart a shard")
                )
                .command(
                    "admin_stats", |c| c
                        .cmd(admin_stats)
                        .desc("Administrator stats")
                )
                .command(
                    "clean_dead_guilds", |c| c
                        .cmd(clean_dead_guilds_cmd)
                        .desc("Delete guilds that the bot is no longer in from the db.")
                )
                .command(
                    "leave_guild", |c| c
                        .cmd(leave_guild_cmd)
                        .desc("Leave a guild by id.")
                )
                .command(
                    "leave_guild_chan", |c| c
                        .cmd(leave_guild_from_channel_cmd)
                        .desc("Leave a guild by a channel id.")
                )
                .command(
                    "block_guild", |c| c
                        .cmd(block_guild_cmd)
                        .desc("Block a guild by id.")
                )
                .command(
                    "block_chan", |c| c
                        .cmd(block_chan_cmd)
                        .desc("Block a guild by a channel id.")
                )
                .command(
                    "guild_info", |c| c
                        .cmd(guild_info_cmd)
                        .desc("Get info on a guild.")
                )
    )
}
