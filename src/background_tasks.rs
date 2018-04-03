use serenity::{
    prelude::*,
    utils,
};
use std::{
    sync::{
        Once,
        ONCE_INIT
    },
    thread,
    time,
};
use dotenv;
use hyper::{
    net::HttpsConnector,
    header::Authorization,
};
use hyper;
use hyper_native_tls::NativeTlsClient;

static START: Once = ONCE_INIT;


pub fn background_task() {

    START.call_once(|| {
        thread::spawn(
            move || {
                let botlist_key = match dotenv::var("DISCORD_BOT_LIST_TOKEN") {
                    Ok(x) => x.to_owned(),
                    _     => return,
                };

                loop {
                    thread::sleep(time::Duration::from_secs(5 * 60));

                    {
                        let tc = NativeTlsClient::new().unwrap();
                        let conn = HttpsConnector::new(tc);
                        let client = hyper::Client::with_connector(conn);

                        let bot_id = utils::with_cache(|c| c.user.id);
                        let guild_count = utils::with_cache(|c| c.all_guilds().len());
                        let header = Authorization(botlist_key.to_owned());

                        let _ = client.post(&format!("https://bots.discord.pw/api/bots/{}/stats", bot_id))
                                      .header(header)
                                      .body(&format!(r#"{{"server_count": {}}}"#, guild_count))
                                      .send();
                    }
                }});
    })
}
