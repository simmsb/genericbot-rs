use rand::{thread_rng, Rng};
use regex::Regex;
use reqwest;
use reqwest::header;
use serenity::{framework::standard::StandardFramework, prelude::*};
use typemap::Key;
use failure::Error;
use utils::send_message;


#[derive(Debug, Fail)]
enum ImageError {
    #[fail(display = "No response from google.")]
    NoResponse,
    #[fail(display = "Google did not provide a valid response.")]
    InvalidResponse,
    #[fail(display = "No images found for: `{}`", _0)]
    NoImages(String)
}


struct ImageClient;

impl Key for ImageClient {
    type Value = reqwest::Client;
}

impl ImageClient {
    fn generate() -> reqwest::Client {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/57.0.2987.133 Safari/537.36".parse().unwrap());
        reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap()
    }
}


struct ImageContext<'a> {
    client: &'a reqwest::Client,
    search_term: &'a str,
}

impl<'a> ImageContext<'a> {
    fn new(client: &'a reqwest::Client, search_term: &'a str) -> Self {
        ImageContext {
            client,
            search_term,
        }
    }
}


struct ImageResponse;

impl ImageResponse {
    fn params<'a>(ctx: &'a ImageContext) -> Vec<(&'static str, &'a str)> {
        vec![("q", ctx.search_term), ("tbm", "isch"), ("safe", "high")]
    }

    fn search_for(ctx: &ImageContext) -> Result<String, Error> {
        let params = Self::params(ctx);
        ctx.client
            .get("https://www.google.com/search")
            .query(&params)
            .send().map_err(|_| ImageError::NoResponse)?
            .text().map_err(|_| ImageError::InvalidResponse.into())
    }

    fn select_response(ctx: &ImageContext, resp: &str) -> Result<String, Error> {
        lazy_static! {
            static ref IMG_REGEX: Regex = Regex::new(r#""ou":"([^"]*)""#).unwrap();
        }

        let images: Vec<_> = IMG_REGEX
            .captures_iter(resp)
            .collect();

        thread_rng()
            .choose(&images)
            .map(|s| s.get(1).unwrap().as_str().to_owned())
            .ok_or_else(|| ImageError::NoImages(ctx.search_term.to_owned()).into())
    }

    fn search(ctx: &ImageContext) -> Result<String, Error> {
        let resp = Self::search_for(ctx)?;
        Self::select_response(ctx, &resp)
    }
}


command!(gimage_cmd(ctx, msg, args) {
    let search = args.full();

    let client = {
        let lock = ctx.data.lock();
        lock.get::<ImageClient>().unwrap().clone()
    };

    let ctx = ImageContext::new(&client, search);
    let resp = ImageResponse::search(&ctx)?;

    void!(send_message(msg.channel_id, |m| m.embed(
        |e| e.colour(0xaf38e4)
             .title(format!("GImage response for {}", search))
             .image(resp)
    )));
});


pub fn setup_gimage(client: &mut Client, frame: StandardFramework) -> StandardFramework {
    let mut data = client.data.lock();
    data.insert::<ImageClient>(ImageClient::generate());

    frame
        .bucket("gimage_bucket", 3, 10, 2)
        .group("GImage", |g| {
            g.bucket("gimage_bucket").command("gimage", |c| {
                c.cmd(gimage_cmd)
                    .desc("Search google for images")
                    .example("memes")
                    .usage("{search string}")
            })
        })
}
