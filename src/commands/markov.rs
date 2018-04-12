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
use utils::markov;
use diesel;
use diesel::prelude::*;
use ::PgConnectionManager;


fn get_messages(ctx: &Context, g_id: i64, u_ids: Vec<i64>) -> Vec<String> {
    use schema::message::dsl::*;
    use diesel::dsl::any;

    let pool = extract_pool!(&ctx);

    message
        .filter(user_id.eq(any(u_ids)))
        .filter(guild_id.eq(g_id))
        .select(msg)
        .limit(1000)
        .load(pool)
        .expect("Error getting messages from DB")
}


command!(markov_cmd(ctx, msg, args) {
    use serenity::model::id::UserId;
    use utils::{names_for_members, and_comma_split};

    let users: Vec<_> = args.multiple_quoted::<UserId>()
        .unwrap_or(vec![]);

    let user_names = names_for_members(&users, msg.guild_id().unwrap());
    let user_names_s = and_comma_split(&user_names);

    let user_ids = users.iter().map(|&id| id.0 as i64).collect();
    let messages = get_messages(&ctx, msg.guild_id().unwrap().0 as i64, user_ids);

    let mut chain = markov::MChain::new();

    for msg in messages.iter() {
        chain.add_string(&msg);
    }

    for _ in 0..10 {
        if let Some(generated) = chain.generate_string(40) {

            msg.channel_id.send_message(
                |m| m.embed(
                    |e| e
                        .title(format!("A markov chain composed of: {}", user_names_s))
                        .description(generated)
                    )
            )?;
            return Ok(());
        }
    }

    msg.channel_id.say("Failed to generate a markov.")?;
});


pub fn setup_markov(_client: &mut Client, frame: StandardFramework) -> StandardFramework {
    frame.group("Markov",
                |g| g
                .command("markov", |c| c
                         .cmd(markov_cmd)
                         .desc("Generate a markov chain for some users")
                         .example("a_username @a_mention")
                         .usage("{users...}")
                )
    )
}
