use serenity::{
    framework::standard::{
        StandardFramework,
    },
    prelude::*,
    utils::MessageBuilder,
};
use diesel;
use diesel::prelude::*;
use itertools::Itertools;
use ::PgConnectionManager;
use ::PrefixCache;


fn get_prefixes(ctx: &Context, g_id: i64) -> Vec<String> {
    use schema::prefix::dsl::*;

    let pool = extract_pool!(&ctx);

    prefix.filter(guild_id.eq(g_id))
                  .select(pre)
                  .load(pool)
                  .expect("Couldn't load prefixes")
}


fn delete_prefix(ctx: &Context, p: &str, g_id: i64) {
    use schema::prefix::dsl::*;

    let pool = extract_pool!(&ctx);

    diesel::delete(prefix
                   .filter(pre.eq(p))
                   .filter(guild_id.eq(g_id)))
        .execute(pool)
        .unwrap();
}


fn add_prefix(ctx: &Context, prefix: &str, g_id: i64) {
    use schema::prefix;
    use models::NewPrefix;

    let pool = extract_pool!(&ctx);

    let pre = NewPrefix {
        guild_id: g_id,
        pre: prefix,
    };

    diesel::insert_into(prefix::table)
        .values(&pre)
        .on_conflict_do_nothing()
        .execute(pool)
        .expect("Couldn't set prefix");
}


command!(add_prefix_cmd(ctx, msg, args) {
    let prefix = args.full_quoted();

    add_prefix(&ctx, &prefix, msg.guild_id().unwrap().0 as i64);

    let resp = MessageBuilder::new()
        .push("Added the prefix: ")
        .push_safe(&prefix)
        .push(" to the list of usable prefixes")
        .build();

    void!(msg.channel_id.say(resp));
});


command!(list_prefixes_cmd(ctx, msg) {
    let prefixes = get_prefixes(&ctx, msg.guild_id().unwrap().0 as i64);

    let resp = MessageBuilder::new()
        .push("Prefixes for this guild: ")
        .push_safe(prefixes.iter().join(", "))
        .build();

    void!(msg.channel_id.say(resp));
});


command!(delete_prefix_cmd(ctx, msg, args) {
    let prefix = args.full_quoted();

    delete_prefix(&ctx, &prefix, msg.guild_id().unwrap().0 as i64);

    let resp = MessageBuilder::new()
        .push("Deleted the prefix: ")
        .push_safe(&prefix)
        .push(" from the list of usable prefixes")
        .build();

    void!(msg.channel_id.say(resp));
});


pub fn setup_prefixes(_client: &mut Client, frame: StandardFramework) -> StandardFramework {
    frame.group("Prefixes",
                |g| g
                .guild_only(true)
                .command("list_prefixes",
                         |c| c
                         .cmd(list_prefixes_cmd)
                         .desc("List prefixes usable in this guild.")
                )
                .command("add_prefix",
                         |c| c
                         .cmd(add_prefix_cmd)
                         .desc("Add a prefix")
                         .example("!!")
                         .usage("{prefix}")
                         .num_args(1)
                )
                .command("delete_prefix",
                         |c| c
                         .cmd(delete_prefix_cmd)
                         .desc("Delete a prefix, if it exists")
                         .example("!!")
                         .usage("{prefix}")
                         .num_args(1)
                )
    )
}
