use serenity::{
    prelude::*,
    model::channel::Message,
    framework::standard::StandardFramework,
};
use diesel::prelude::*;
use diesel;
use ::PgConnectionManager;
use models::Tag;

fn get_tag(ctx: &Context, g_id: i64, key: &String) -> Option<Tag> {
    use schema::tag::dsl::*;

    let data = ctx.data.lock();
    let pool = &*data.get::<PgConnectionManager>().unwrap().get().unwrap();
    drop(data);

    tag.filter(guild_id.eq(&g_id))
       .filter(text.eq(key))
       .first(pool).ok()
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


command!(add_tag(ctx, msg, args) {
    let key = args.single_quoted::<String>().unwrap();
    let value = args.multiple::<String>().unwrap().join(" ");

    if let Some(t) = get_tag(&ctx, msg.guild_id().unwrap().0 as i64, &key) {
        msg.channel_id.say(format!("The tag: {} already exists", t.key))?;
    } else if key.len() >= 50 {
        msg.channel_id.say("Tag keys cannot be longer than 50 characters.")?;
    } else {
        insert_tag(&ctx, &msg, &key, &value);
        msg.channel_id.say(format!("Created tag: {} with content: {}!", key, value))?;
    }
});


pub fn setup_tags(client: &mut Client, frame: StandardFramework) -> StandardFramework {
    frame.group("Tags",
                |g| g
                .guild_only(true)
                .command(
                    "add_tag", |c| c
                        .cmd(add_tag)
                        .desc("Create a tag with a name and response.")))
    }
