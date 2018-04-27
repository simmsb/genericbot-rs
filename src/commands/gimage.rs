use serenity::{
    prelude::*,
    framework::standard::{
        StandardFramework,
    },
};
use reqwest::header;
use reqwest;
use rand::{Rng, thread_rng};
use typemap::Key;
use regex::Regex;

struct ImageClient;

impl Key for ImageClient {
    type Value = reqwest::Client;
}

fn make_gimage_client() -> reqwest::Client {
    let mut headers = header::Headers::new();
    headers.set(header::UserAgent::new("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/57.0.2987.133 Safari/537.36"));
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap()
}

struct ImageResponse;

impl ImageResponse {
    fn params<'a>(search: &'a str) -> Vec<(&'static str, &'a str)> {
        vec![("q", search), ("tbm", "isch"), ("safe", "high")]
    }

    fn search_for(client: &reqwest::Client, search: &str) -> Result<String, &'static str> {
        let params = Self::params(search);
        client.get("https://www.google.com/search")
              .query(&params)
              .send().map_err(|_| "No response from google")?
              .text().map_err(|_| "Invalid response from google")
    }

    fn select_response(resp: String) -> Result<String, &'static str> {
        lazy_static! {
            static ref IMG_REGEX: Regex = Regex::new(r#""ou":"([^"]*)""#).unwrap();
        }

        let images: Vec<_> = IMG_REGEX.captures_iter(&resp).collect();
        return thread_rng().choose(&images).map(|s| s.get(1).unwrap().as_str().to_owned()).ok_or("No images found");
    }

    fn search(client: &reqwest::Client, search: &str) -> Result<String, &'static str> {
        let resp = Self::search_for(&client, search)?;
        Self::select_response(resp)
    }
}


command!(gimage_cmd(ctx, msg, args) {
    let search = args.full();

    let client = {
        let lock = ctx.data.lock();
        lock.get::<ImageClient>().unwrap().clone()
    };

    let resp = ImageResponse::search(&client, &search)?;

    void!(msg.channel_id.send_message(|m| m.embed(
        |e| e.colour(0xaf38e4)
             .title(format!("GImage response for {}", search))
             .image(resp)
    )));
});


pub fn setup_gimage(client: &mut Client, frame: StandardFramework) -> StandardFramework {
    {
        let mut data = client.data.lock();
        data.insert::<ImageClient>(make_gimage_client());
    }

    frame.group("GImage",
                |g| g
                .command("gimage", |c| c
                         .cmd(gimage_cmd)
                         .desc("Search google for images")
                         .example("memes")
                         .usage("{search string}")
                )
    )
}
