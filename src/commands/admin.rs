use base64;
use serenity::{
    prelude::*,
    framework::standard::{
        StandardFramework,
        CommandError,
    },
    utils::{
        MessageBuilder,
    },
};
use diesel;
use diesel::prelude::*;
use ::PgConnectionManager;
use utils::say;


command!(set_avatar(ctx, msg) {
    for att in msg.attachments.iter() {
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

        ctx.edit_profile(
            |e| e.avatar(Some(&data))
        )?;

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


fn drop_guilds(ctx: &Context, guilds: &Vec<i64>) {
    use schema::guild::dsl::*;

    let pool = extract_pool!(&ctx);

    diesel::delete(guild.filter(id.eq_any(guilds))).execute(pool).unwrap();
}


command!(clean_guilds(ctx, msg, args) {
    let dry_run = get_arg!(args, single, bool, dry_run, true);

    let ignored_guilds: [i64; 1] = [110373943822540800];

    let mut guilds_to_leave = empty_guilds(&ctx)?;

    for guild in ignored_guilds.iter() {
        guilds_to_leave.remove_item(&guild);
    }

    if dry_run {
        void!(say(msg.channel_id, format!("Would leave: {} guilds.", guilds_to_leave.len())));
    } else {
        drop_guilds(&ctx, &guilds_to_leave);
        void!(say(msg.channel_id, format!("Leaving: {} guilds.", guilds_to_leave.len())));
    }

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
        .push_line(format!("Shards: {:?}", smanager.shards_instantiated()))
        .build();

    let resp = MessageBuilder::new()
        .push_codeblock(inner, None)
        .build();

    void!(say(msg.channel_id, resp));
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
                        .num_args(0)
                )
                .command(
                    "clean_guilds", |c| c
                        .cmd(clean_guilds)
                        .desc("Cleanup guilds from bot db. Does a dry calculation by default.")
                        .usage("{non-dry}")
                        .max_args(1)
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
    )
}
