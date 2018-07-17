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
use itertools::Itertools;
use utils::say;


fn get_tag(ctx: &Context, g_id: i64, tag_key: &str) -> QueryResult<Tag> {
    use schema::tag::dsl::*;

    let pool = extract_pool!(&ctx);

    tag.filter(guild_id.eq(&g_id))
       .filter(key.eq(tag_key))
       .first(pool)
}


fn insert_tag(ctx: &Context, msg: &Message, key: &str, content: &str) {
    use schema::tag;
    use models::NewTag;

    let new_tag =  NewTag {
        author_id: msg.author.id.0 as i64,
        guild_id: msg.guild_id.unwrap().0 as i64,
        key,
        text: content,
    };

    let pool = extract_pool!(&ctx);

    diesel::insert_into(tag::table)
        .values(&new_tag)
        .execute(pool)
        .expect("Couldn't insert tag");
}


fn delete_tag_do(ctx: &Context, tag_id: i64) {
    use schema::tag::dsl::*;

    let pool = extract_pool!(&ctx);

    diesel::delete(tag.filter(id.eq(tag_id)))
        .execute(pool)
        .unwrap();
}


fn get_tags_range(ctx: &Context, g_id: i64, page: i64) -> Vec<Tag> {
    use schema::tag::dsl::*;

    let pool = extract_pool!(&ctx);

    let start = page * 20;
    tag.filter(guild_id.eq(&g_id))
       .order(key.asc())
       .offset(start)
       .limit(20)
       .load(pool)
       .unwrap()
}


fn get_tag_count(ctx: &Context, g_id: i64) -> i64 {
    use schema::tag::dsl::*;

    let pool = extract_pool!(&ctx);

    tag.filter(guild_id.eq(&g_id))
       .count()
       .get_result(pool)
       .unwrap()
}


fn set_auto_tags(ctx: &Context, g_id: i64, value: bool) {
    use schema::guild::dsl::*;

    let pool = extract_pool!(&ctx);

    diesel::update(guild.find(&g_id))
        .set(tag_prefix_on.eq(value))
        .execute(pool)
        .unwrap();
}


command!(add_tag(ctx, msg, args) {
    let key = get_arg!(args, single_quoted, String, key);
    let value = args.iter::<String>().map(|s| s.unwrap()).join(" ");

    if let Ok(t) = get_tag(&ctx, msg.guild_id.unwrap().0 as i64, &key) {
        void!(say(msg.channel_id, format!("The tag: {} already exists", t.key)));
    } else if key.len() >= 50 {
        void!(say(msg.channel_id, "Tag keys cannot be longer than 50 characters."));
    } else {
        insert_tag(&ctx, &msg, &key, &value);
        void!(say(msg.channel_id, format!("Created tag: {} with content: {}!", key, value)));
    }
});


command!(tag(ctx, msg, args) {
    let key = get_arg!(args, multiple, String, key).join(" ");

    if let Ok(t) = get_tag(&ctx, msg.guild_id.unwrap().0 as i64, &key) {
        void!(say(msg.channel_id, t.text));
    } else {
        void!(say(msg.channel_id, "This tag does not exist."));
    }
});


command!(delete_tag(ctx, msg, args) {
    let key = get_arg!(args, multiple, String, key).join(" ");

    if let Ok(t) = get_tag(&ctx, msg.guild_id.unwrap().0 as i64, &key) {

        let has_manage_messages = with_cache(
            |cache| cache.guild(msg.guild_id.unwrap()).map_or(
                false,
                |g| g.read().member_permissions(msg.author.id).manage_messages()
            )
        );

        if has_manage_messages || (t.author_id as u64 == msg.author.id.0) {
            delete_tag_do(&ctx, t.id);
            void!(say(msg.channel_id, format!("Deleted tag of name: {}.", t.key)));
        } else {
            void!(say(msg.channel_id, "You are not the owner of this tag or do not have manage messages."));
        }
    } else {
        void!(say(msg.channel_id, "That tag does not exist."));
    }
});


command!(list_tags(ctx, msg, args) {
    use std::cmp;
    use utils::names_for_members;

    let page = args.single::<i64>().unwrap_or(1) - 1;

    if page < 0 {
        void!(say(msg.channel_id, "That page does not exist."));
        return Ok(());
    }

    let start = page * 20;
    let last = (page + 1) * 20 - 1;
    let tag_list: Vec<Tag> = get_tags_range(&ctx, msg.guild_id.unwrap().0 as i64, page as i64);
    let tag_count = get_tag_count(&ctx, msg.guild_id.unwrap().0 as i64);

    if start > tag_count {
        void!(say(msg.channel_id, format!("The requested page ({}) is greater than the number of pages ({}).", page, last / 20)));
        return Ok(());
    }

    let user_ids: Vec<u64> = tag_list.iter().map(|t| t.author_id as u64).collect();

    let user_names = names_for_members(&user_ids, msg.guild_id.unwrap());

    let tag_content = user_names
        .into_iter()
        .zip(tag_list)
        .enumerate().map(|(i, (name, tag_v))| format!("{:>3} | {}: {}", i, name, tag_v.key))
        .join("\n");

    let content = MessageBuilder::new()
        .push_line(format!("Tags {}-{} of {}", start, cmp::min(last, tag_count), tag_count))
        .push_codeblock_safe(tag_content, None);

    void!(say(msg.channel_id, content));
});


command!(auto_tags_on(ctx, msg) {
    set_auto_tags(&ctx, msg.guild_id.unwrap().0 as i64, true);
    void!(say(msg.channel_id, "Enabled automatic tags on this guild."));
});


command!(auto_tags_off(ctx, msg) {
    set_auto_tags(&ctx, msg.guild_id.unwrap().0 as i64, false);
    void!(say(msg.channel_id, "Disabled automatic tags on this guild."));
});


pub fn setup_tags(_client: &mut Client, frame: StandardFramework) -> StandardFramework {
    frame.group("Tags",
                |g| g
                .guild_only(true)
                .command(
                    "add_tag", |c| c
                        .cmd(add_tag)
                        .desc("Create a tag with a name and response.")
                        .example("\"something\" This tag's content.")
                        .usage("{tag name} {tag content}")
                )
                .command(
                    "tag", |c| c
                        .cmd(tag)
                        .desc("Retrieve a tag.")
                        .example("\"something\"")
                        .usage("{tag name}")
                )
                .command(
                    "delete_tag", |c| c
                        .cmd(delete_tag)
                        .desc("Delete a tag, only the owner of the tag, or a member with manage message perms can delete tags.")
                        .example("tag name")
                        .usage("{tag name}")
                )
                .command(
                    "list_tags", |c| c
                        .cmd(list_tags)
                        .desc("List tags for this guild.")
                        .example("1 -- lists tags on the first page")
                        .usage("{page}")
                )
                .command(
                    "auto_tags_on", |c| c
                        .cmd(auto_tags_on)
                        .desc(concat!(
                            "By enabling this, you can allow tags to be ",
                            "used by just saying the prefix followed by the tag. ",
                            "For example, #!my_tag."))
                        .required_permissions(Permissions::ADMINISTRATOR)
                )
                .command(
                    "auto_tags_off", |c| c
                        .cmd(auto_tags_off)
                        .desc("Disables the prefix only tagging that is enabled by the command: 'auto_tags_on'")
                        .required_permissions(Permissions::ADMINISTRATOR)
                )
    )
}
