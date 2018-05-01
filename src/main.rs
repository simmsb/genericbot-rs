#![feature(vec_remove_item)]

pub mod schema;
pub mod models;
#[macro_use] pub mod utils;
pub mod background_tasks;

mod commands;

#[macro_use] extern crate serenity;
#[macro_use] extern crate diesel;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;
#[macro_use] extern crate serde_json;
extern crate serde;
extern crate dotenv;
extern crate r2d2;
extern crate r2d2_diesel;
extern crate chrono;
extern crate typemap;
extern crate base64;
extern crate regex;
extern crate itertools;
extern crate rand;
extern crate procinfo;
extern crate systemstat;
extern crate whirlpool;
extern crate reqwest;
extern crate lru_cache;
extern crate threadpool;
extern crate simplelog;
extern crate rmp_serde;

use serenity::{
    prelude::*,
    model::{
        guild::Guild,
        id::GuildId,
        channel::Message,
        gateway::{Ready, Game},
    },
    client::bridge::gateway::{ShardManager},
    framework::standard::StandardFramework,
    utils::with_cache,
};

use diesel::{
    prelude::*,
    pg::PgConnection,
};
use r2d2_diesel::ConnectionManager;

use std::sync::Arc;
use std::os::unix::net::UnixStream;
use typemap::Key;
use lru_cache::LruCache;
use threadpool::ThreadPool;
use log::{LevelFilter, Metadata, Record, Log};
use simplelog::*;
use utils::say;

struct Handler;

impl EventHandler for Handler {
    fn ready(&self, ctx: Context, ready: Ready) {
        use background_tasks;

        if let Some(shard) = ready.shard {
            info!(target: "bot", "Connected as: {} on shard {} of {}", ready.user.name, shard[0], shard[1]);

            ctx.set_game(Game::playing(&format!("Little generic bot | generic#help | Shard {}", shard[0])));
        }

        background_tasks::background_task(&ctx);

        utils::insert_missing_guilds(&ctx);
    }

    fn message(&self, ctx: Context, msg: Message) {
        use schema::message;
        use models::NewStoredMessage;

        if !commands::markov::message_filter(&msg) {
            return;
        }

        let g_id = match msg.guild_id() {
            Some(id) => id,
            None     => return,
        };

        if msg.content.len() < 40 {
            return;
        }

        if !commands::markov::check_markov_state(&ctx, g_id) {
            return;
        }

        let pool = extract_pool!(&ctx);

        let to_insert = NewStoredMessage {
            id: msg.id.0 as i64,
            guild_id: g_id.0 as i64,
            user_id: msg.author.id.0 as i64,
            msg: &msg.content,
            created_at: &msg.timestamp.naive_utc(),
        };

        diesel::insert_into(message::table)
            .values(&to_insert)
            .execute(pool)
            .expect("Couldn't insert message.");
    }

    fn guild_create(&self, ctx: Context, guild: Guild, _new: bool) {
        // use schema::{guild, prefix};
        use schema;
        use diesel::dsl::exists;

        let pool = extract_pool!(&ctx);

        let guild_known: bool = diesel::select(exists(
            schema::guild::table
                .find(guild.id.0 as i64)))
            .get_result(pool)
            .expect("Failed to check guild existence");

        if guild_known {
            return;
        }

        info!(target: "bot", "Joined guild: {}", guild.name);

        ensure_guild(&ctx, guild.id);
    }
}


fn ensure_guild(ctx: &Context, g_id: GuildId) {
    use schema;
    use models::{NewGuild, NewPrefix};

    let pool = extract_pool!(&ctx);

    let new_guild = NewGuild {
        id: g_id.0 as i64,
    };

    let default_prefix = NewPrefix {
        guild_id: g_id.0 as i64,
        pre: "#!",
    };

    diesel::insert_into(schema::guild::table)
        .values(&new_guild)
        .on_conflict_do_nothing()
        .execute(pool)
        .expect("Couldn't create guild");

    diesel::insert_into(schema::prefix::table)
        .values(&default_prefix)
        .on_conflict_do_nothing()
        .execute(pool)
        .expect("Couldn't create default prefix");
}


struct ShardManagerContainer;

impl Key for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

struct PgConnectionManager;

impl Key for PgConnectionManager {
    type Value = r2d2::Pool<ConnectionManager<PgConnection>>;
}

struct StartTime;

impl Key for StartTime {
    type Value = chrono::NaiveDateTime;
}

struct CmdCounter;

impl Key for CmdCounter {
    type Value = Arc<RwLock<usize>>;
}

struct OwnerId;

impl Key for OwnerId {
    type Value = serenity::model::user::User;
}

struct PrefixCache;

impl Key for PrefixCache {
    // Jesus christ
    type Value = LruCache<GuildId, Arc<RwLock<Vec<String>>>>;
}

struct ThreadPoolCache;

impl Key for ThreadPoolCache {
    type Value = Arc<Mutex<ThreadPool>>;
}


lazy_static! {
    pub static ref MESSENGER_SOCKET: Arc<Mutex<UnixStream>> = {
        let messenger_socket = dotenv::var("DISCORD_BOT_MESSENGER_SOCKET").unwrap();
        let message_socket = UnixStream::connect(messenger_socket).unwrap();
        message_socket.shutdown(std::net::Shutdown::Read).unwrap();
        Arc::new(Mutex::new(message_socket))
    };
}


