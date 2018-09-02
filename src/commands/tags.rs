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
use utils::{
    say,
    pagination::{
        PaginationResult,
        Paginate,
    },
};


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


fn list_tags(ctx: &Context, g_id: i64, page: i64) -> PaginationResult<Tag> {
    use schema::tag::dsl::*;

    let pool = extract_pool!(&ctx);

    tag.filter(guild_id.eq(&g_id))
       .order(key.asc())
       .paginate(page)
       .load_and_count_pages(pool)
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


command!(add_tag_cmd(ctx, msg, args) {
    let key = get_arg!(args, single_quoted, String, key);
    let value = args.rest().trim();

    if let Ok(t) = get_tag(&ctx, msg.guild_id.unwrap().0 as i64, &key) {
        void!(say(msg.channel_id, format!("The tag: {} already exists", t.key)));
    } else if key.len() >= 50 {
        void!(say(msg.channel_id, "Tag keys cannot be longer than 50 characters."));
    } else {
        insert_tag(&ctx, &msg, &key, &value);
        void!(say(msg.channel_id, format!("Created tag: {} with content: {}!", key, value)));
    }
});


command!(tag_cmd(ctx, msg, args) {
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

        let has_manage_messages = log_time!(with_cache(
            |cache| cache.guild(msg.guild_id.unwrap()).map_or(
                false,
                |g| g.read().member_permissions(msg.author.id).manage_messages()
            )
        ), "with_cache: has_manage_messages");

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


command!(list_tags_cmd(ctx, msg, args) {
    use utils::names_for_members;

    let page = args.single::<i64>().unwrap_or(1);

    if page <= 0 {
        return Err("That page does not exist.".into());
    }

    let tags = list_tags(&ctx, msg.guild_id.unwrap().0 as i64, page);

    if !tags.page_exists() {
        return Err("That page does not exist or no tags exist for this server.".into());
    }

    let user_ids: Vec<u64> = tags.results.iter().map(|t| t.author_id as u64).collect();

    let user_names = names_for_members(&user_ids, msg.guild_id.unwrap());

    let tag_content = user_names
        .into_iter()
        .zip(tags.iter_with_indexes())
        .map(|(name, (tag_v, i))| format!("{:>3} | {}: {}", i, name, tag_v.key))
        .join("\n");

    let content = MessageBuilder::new()
        .push_codeblock_safe(tag_content, None)
        .push_line(format!("Page {} of {}", tags.page, tags.total_pages));

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
                        .cmd(add_tag_cmd)
                        .desc("Create a tag with a name and response.")
                        .example("\"something\" This tag's content.")
                        .usage("{tag name} {tag content}")
                )
                .command(
                    "tag", |c| c
                        .cmd(tag_cmd)
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
                        .cmd(list_tags_cmd)
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
