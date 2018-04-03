use serenity::{
    prelude::*,
    model::channel::Message,
    framework::standard::{
        StandardFramework,
        CommandError,
    },
};
use serenity;
use diesel::prelude::*;
use diesel;
use ::PgConnectionManager;
use models::Tag;

#[macro_use]
use utils::macros::*;

fn get_tag(ctx: &Context, g_id: i64, tag_key: &String) -> QueryResult<Tag> {
    use schema::tag::dsl::*;

    let data = ctx.data.lock();
    let pool = &*data.get::<PgConnectionManager>().unwrap().get().unwrap();
    drop(data);

    tag.filter(guild_id.eq(&g_id))
       .filter(key.eq(tag_key))
       .first(pool)
}


fn insert_tag(ctx: &Context, msg: &Message, key: &String, content: &String) -> Tag {
    use schema::tag;
    use models::NewTag;

    let new_tag =  NewTag {
        author_id: msg.author.id.0 as i64,
        guild_id: msg.guild_id().unwrap().0 as i64,
        key: key,
        text: content,
    };

    let data = ctx.data.lock();
    let pool = &*data.get::<PgConnectionManager>().unwrap().get().unwrap();
    drop(data);

    diesel::insert_into(tag::table)
        .values(&new_tag)
        .get_result(pool)
        .expect("Couldn't save posts")
}


fn delete_tag_do(ctx: &Context, tag_id: i64) {
    use schema::tag::dsl::*;

    let data = ctx.data.lock();
    let pool = &*data.get::<PgConnectionManager>().unwrap().get().unwrap();
    drop(data);

    diesel::delete(tag.filter(id.eq(tag_id))).execute(pool).unwrap();
}


fn get_tags_range(ctx: &Context, g_id: i64, page: i64) -> QueryResult<Vec<Tag>> {
    use schema::tag::dsl::*;

    let data = ctx.data.lock();
    let pool = &*data.get::<PgConnectionManager>().unwrap().get().unwrap();
    drop(data);

    let start = page * 20;
    tag.filter(guild_id.eq(&g_id))
       .order(key.asc())
       .offset(start)
       .limit(20)
       .load(pool)
}


fn get_tag_count(ctx: &Context, g_id: i64) -> QueryResult<i64> {
    use schema::tag::dsl::*;

    let data = ctx.data.lock();
    let pool = &*data.get::<PgConnectionManager>().unwrap().get().unwrap();
    drop(data);

    tag.filter(guild_id.eq(&g_id))
       .count()
       .get_result(pool)
}


command!(add_tag(ctx, msg, args) {
    let key = get_arg!(args, single_quoted, String, key);
    let value = get_arg!(args, multiple, String, key).join(" ");

    if let Ok(t) = get_tag(&ctx, msg.guild_id().unwrap().0 as i64, &key) {
        msg.channel_id.say(format!("The tag: {} already exists", t.key))?;
    } else if key.len() >= 50 {
        msg.channel_id.say("Tag keys cannot be longer than 50 characters.")?;
    } else {
        insert_tag(&ctx, &msg, &key, &value);
        msg.channel_id.say(format!("Created tag: {} with content: {}!", key, value))?;
    }
});


command!(tag(ctx, msg, args) {
    let key = get_arg!(args, multiple, String, key).join(" ");

    if let Ok(t) = get_tag(&ctx, msg.guild_id().unwrap().0 as i64, &key) {
        msg.channel_id.say(t.text)?;
    } else {
        msg.channel_id.say("This tag does not exist.")?;
    }
});


command!(delete_tag(ctx, msg, args) {
    use serenity::CACHE;

    let key = get_arg!(args, multiple, String, key).join(" ");

    if let Ok(t) = get_tag(&ctx, msg.guild_id().unwrap().0 as i64, &key) {

        let cache = CACHE.read();
        let has_manage_messages = {
            // TODO: refactor, use guild.member_permissions instead of getting the member
            if let Some(guild) = cache.guild(msg.guild_id().unwrap()) {
                if let Some(member) = guild.read().members.get(&msg.author.id) {
                    member.permissions().ok().map_or(false, |p| p.manage_messages())
                } else {  // AAAAAAAA
                    false
                }
            } else {
                false
            }
        };

        if has_manage_messages || (t.author_id as u64 == msg.author.id.0) {
            delete_tag_do(&ctx, t.id);
            msg.channel_id.say(format!("Deleted tag of name: {}.", t.key))?;
        } else {
            msg.channel_id.say(format!("You are not the owner of this tag or do not have manage messages."))?;
        }
    } else {
        msg.channel_id.say("That tag does not exist.")?;
    }
});


command!(list_tags(ctx, msg, args) {
    use serenity::utils::{with_cache, MessageBuilder};
    use std::cmp;

    let page = args.single::<i64>().unwrap_or(1) - 1;

    if page < 0 {
        msg.channel_id.say(format!("That page does not exist."))?;
        return Ok(());
    }

    let start = page * 20;
    let last = (page + 1) * 20 - 1;
    let tag_list: Vec<Tag> = get_tags_range(&ctx, msg.guild_id().unwrap().0 as i64, page as i64)?;
    let tag_count = get_tag_count(&ctx, msg.guild_id().unwrap().0 as i64)?;

    if start > tag_count {
        msg.channel_id.say(format!("The requested page ({}) is greater than the number of pages ({}).", page, last / 20))?;
        return Ok(());
    }

    let user_ids: Vec<i64> = tag_list.iter().map(|t| t.author_id).collect();

    let user_names: Vec<String> = match with_cache( // AAAAAAAAAAAAAAAAa
        |cache| cache.guild(msg.guild_id().unwrap()).map(|g| {
            let members = &g.read().members;
            user_ids.iter().map(
                |&id| members.get(&From::from(id as u64)).map_or_else(
                    || id.to_string(),
                    |m| m.display_name().to_string()))
                .collect()
        })) {
        Some(x) => x,
        None    => user_ids.iter().map(|&id| id.to_string()).collect(),
    };


    let tag_content: String = user_names
        .into_iter()
        .zip(tag_list)
        .enumerate().map(
            |(i, (name, tag_v))|
            format!("{:>3} | {}: {:.50}", i, name, tag_v.key)
        ).collect::<Vec<String>>().join("\n");


    let content = MessageBuilder::new()
        .push_line(format!("Tags {}-{} of {}", start, cmp::min(last, tag_count), tag_count))
        .push_codeblock_safe(tag_content, None)
        .build();

    msg.channel_id.say(content)?;
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
                        .num_args(2)
                )
                .command(
                    "tag", |c| c
                        .cmd(tag)
                        .desc("Retrieve a tag.")
                        .example("\"something\"")
                        .usage("{tag name}")
                        .num_args(1)
                )
                .command(
                    "delete_tag", |c| c
                        .cmd(delete_tag)
                        .desc("Delete a tag, only the owner of the tag, or a member with manage message perms can delete tags.")
                        .example("tag name")
                        .usage("{tag name}")
                        .num_args(1)
                )
                .command(
                    "list_tags", |c| c
                        .cmd(list_tags)
                        .desc("List tags for this guild.")
                        .example("1 -- lists tags on the first page")
                        .usage("{page}")
                        .max_args(1)
                )
    )
}
