pub mod schema;
pub mod models;
pub mod myframework;

#[macro_use]
extern crate serenity;
extern crate dotenv;
#[macro_use]
extern crate diesel;
extern crate chrono;
extern crate typemap;
extern crate threadpool;

use serenity::prelude::*;
use serenity::model::channel::Message;
use serenity::client::bridge::gateway::{ShardManager};
use serenity::model::gateway::Ready;
use serenity::framework::standard::StandardFramework;

use diesel::prelude::*;
use diesel::pg::PgConnection;

use std::sync::Arc;
use typemap::Key;

struct Handler;

impl EventHandler for Handler {
    fn ready(&self, _: Context, ready: Ready) {
        if let Some(shard) = ready.shard {
            println!("Connected as: {} on shard {} of {}", ready.user.name, shard[0], shard[1]);
        }
    }
}


struct ShardManagerContainer;

impl Key for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

struct PgConnectionManager;

impl Key for PgConnectionManager {
    type Value = Arc<Mutex<PgConnection>>;
}


fn get_prefixes(ctx: Context, m: Message) -> Option<Vec<String>> {
    use models::Prefix;
    use schema::prefix::dsl::*;

    let data = ctx.data.lock();
    let db_conn = &*data.get::<PgConnectionManager>().unwrap().lock();

    if let Some(g_id) = m.guild_id() {
        let prefixes = prefix.filter(guild_id.eq(g_id.0 as i64))
            .load::<Prefix>(db_conn)
            .expect("Error loading prefixes")
            .into_iter()
            .map(|p| p.pre)
            .collect();
        Some(prefixes)
    } else {
        None
    }
}


fn main() {
    let token = dotenv::var("DISCORD_BOT_TOKEN").unwrap();
    let log_chan = dotenv::var("DISCORD_BOT_LOG_CHAN").unwrap();
    let db_url = dotenv::var("DISCORD_BOT_DB").unwrap();

    let pg_conn = Arc::new(Mutex::new(PgConnection::establish(&db_url).unwrap()));
    let mut client = Client::new(&token, Handler).unwrap();

    client.with_framework(
        StandardFramework::new()
            .configure(| c | c.prefix("!"))
    );

    {
        let mut data = client.data.lock();
        data.insert::<ShardManagerContainer>(Arc::clone(&client.shard_manager));
        data.insert::<PgConnectionManager>(pg_conn);
    }


    if let Err(why) = client.start_autosharded() {
        println!("AAA: {:?}", why);
    }
}
