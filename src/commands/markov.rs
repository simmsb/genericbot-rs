use serenity::{
    prelude::*,
    framework::standard::{
        StandardFramework,
        CommandError,
    },
    model::{
        id::{
            ChannelId,
            GuildId,
        },
        channel::Message,
        permissions::Permissions,
    },
    utils::Colour,
};
use utils::{markov, try_resolve_user};
use diesel;
use diesel::prelude::*;
use ::{
    PgConnectionManager,
    ensure_guild,
};
use utils::{HistoryIterator, say, send_message, get_random_members};
use itertools::Itertools;
use typemap::Key;
use lru_cache::LruCache;


struct MarkovStateCache;

impl Key for MarkovStateCache {
    type Value = LruCache<GuildId, bool>;
}


fn get_messages(ctx: &Context, g_id: i64, u_ids: Option<Vec<i64>>) -> Vec<String> {
    use schema::message::dsl::*;
    use diesel::dsl::any;

    let pool = extract_pool!(&ctx);

    no_arg_sql_function!(RANDOM, (), "Represents the pgsql RANDOM() function");


    if let Some(ids) = u_ids {
        message
            .filter(user_id.eq(any(ids)))
            .filter(guild_id.eq(g_id))
            .select(msg)
            .order(RANDOM)
            .limit(7000)
            .load(pool)
            .expect("Error getting messages from DB")
    } else {
        message
            .filter(guild_id.eq(g_id))
            .select(msg)
            .order(RANDOM)
            .limit(7000)
            .load(pool)
            .expect("Error getting messages from DB")
    }
}


fn set_markov(ctx: &Context, g_id: i64, on: bool) {
    use schema::guild::dsl::*;

    let pool = extract_pool!(&ctx);

    diesel::update(guild.find(g_id))
        .set(markov_on.eq(on))
        .execute(pool)
        .unwrap();
}


pub fn check_markov_state(ctx: &Context, g_id: GuildId) -> bool {
    use schema::guild::dsl::*;

    let mut data = ctx.data.lock();

    if let Some(val) = data.get_mut::<MarkovStateCache>().unwrap().get_mut(&g_id) {
        return *val;
    }

    let state = {
        let pool = &*data.get::<PgConnectionManager>().unwrap().get().unwrap();
        guild.find(g_id.0 as i64)
             .select(markov_on)
             .first(pool)
             .unwrap_or_else(|_| {
                 ensure_guild(&ctx, g_id);
                 false
             })
    };

    let cache = data.get_mut::<MarkovStateCache>().unwrap();
    cache.insert(g_id, state);
    return state;
}


fn drop_messages(ctx: &Context, g_id: i64) {
    use schema::message::dsl::*;

    let pool = extract_pool!(&ctx);

    diesel::delete(message.filter(guild_id.eq(g_id))).execute(pool).unwrap();
}


