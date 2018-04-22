use base64;
use serenity::{
    prelude::*,
    framework::standard::{
        StandardFramework,
    },
};
use diesel;
use diesel::prelude::*;
use ::PgConnectionManager;


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

        void!(msg.channel_id.say("Set avatar!"));
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
        void!(msg.channel_id.say(format!("Would leave: {} guilds.", guilds_to_leave.len())));
    } else {
        drop_guilds(&ctx, &guilds_to_leave);
        void!(msg.channel_id.say(format!("Leaving: {} guilds.", guilds_to_leave.len())));
    }

});


command!(stop_bot(ctx, msg) {
    use ::ShardManagerContainer;

    void!(msg.channel_id.say("🤖🔫"));
    let lock = ctx.data.lock();
    let mut manager = lock.get::<ShardManagerContainer>().unwrap().lock();
    manager.shutdown_all();
});


pub fn setup_admin(_client: &mut Client, frame: StandardFramework) -> StandardFramework {
    frame.group("Admin",
                |g| g
                .owners_only(true)
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
    )
}
