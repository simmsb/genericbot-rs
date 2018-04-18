use serenity::{
    prelude::*,
    framework::standard::{
        StandardFramework,
        CommandError,
    },
    model::{
        id::ChannelId,
        permissions::Permissions,
    },
};
use utils::{markov, try_resolve_user};
use diesel;
use diesel::prelude::*;
use ::PgConnectionManager;
use utils::HistoryIterator;
use itertools::Itertools;
use rand::Rng;
use rand;


fn get_messages(ctx: &Context, g_id: i64, u_ids: Vec<i64>) -> Vec<String> {
    use schema::message::dsl::*;
    use diesel::dsl::any;

    let pool = extract_pool!(&ctx);

    no_arg_sql_function!(RANDOM, (), "Represents the pgsql RANDOM() function");

    message
        .filter(user_id.eq(any(u_ids)))
        .filter(guild_id.eq(g_id))
        .select(msg)
        .order(RANDOM)
        .limit(1000)
        .load(pool)
        .expect("Error getting messages from DB")
}


fn set_markov(ctx: &Context, g_id: i64, on: bool) {
    use schema::guild::dsl::*;

    let pool = extract_pool!(&ctx);

    diesel::update(guild.find(g_id))
        .set(markov_on.eq(on))
        .execute(pool)
        .unwrap();
}


pub fn check_markov_state(ctx: &Context, g_id: i64) -> bool {
    use schema::guild::dsl::*;

    let pool = extract_pool!(&ctx);

    guild.find(g_id)
        .select(markov_on)
        .first(pool)
        .unwrap() // TODO: if fail, go and generate the guild
}


fn drop_messages(ctx: &Context, g_id: i64) {
    use schema::message::dsl::*;

    let pool = extract_pool!(&ctx);

    diesel::delete(message.filter(guild_id.eq(g_id))).execute(pool).unwrap();
}


fn fill_messages(ctx: &Context, c_id: ChannelId, g_id: i64) -> usize {
    use schema::message;
    use models::NewStoredMessage;

    let iterator = HistoryIterator::new(c_id).chunks(100);
    let messages = iterator.into_iter().take(40);

    let pool = extract_pool!(&ctx);

    let mut count: usize = 0;

    for chunk in messages {
        let messages: Vec<_> = chunk
            .filter(|m| m.content.len() >= 40)
            .collect();

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


command!(markov_cmd(ctx, msg, args) {
    use utils::{names_for_members, and_comma_split};

    if !check_markov_state(&ctx, msg.guild_id().unwrap().0 as i64) {
        void!(msg.channel_id.say("You don't have markov chains enabled, use the 'markov_enable' command to enable them."));
        return Ok(());
    }

    // All this to just get a random user?
    let users: Vec<_> = args.multiple_quoted::<String>()
        .map(|u| u.into_iter() // resolve users
             .filter_map(|s| try_resolve_user(&s, msg.guild_id().unwrap()).ok())
             .collect::<Vec<_>>()
        )
        .ok()
        .or_else( // this fails us if the vec is empty, so grab a random user
            || {
                // println!("finding users");
                msg.guild_id().unwrap().find().and_then(|g| {
                    let guild = g.read();
                    let member_ids: Vec<_> = guild.members.keys().collect();
                    rand::thread_rng()
                        .choose(&member_ids)
                        .map(|&&m_id| vec![m_id])
                })
            }
        )
        .ok_or(CommandError::from("Couldn't get any users to markov on"))?;


    let user_names = names_for_members(&users, msg.guild_id().unwrap());
    let user_names_s = and_comma_split(&user_names);

    let user_ids = users.iter().map(|&id| id.0 as i64).collect();

    // println!("user id's: {:?}", &user_ids);

    let messages = get_messages(&ctx, msg.guild_id().unwrap().0 as i64, user_ids);

    let mut chain = markov::MChain::new();

    for msg in messages.iter() {
        chain.add_string(&msg);
    }

    for _ in 0..10 { // try 10 times
        if let Some(generated) = chain.generate_string(40) {
            msg.channel_id.send_message(
                |m| m.embed(
                    |e| e
                        .title(format!("A markov chain composed of: {}.", user_names_s))
                        .description(generated)
                    )
            )?;
            return Ok(());
        }
    }

    void!(msg.channel_id.say("Failed to generate a markov."));
});


command!(markov_enable(ctx, msg) {
    set_markov(&ctx, msg.guild_id().unwrap().0 as i64, true);
    void!(msg.channel_id.say("Enabled markov chains for this guild, now filling messages..."));
    let count = fill_messages(&ctx, msg.channel_id, msg.guild_id().unwrap().0 as i64);
    void!(msg.channel_id.say(format!("Build the markov chain with {} messages", count)));
});


command!(markov_disable(ctx, msg) {
    set_markov(&ctx, msg.guild_id().unwrap().0 as i64, false);
    drop_messages(&ctx, msg.guild_id().unwrap().0 as i64);
    void!(msg.channel_id.say("Disabled markov chains for this guild."));
});


command!(fill_markov(ctx, msg) {
    let count = fill_messages(&ctx, msg.channel_id, msg.guild_id().unwrap().0 as i64);
    void!(msg.channel_id.say(format!("Inserted {} messages into the chain.", count)));
});


pub fn setup_markov(_client: &mut Client, frame: StandardFramework) -> StandardFramework {
    frame.group("Markov",
                |g| g
                .guild_only(true)
                .command("markov", |c| c
                         .cmd(markov_cmd)
                         .desc("Generate a markov chain for some users, if not users given: pick a random user")
                         .example("a_username @a_mention")
                         .usage("{users...}")
                )
                .command("markov_enable", |c| c
                         .cmd(markov_enable)
                         .desc("Enable usage of markov chains for this guild.")
                         .required_permissions(Permissions::ADMINISTRATOR)
                )
                .command("markov_disable", |c| c
                         .cmd(markov_disable)
                         .desc("Disable usage of markov chains for this guild.")
                         .required_permissions(Permissions::ADMINISTRATOR)
                )
                .command("fill_markov", |c| c
                         .cmd(fill_markov)
                         .desc("Add messages to the markov chain.")
                         .required_permissions(Permissions::ADMINISTRATOR)
                )
    )
}