fn clean_crap(ctx: &Context) -> usize {
    use schema::message::dsl::*;
    use diesel::dsl::sql;

    let pool = extract_pool!(&ctx);

    diesel::delete(
        message.filter(sql(r#"
            (char_length(msg) = 0)
            OR (array_length(regexp_split_to_array(msg, E' '), 1) < 4)
            OR (array_length(regexp_split_to_array(msg, E'[^\\w\\s\\d]'), 1) > (char_length(msg) / 2))"#)))
       .execute(pool)
       .expect("Couldn't strip crap.")
}


pub fn message_filter(msg: &Message) -> bool {

    if msg.author.bot {
        return false;
    }

    if !crap_filter(&msg.content) {
        return false;
    }

    return true;
}


fn crap_filter(msg: &str) -> bool {
    // nonzero length
    if msg.len() == 0 {
        return false;
    }

    // atleast half is alphanumeric
    if msg.chars().filter(|&c| c.is_alphanumeric()).count() < (msg.len() / 2) {
            return false;
    }

    // atleast 4 spaces
    if msg.chars().filter(|&c| c.is_whitespace()).count() < 4 {
        return false;
    }

    return true;
}


fn fill_messages(ctx: &Context, c_id: ChannelId, g_id: i64) -> usize {
    use schema::message;
    use models::NewStoredMessage;
    use std::{thread, time};

    let iterator = HistoryIterator::new(c_id).chunks(100);
    let messages = iterator.into_iter().take(40);

    let pool = extract_pool!(&ctx);

    let mut count: usize = 0;

    for chunk in messages {
        // manual sleep here because discord likes to global rl us
        thread::sleep(time::Duration::from_secs(5));

        let messages: Vec<_> = chunk.filter(message_filter).collect();

        count += messages.len();

        let timestamps: Vec<_> = messages
            .iter()
            .map(|m| m.timestamp.naive_utc())
            .collect();
        let new_messages: Vec<_> = messages
            .iter()
            .zip(timestamps.iter())
            .map(|(m, ts)| NewStoredMessage {
                id: m.id.0 as i64,
                guild_id: g_id,
                user_id: m.author.id.0 as i64,
                msg: &m.content,
                created_at: &ts,
            })
            .collect();

        diesel::insert_into(message::table)
            .values(&new_messages)
            .on_conflict_do_nothing()
            .execute(pool)
            .expect("error inserting messages");
    }

    return count;
}


fn average_colours(colours: Vec<Colour>) -> Colour {
    let (s_r, s_g, s_b) = colours.iter().fold((0, 0, 0),
        |(r, g, b), &c| (r + (c.r() as u16).pow(2),
                         g + (c.g() as u16).pow(2),
                         b + (c.b() as u16).pow(2))
    );

    let len = colours.len() as f32;
    let (a_r, a_g, a_b) = (s_r as f32 / len,
                           s_g as f32 / len,
                           s_b as f32 / len);
    let res = (a_r.sqrt() as u8, a_g.sqrt() as u8, a_b.sqrt() as u8);

    Colour::from(res)
}


command!(markov_cmd(ctx, msg, args) {
    use utils::{names_for_members, and_comma_split};

    if !check_markov_state(&ctx, msg.guild_id.unwrap()) {
        void!(say(msg.channel_id, "You don't have markov chains enabled, use the 'markov_enable' command to enable them."));
        return Ok(());
    }

    // All this to just get a random user?
    let members: Vec<_> = args.multiple_quoted::<String>()
        .map(|u| u.into_iter() // resolve members
             .filter_map(|s| try_resolve_user(&s, msg.guild_id.unwrap()).ok())
             .collect::<Vec<_>>()
        )
        .ok()
        .or_else( // this fails us if the vec is empty, so grab a random user
            ||  get_random_members(msg.guild_id.unwrap())
        )
        .ok_or(CommandError::from("Couldn't get any members to markov on"))?;

    let users: Vec<_> = members.iter().map(|m| m.user.read().id).collect();

    let user_names = names_for_members(&users, msg.guild_id.unwrap());
    let user_names_s = and_comma_split(&user_names);

    let user_ids = users.iter().map(|&id| id.0 as i64).collect();
    let messages = get_messages(&ctx, msg.guild_id.unwrap().0 as i64, Some(user_ids));

    let mut chain = markov::MChain::new();

    for msg in &messages {
        chain.add_string(&msg);
    }

    let colours: Vec<_> = members.iter().filter_map(|ref m| m.colour()).collect();

    let col = average_colours(colours);

    for _ in 0..20 { // try 20 times
        if let Some(generated) = chain.generate_string(50, 10) {
            void!(send_message(msg.channel_id,
                |m| m.embed(
                    |e| e
                        .title(format!("A markov chain composed of: {}.", user_names_s))
                        .colour(col)
                        .description(generated)
                    )
            ));
            return Ok(());
        }
    }

    void!(say(msg.channel_id, "Failed to generate a markov."));
});


command!(markov_all(ctx, msg) {
    if !check_markov_state(&ctx, msg.guild_id.unwrap()) {
        void!(say(msg.channel_id, "You don't have markov chains enabled, use the 'markov_enable' command to enable them."));
        return Ok(());
    }

    let messages = get_messages(&ctx, msg.guild_id.unwrap().0 as i64, None);
    let mut chain = markov::MChain::new();

    for msg in &messages {
        chain.add_string(&msg);
    }

    for _ in 0..20 {
        if let Some(generated) = chain.generate_string(50, 4) {
            void!(send_message(msg.channel_id,
                         |m| m.embed(
                             |e| e
                                 .title("A markov chain for the entire guild.")
                                 .description(generated)
                         )
            ));
            return Ok(());
        }
    }

    void!(say(msg.channel_id, "Failed to generate a markov."));
});


command!(markov_enable(ctx, msg) {
    let current_state = check_markov_state(&ctx, msg.guild_id.unwrap());

    if current_state {
        void!("Markov chains are already enabled here.");
    } else {
        set_markov(&ctx, msg.guild_id.unwrap().0 as i64, true);
        void!(say(msg.channel_id, "Enabled markov chains for this guild, now filling messages..."));
        let count = fill_messages(&ctx, msg.channel_id, msg.guild_id.unwrap().0 as i64);
        void!(say(msg.channel_id, format!("Build the markov chain with {} messages", count)));
    }
});


command!(markov_disable(ctx, msg) {
    let current_state = check_markov_state(&ctx, msg.guild_id.unwrap());

    if !current_state {
        void!("Markov chains are already disabled here.");
    } else {
        set_markov(&ctx, msg.guild_id.unwrap().0 as i64, false);
        drop_messages(&ctx, msg.guild_id.unwrap().0 as i64);
        void!(say(msg.channel_id, "Disabled markov chains and dropped messages for this guild."));
    }
});


command!(fill_markov(ctx, msg) {
    void!(say(msg.channel_id, "Adding messages to the chain."));
    let count = fill_messages(&ctx, msg.channel_id, msg.guild_id.unwrap().0 as i64);
    void!(say(msg.channel_id, format!("Inserted {} messages into the chain.", count)));
});


command!(strip_crap(ctx, msg) {
    void!(say(msg.channel_id, "Beginning to clean messages."));
    let num_deleted = clean_crap(&ctx);
    void!(say(msg.channel_id, format!("Deleted {} messages.", num_deleted)));
});


pub fn setup_markov(client: &mut Client, frame: StandardFramework) -> StandardFramework {
    {
        let mut data = client.data.lock();
        data.insert::<MarkovStateCache>(LruCache::new(1000));
    }

    frame
        .simple_bucket("markov_fill_bucket", 60 * 60) // once each hour
        .group("Markov",
               |g| g
               .guild_only(true)
               .command("markov", |c| c
                        .cmd(markov_cmd)
                        .desc("Generate a markov chain for some users, if not users given: pick a random user")
                        .example("a_username @a_mention")
                        .usage("{users...}")
               )
               .command("markov_all", |c| c
                        .cmd(markov_all)
                        .desc("Generate a markov chain for all users in a guild")
               )
               .command("markov_enable", |c| c
                        .cmd(markov_enable)
                        .desc("Enable usage of markov chains for this guild.")
                        .required_permissions(Permissions::ADMINISTRATOR)
               )
               .command("markov_disable", |c| c
                        .cmd(markov_disable)
                        .desc("Disable usage of markov chains for this guild.\n This also drops all messages from the chain.")
                        .required_permissions(Permissions::ADMINISTRATOR)
               )
               .command("fill_markov", |c| c
                        .cmd(fill_markov)
                        .desc("Add messages to the markov chain.")
                        .required_permissions(Permissions::ADMINISTRATOR)
                        .bucket("markov_fill_bucket")
               )
               .command("strip_crap", |c| c
                        .cmd(strip_crap)
                        .desc("Strip crap from the db.")
                        .owners_only(true)
                        .help_available(false))
    )
}
