#![feature(vec_remove_item)]

pub mod schema;
pub mod models;
#[macro_use]
pub mod utils;
pub mod background_tasks;

mod commands;

#[macro_use]
extern crate serenity;
extern crate dotenv;
#[macro_use]
extern crate diesel;
extern crate r2d2;
extern crate r2d2_diesel;
extern crate chrono;
extern crate typemap;
extern crate base64;
extern crate hyper;
extern crate hyper_native_tls;
extern crate regex;

use serenity::{
    CACHE,
    prelude::*,
    model::{
        guild::Guild,
        channel::Message,
        gateway::Ready,
    },
    client::bridge::gateway::{ShardManager},
    framework::standard::StandardFramework
};

use diesel::{
    prelude::*,
    pg::PgConnection,
};
use r2d2_diesel::ConnectionManager;

use std::sync::Arc;
use typemap::Key;

struct Handler;

impl EventHandler for Handler {
    fn ready(&self, _: Context, ready: Ready) {
        use background_tasks;

        if let Some(shard) = ready.shard {
            println!("Connected as: {} on shard {} of {}", ready.user.name, shard[0], shard[1]);
        }

        background_tasks::background_task();
    }

    fn guild_create(&self, ctx: Context, guild: Guild, _: bool) {
        use schema::{guild, prefix};
        use models::{Guild, NewPrefix};

        let pool = extract_pool!(&ctx);

        let new_guild = Guild {
            id: guild.id.0 as i64,
            markov_on: false,
            tag_prefix_on: false,
            commands_from: 0,
        };

        let default_prefix = NewPrefix {
            guild_id: guild.id.0 as i64,
            pre: "#!",
        };

        diesel::insert_into(guild::table)
            .values(&new_guild)
            .on_conflict_do_nothing()
            .execute(pool)
            .expect("Couldn't create guild");

        diesel::insert_into(prefix::table)
            .values(&default_prefix)
            .on_conflict_do_nothing()
            .execute(pool)
            .expect("Couldn't create default prefix");
    }
}


struct ShardManagerContainer;

impl Key for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

struct PgConnectionManager;

impl Key for PgConnectionManager {
    type Value = r2d2::Pool<ConnectionManager<PgConnection>>;
}


fn get_prefixes(ctx: &mut Context, m: &Message) -> Option<Arc<Vec<String>>> {
    use schema::prefix::dsl::*;

    let pool = extract_pool!(&ctx);

    if let Some(g_id) = m.guild_id() {
        let prefixes = prefix
            .filter(guild_id.eq(g_id.0 as i64))
            .select(pre)
            .load::<String>(pool)
            .expect("Error loading prefixes");
        Some(Arc::new(prefixes))
    } else {
        None
    }
}

// Our setup stuff
fn setup(_client: &mut Client, frame: StandardFramework) -> StandardFramework {
    use serenity::framework::standard::{
        DispatchError::*,
        help_commands,
        HelpBehaviour,
    };

    use std::collections::HashSet;

    let owners = match serenity::http::get_current_application_info() {
        Ok(info) => {
            let mut set = HashSet::new();
            set.insert(info.owner.id);
            set
        },
        Err(why) => panic!("Couldn't retrieve app info: {:?}", why),
    };

    frame
        .on_dispatch_error(| _, msg, err | {
            println!("handling error: {:?}", err);
            let s = match err {
                OnlyForGuilds =>
                    "This command can only be used in private messages.".to_string(),
                RateLimited(time) =>
                    format!("You are ratelimited, try again in: {} seconds.", time),
                CheckFailed =>
                    "The check for this command failed.".to_string(),
                LackOfPermissions(perms) =>
                    format!("This command requires permissions: {:?}", perms),
                _ => return,
            };
            let _ = msg.channel_id.say(&s);
        })
         .after(| ctx, msg, _, err | {
             use schema::guild::dsl::*;

             match err {
                 Ok(_) => {
                     if let Some(g_id) = msg.guild_id() {
                         let pool = extract_pool!(&ctx);

                         diesel::update(guild.find(g_id.0 as i64))
                             .set(commands_from.eq(commands_from + 1))
                             .execute(pool)
                             .unwrap();
                     }
                 }
                 Err(e) => { let _ = msg.channel_id.say(e.0); },
             }
         })
        .configure(|c| c
                   .dynamic_prefixes(get_prefixes)
                   .prefix("--")
                   .owners(owners))
        .customised_help(help_commands::plain, |c| c
                         .individual_command_tip(
                             "To get help on a specific command, pass the command name as an argument to help.")
                         .command_not_found_text("A command with the name {} does not exist.")
                         .suggestion_text("No command with the name '{}' was found.")
                         .lacking_permissions(HelpBehaviour::Hide))
        .unrecognised_command(|ctx, msg, cmd_name| {
            use schema::guild::dsl::*;
            use schema::tag::dsl::*;

            let pool = extract_pool!(&ctx);

            let g_id = match msg.guild_id() {
                Some(x) => x.0 as i64,
                None    => return,
            };

            let has_auto_tags = guild
                .find(&g_id)
                .select(tag_prefix_on)
                .first(pool)
                .unwrap_or(false);

            if has_auto_tags {
                if let Ok(r_tag) = tag
                    .filter(guild_id.eq(&g_id))
                    .filter(key.eq(cmd_name))
                    .select(text)
                    .first::<String>(pool) {
                        let _ = msg.channel_id.say(r_tag);
                }
            }

        })
}


pub fn log_message(msg: &String) {
    use serenity::model::channel::Channel::Guild;

    let chan_id = dotenv::var("DISCORD_BOT_LOG_CHAN").unwrap().parse::<u64>().unwrap();
    if let Some(Guild(chan)) = CACHE.read().channel(chan_id) {
        chan.read().say(msg).unwrap();
    }
}


fn main() {
    let token = dotenv::var("DISCORD_BOT_TOKEN").unwrap();
    let db_url = dotenv::var("DISCORD_BOT_DB").unwrap();

    let manager = ConnectionManager::<PgConnection>::new(db_url);
    let pool = r2d2::Pool::builder().build(manager).unwrap();

    let mut client = Client::new(&token, Handler).unwrap();

    let setup_fns = &[setup,
                      commands::tags::setup_tags,
                      commands::admin::setup_admin,
                     ];

    let framework = setup_fns.iter().fold(
        StandardFramework::new(),
        | acc, fun | fun(&mut client, acc));

    client.with_framework(framework);

    {
        let mut data = client.data.lock();
        data.insert::<ShardManagerContainer>(Arc::clone(&client.shard_manager));
        data.insert::<PgConnectionManager>(pool);
    }


    if let Err(why) = client.start_autosharded() {
        println!("AAA: {:?}", why);
    }
}