fn get_prefixes(ctx: &mut Context, m: &Message) -> Option<Arc<RwLock<Vec<String>>>> {
    use schema::prefix::dsl::*;

    if let Some(g_id) = m.guild_id() {

        let mut data = ctx.data.lock();
        {
            let mut cache = data.get_mut::<PrefixCache>().unwrap();
            if let Some(val) = cache.get_mut(&g_id) {
                return Some(val.clone());
            }
        }

        let mut prefixes = {
            let pool  = &*data.get::<PgConnectionManager>().unwrap().get().unwrap();
            prefix
                .filter(guild_id.eq(g_id.0 as i64))
                .select(pre)
                .load::<String>(pool)
                .expect("Error loading prefixes")
        };

        prefixes.push("generic#".to_owned());

        {
            let mut cache = data.get_mut::<PrefixCache>().unwrap();
            let prefixes = Arc::new(RwLock::new(prefixes));
            cache.insert(g_id, prefixes.clone());
            Some(prefixes.clone())
        }
    } else {
        None
    }
}

// Our setup stuff
fn setup(client: &mut Client, frame: StandardFramework) -> StandardFramework {
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
            {
                let mut data = client.data.lock();
                data.insert::<OwnerId>(info.owner);
            }
            set
        },
        Err(why) => panic!("Couldn't retrieve app info: {:?}", why),
    };

    frame
        .on_dispatch_error(| _ctx, msg, err | {
            debug!(target: "bot", "handling error: {:?}", err);
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
            void!(say(msg.channel_id, &s));
        })
         .after(| ctx, msg, _, err | {
             use schema::guild::dsl::*;

             match err {
                 Ok(_) => {
                     {
                         let lock = ctx.data.lock(); ;
                         let mut count = lock.get::<CmdCounter>().unwrap().write();
                         *count += 1;
                     }

                     if let Some(g_id) = msg.guild_id() {
                         let pool = extract_pool!(&ctx);

                         diesel::update(guild.find(g_id.0 as i64))
                             .set(commands_from.eq(commands_from + 1))
                             .execute(pool)
                             .unwrap();
                     }
                 }
                 Err(e) => void!(say(msg.channel_id, e.0)),
             }
         })
        .configure(|c| c
                   .allow_whitespace(true)
                   .dynamic_prefixes(get_prefixes)
                   .prefix("generic#")
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

            let g_id = match msg.guild_id() {
                Some(x) => x.0 as i64,
                None    => return,
            };

            let pool = extract_pool!(&ctx);

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


pub fn log_message(msg: &str) {
    let chan_id = dotenv::var("DISCORD_BOT_LOG_CHAN").unwrap().parse::<u64>().unwrap();

    with_cache(|c| {
        if let Some(chan) = c.guild_channel(chan_id) {
            void!(chan.read().say(msg));
        }
    });
}


struct DiscordLogger {
    level: LevelFilter,
    config: Config,
    filter_target: String,
}


impl DiscordLogger {
    // fn init(log_level: LevelFilter, config: Config) -> Result<(), SetLoggerError> {
    //     set_max_level(log_level.clone());
    //     set_boxed_logger(DiscordLogger::new(log_level, config))
    // }

    fn new(log_level: LevelFilter, config: Config, filter_target: &str) -> Box<DiscordLogger> {
        Box::new(DiscordLogger { level: log_level, config: config, filter_target: filter_target.to_owned() })
    }
}


impl Log for DiscordLogger {

    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        if self.filter_target != record.target() {
            return;
        }

        log_message(&format!("LOG: {}:{} -- {}", record.level(), record.target(), record.args()));
    }

    fn flush(&self) {
    }
}


impl SharedLogger for DiscordLogger {
    fn level(&self) -> LevelFilter {
        self.level
    }

    fn config(&self) -> Option<&Config>
    {
        Some(&self.config)
    }

    fn as_log(self: Box<Self>) -> Box<Log> {
        Box::new(*self)
    }
}


fn main() {
    CombinedLogger::init(
        vec![
            SimpleLogger::new(LevelFilter::Debug, Config::default()),
            DiscordLogger::new(LevelFilter::Info, Config::default(), "bot"),
        ]
    ).unwrap();

    let token = dotenv::var("DISCORD_BOT_TOKEN").unwrap();
    let db_url = dotenv::var("DISCORD_BOT_DB").unwrap();

    let manager = ConnectionManager::<PgConnection>::new(db_url);
    let pool = r2d2::Pool::builder().build(manager).unwrap();

    let mut client = Client::new(&token, Handler).unwrap();

    client.threadpool.set_num_threads(20);

    let setup_fns = &[setup,
                      commands::tags::setup_tags,
                      commands::admin::setup_admin,
                      commands::reminders::setup_reminders,
                      commands::markov::setup_markov,
                      commands::misc::setup_misc,
                      commands::booru::setup_booru,
                      commands::prefixes::setup_prefixes,
                      commands::gimage::setup_gimage,
                     ];

    let framework = setup_fns.iter().fold(
        StandardFramework::new(),
        | acc, fun | fun(&mut client, acc));

    client.with_framework(framework);

    {
        let mut data = client.data.lock();
        data.insert::<ShardManagerContainer>(Arc::clone(&client.shard_manager));
        data.insert::<PgConnectionManager>(pool);
        data.insert::<StartTime>(chrono::Utc::now().naive_utc());
        data.insert::<CmdCounter>(Arc::new(RwLock::new(0)));
        data.insert::<PrefixCache>(LruCache::new(1000));
        data.insert::<ThreadPoolCache>(Arc::new(Mutex::new(client.threadpool.clone())));
    }

    if let Err(why) = client.start_autosharded() {
        println!("AAA: {:?}", why);
    }
}
