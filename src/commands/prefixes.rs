use serenity::{
    framework::standard::{
        StandardFramework,
    },
    prelude::*,
    utils::MessageBuilder,
    model::{
        permissions::Permissions,
        id::GuildId,
    },
};
use diesel;
use diesel::prelude::*;
use itertools::Itertools;
use ::PgConnectionManager;
use ::PrefixCache;
use utils::say;


fn delete_prefix(ctx: &Context, p: &str, g_id: GuildId) {
    use schema::prefix::dsl::*;

    let mut data = ctx.data.lock();
    {
        let cache = data.get_mut::<PrefixCache>().unwrap();
        if let Some(mut pre_vec) = cache.get_mut(&g_id).map(|l| l.write()) {
            pre_vec.remove_item(&p.to_owned());
        }
    }

    let pool  = &*data.get::<PgConnectionManager>().unwrap().get().unwrap();

    diesel::delete(prefix
                   .filter(pre.eq(p))
                   .filter(guild_id.eq(g_id.0 as i64)))
        .execute(pool)
        .unwrap();
}


fn add_prefix(ctx: &Context, p: &str, g_id: GuildId) {
    use schema::prefix;
    use models::NewPrefix;

    let mut data = ctx.data.lock();
    {
        let cache = data.get_mut::<PrefixCache>().unwrap();
        if let Some(mut pre_vec) = cache.get_mut(&g_id).map(|l| l.write()) {
            pre_vec.push(p.to_owned());
        }
    }

    let pool  = &*data.get::<PgConnectionManager>().unwrap().get().unwrap();

    let pre = NewPrefix {
        guild_id: g_id.0 as i64,
        pre: p,
    };

    diesel::insert_into(prefix::table)
        .values(&pre)
        .on_conflict_do_nothing()
        .execute(pool)
        .expect("Couldn't set prefix");
}


command!(add_prefix_cmd(ctx, msg, args) {
    let prefix = args.full_quoted();

    add_prefix(&ctx, &prefix, msg.guild_id.unwrap());

    let resp = MessageBuilder::new()
        .push("Added the prefix: ")
        .push_safe(&prefix)
        .push(" to the list of usable prefixes");

    void!(say(msg.channel_id, resp));
});


command!(list_prefixes_cmd(ctx, msg) {
    use ::get_prefixes;
    let prefixes_l = get_prefixes(ctx, &msg).unwrap();
    let prefixes = prefixes_l.read();

    let resp = MessageBuilder::new()
        .push("Prefixes for this guild: ")
        .push_safe(prefixes.iter().join(", "));

    void!(say(msg.channel_id, resp));
});


command!(delete_prefix_cmd(ctx, msg, args) {
    let prefix = args.full_quoted();

    delete_prefix(&ctx, &prefix, msg.guild_id.unwrap());

    let resp = MessageBuilder::new()
        .push("Deleted the prefix: ")
        .push_safe(&prefix)
        .push(" from the list of usable prefixes");

    void!(say(msg.channel_id, resp));
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
                         .required_permissions(Permissions::ADMINISTRATOR)
                         .num_args(1)
                )
                .command("delete_prefix",
                         |c| c
                         .cmd(delete_prefix_cmd)
                         .desc("Delete a prefix, if it exists. Note you cannot delete the `generic#` prefix.")
                         .example("!!")
                         .usage("{prefix}")
                         .required_permissions(Permissions::ADMINISTRATOR)
                         .num_args(1)
                )
    )
}
