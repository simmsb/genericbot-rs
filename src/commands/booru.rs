use serenity::{
    prelude::*,
    builder::CreateEmbed,
    framework::standard::{
        StandardFramework,
    },
};
use serde_json::Value;
use serde_json;
use reqwest::header;
use reqwest;
use itertools::Itertools;
use rand::{Rng, thread_rng};
use typemap::Key;
use std::marker;
use failure::Error;
use utils::{nsfw_check, send_message, say};


#[derive(Debug, Fail)]
enum BooruError {
    #[fail(display = "The booru did not respond.")]
    NoResponse,
    #[fail(display = "The booru did not provide a valid response.")]
    InvalidResponse,
    #[fail(display = "No images found for tags: `{}`.", _0)]
    NoImages(String),
}


struct BooruClient;

impl Key for BooruClient {
    type Value = reqwest::Client;
}

impl BooruClient {
    fn generate() -> reqwest::Client {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::USER_AGENT, "genericBot Discord Bot: https://github.com/nitros12/genericbot-rs".parse().unwrap());
        reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap()
    }
}


struct BooruResponse<T: ?Sized + BooruRequestor> {
    image_url: String,
    tags: String,
    page_url: String,
    _marker: marker::PhantomData<T>,  // thonk
}

impl<T: ?Sized + BooruRequestor> BooruResponse<T> {
    fn new(image_url: String, tags: String, page_url: String) -> BooruResponse<T> {

        debug!("creating with image_url: {}", image_url);

        BooruResponse {
            image_url,
            tags,
            page_url,
            _marker: marker::PhantomData,
        }
    }

    fn generate_embed(&self, e: CreateEmbed) -> CreateEmbed {
        let tags = self.tags
                       .replace("_", "\\_")
                       .chars()
                       .take(1024)
                       .collect::<String>();

        let mut e = e.colour(0xc3569f)
                     .title(format!("Booru response for {}", T::BOORU_NAME))
                     .image(&self.image_url)
                     .url(&self.page_url)
                     .field("Tags", &tags, true);

        if let Some(s) = T::THUMBURL {
            e = e.thumbnail(s);
        }

        e
    }
}


#[derive(Clone)]
struct BooruContext<'a> {
    client: &'a reqwest::Client,
    tags: &'a [String],
    params: Option<Vec<(&'static str, String)>>,
}

impl<'a> BooruContext<'a> {
    fn new(client: &'a reqwest::Client, tags: &'a [String], params: Option<Vec<(&'static str, String)>>) -> Self {
        BooruContext {
            client,
            tags,
            params,
        }
    }
}


trait BooruRequestor {
    fn search_for(ctx: &BooruContext) -> Result<String, Error> {
        let mut params = Self::params(ctx);
        if let Some(ref p) = ctx.params {
            params.extend(p.clone());
        }

        ctx.client.get(&Self::full_url())
              .query(&params)
              .send().map_err(|_| BooruError::NoResponse)?
              .text().map_err(|_| BooruError::InvalidResponse.into())
    }

    const BOORU_NAME: &'static str;
    const BASE_URL: &'static str;
    const EXTENSION: &'static str;
    const THUMBURL: Option<&'static str>;
    const PAGE_PATH: &'static str;
    const URL_KEY: &'static str = "file_url";
    const TAG_KEY: &'static str = "tags";

    fn full_url() -> String {
        format!("{}{}", Self::BASE_URL, Self::EXTENSION)
    }

    fn params(ctx: &BooruContext) -> Vec<(&'static str, String)> {
        vec![("tags", ctx.tags.iter().join(" "))]
    }

    fn select_response<'a>(ctx: &BooruContext, resps: &'a [Value],
                           key: &'static str) -> Result<&'a Value, Error>
    {
        // get all the image urls from each response
        let mut urls: Vec<_> = resps
            .iter()
            .map(|v| v[&key].as_str())
            .enumerate()
            .collect();

        thread_rng().shuffle(&mut urls);

        // for each url, check that a HTTP HEAD on the url gives a result
        for (index, url) in urls {
            let url = try_opt_continue!(url);
            let resp = try_continue!(ctx.client.head(url).send());

            if resp.status().is_success() {
                return Ok(&resps[index]);
            }
        }

        Err(BooruError::NoImages(ctx.tags.iter().join(" ")).into())
    }

    fn parse_response(val: &str) -> Result<Vec<Value>, Error> {
        serde_json::from_str(val).map_err(|_| BooruError::InvalidResponse.into())
    }

    fn get_page(val: &Value) -> Result<String, Error> {
        let id = val["id"].as_u64().ok_or(BooruError::InvalidResponse)?;

        Ok(format!("{}/{}{}", Self::BASE_URL, Self::PAGE_PATH, id))
    }

    fn search(ctx: &BooruContext) -> Result<BooruResponse<Self>, Error> {
        let resp = Self::search_for(ctx)?;
        let parsed = Self::parse_response(&resp)?;
        let selected = Self::select_response(ctx, &parsed, Self::URL_KEY)?;

        Ok(BooruResponse::new(
            selected[Self::URL_KEY]
                .as_str()
                .map(|s| s.to_owned())
                .ok_or(BooruError::InvalidResponse)?,
            selected[Self::TAG_KEY]
                .as_str()
                .map(|s| s.to_owned())
                .ok_or(BooruError::InvalidResponse)?,
            Self::get_page(selected)?
        ))

    }
}


struct Ninja;


impl BooruRequestor for Ninja {
    const BOORU_NAME: &'static str = "Cure.Ninja";

    const BASE_URL: &'static str = "https://cure.ninja";

    const EXTENSION: &'static str = "/booru/api/json";

    const THUMBURL: Option<&'static str> = None;

    const URL_KEY: &'static str = "url";

    // we don't use this
    const PAGE_PATH: &'static str = "";

    fn params(ctx: &BooruContext) -> Vec<(&'static str, String)> {
        vec![("q", ctx.tags.iter().join(" ")), ("o", "r".to_owned())]
    }

    fn parse_response(val: &str) -> Result<Vec<Value>, Error> {
        let parsed: Value = serde_json::from_str(val).map_err(|_| BooruError::InvalidResponse)?;
        parsed["results"].as_array()
                         .map(|v| v.to_owned())
                         .ok_or_else(|| BooruError::InvalidResponse.into())
    }

    fn get_page(val: &Value) -> Result<String, Error> {
        val["page"].as_str()
                   .map(|s| s.to_owned())
                   .ok_or_else(|| BooruError::InvalidResponse.into())
    }
}


macro_rules! booru_def {
    ( booru: $booru:ident,
      cmd_name: $cmd_name:ident,
      base: $base:expr,
      ext: $ext:expr,
      thumb: $thumb:expr,
      url_key: $url_key:expr,
      tag_key: $tag_key:expr,
      page_path: $page_path:expr,
      $({ $($extras:tt)* }),*
    ) => (
        struct $booru;
        impl BooruRequestor for $booru {
            const BOORU_NAME: &'static str = stringify!($booru);

            const BASE_URL: &'static str = $base;

            const EXTENSION: &'static str = $ext;

            const THUMBURL: Option<&'static str> = $thumb;

            const URL_KEY: &'static str = $url_key;

            const TAG_KEY: &'static str = $tag_key;

            const PAGE_PATH: &'static str = $page_path;

            $($($extras)*)*
        }

        command!($cmd_name(ctx, msg, args) {
            let client = {
                let lock = ctx.data.lock();
                lock.get::<BooruClient>().unwrap().clone()
            };

            let tags = args.multiple::<String>().unwrap_or_else(|_| Vec::new());

            let ctx = BooruContext::new(&client, &tags, None);
            let response = <$booru as BooruRequestor>::search(&ctx)?;

            void!(send_message(msg.channel_id, |m| m.embed(|e| response.generate_embed(e))));
        });
    )
}


booru_def!(
    booru: DanBooru,
    cmd_name: danbooru_cmd,
    base: "https://danbooru.donmai.us",
    ext: "/posts.json",
    thumb: Some("https://i.imgur.com/1Sk5Bp4.png"),
    url_key: "file_url",
    tag_key: "tag_string",
    page_path: "posts/",

    {
        fn params(ctx: &BooruContext) -> Vec<(&'static str, String)> {
            vec![("tags", ctx.tags.iter().join(" ")), ("random", "true".to_owned())]
        }
    }
);


booru_def!(
    booru: E621,
    cmd_name: e621_cmd,
    base: "https://e621.net",
    ext: "/post/index.json",
    thumb: Some("http://emblemsbf.com/img/63681.jpg"),
    url_key: "file_url",
    tag_key: "tags",
    page_path: "post/show/",
);


booru_def!(
    booru: E926,
    cmd_name: e926_cmd,
    base: "https://e926.net",
    ext: "/post/index.json",
    thumb: Some("http://emblemsbf.com/img/63681.jpg"),
    url_key: "file_url",
    tag_key: "tags",
    page_path: "post/show/",
);


// https://gelbooru.com/index.php?page=post&s=view&id=4310590

booru_def!(
    booru: GelBooru,
    cmd_name: gelbooru_cmd,
    base: "https://gelbooru.com",
    ext: "/index.php",
    thumb: Some("https://i.imgur.com/Aeabusr.png"),
    url_key: "file_url",
    tag_key: "tags",
    page_path: "index.php?page=post&s=view&id=",

    {
        fn params(ctx: &BooruContext) -> Vec<(&'static str, String)> {
            vec![("page", "dapi".to_owned()),
                 ("s", "post".to_owned()),
                 ("q", "index".to_owned()),
                 ("json", "1".to_owned()),
                 ("tags", ctx.tags.iter().join(" "))
            ]
        }
    }
);


booru_def!(
    booru: SafeBooru,
    cmd_name: safebooru_cmd,
    base: "https://safebooru.org",
    ext: "/index.php",
    thumb: None,
    url_key: "fixed", // we have a specialist fixup
    tag_key: "tags",
    page_path: "index.php?page=post&s=view&id=",

    {
        fn params(ctx: &BooruContext) -> Vec<(&'static str, String)> {
            vec![("page", "dapi".to_owned()),
                 ("s", "post".to_owned()),
                 ("q", "index".to_owned()),
                 ("json", "1".to_owned()),
                 ("tags", ctx.tags.iter().join(" "))
            ]
        }

        fn parse_response(val: &str) -> Result<Vec<Value>, Error> {
            let mut parsed: Vec<Value> = serde_json::from_str(val).map_err(|_| BooruError::InvalidResponse)?;

            for mut elem in &mut parsed {

                let fixed = json!(format!("{}/images/{}/{}",
                                          Self::BASE_URL,
                                          elem["directory"].as_str().ok_or(BooruError::InvalidResponse)?,
                                          elem["image"].as_str().ok_or(BooruError::InvalidResponse)?));

                elem["fixed"] = fixed;
            }
            Ok(parsed)
        }
    }
);


booru_def!(
    booru: Yandere,
    cmd_name: yandere_cmd,
    base: "https://yande.re",
    ext: "/post.json",
    thumb: Some("https://i.imgur.com/B6TiG94.png"),
    url_key: "file_url",
    tag_key: "tags",
    page_path: "post/show/",
);


booru_def!(
    booru: genericBooru,
    cmd_name: genericbooru_cmd,
    base: "https://genericbooru.moe",
    ext: "/post/index.json",
    thumb: None,
    url_key: "file_url",
    tag_key: "tags",
    page_path: "post/show/",

    {
        fn params(ctx: &BooruContext) -> Vec<(&'static str, String)> {
            vec![("tags", ctx.tags.iter().chain(&["-rating:e".to_owned()]).join(" "))]
        }

        fn parse_response(val: &str) -> Result<Vec<Value>, Error> {
            let mut parsed: Vec<Value> = serde_json::from_str(val).map_err(|_| BooruError::InvalidResponse)?;

            for mut elem in &mut parsed {

                let mut ext = elem[Self::URL_KEY].as_str()
                                                 .ok_or(BooruError::InvalidResponse)?
                                                 .chars()
                                                 .rev()
                                                 .take(3)
                                                 .collect::<Vec<_>>();
                ext.reverse();

                let ext = ext.into_iter().collect::<String>();

                let fixed = json!(format!("{}/image/{}.{}",
                                          Self::BASE_URL,
                                          elem["md5"].as_str().ok_or(BooruError::InvalidResponse)?,
                                          ext));

                elem["file_url"] = fixed;
            }
            Ok(parsed)
        }
    }
);


command!(ninja_cmd(ctx, msg, args) {
    let client = {
        let lock = ctx.data.lock();
        lock.get::<BooruClient>().unwrap().clone()
    };

    let tags = args.multiple::<String>().unwrap_or_else(|_| Vec::new());

    let is_nsfw = msg.channel_id.to_channel_cached().map_or(false, |c| c.is_nsfw());
    let nsfw_key = if is_nsfw { "a" } else { "s" }; // a = any, s = safe

    let ctx = BooruContext::new(&client, &tags, Some(vec![("f", nsfw_key.to_owned())]));
    let response = Ninja::search(&ctx)?;

    void!(send_message(msg.channel_id, |m| m.embed(|e| response.generate_embed(e))));
});


command!(booru_bomb(ctx, msg, args) {
    use ::ThreadPoolCache;

    let threadpool = {
        let lock = ctx.data.lock();
        let threadpool = lock.get::<ThreadPoolCache>().unwrap().lock().clone();
        threadpool
    };

    let tags = args.multiple::<String>().unwrap_or_else(|_| Vec::new());

    let channel_id = msg.channel_id;

    macro_rules! run_booru {
        ( $booru:ident ) => ( {
            let data = ctx.data.clone();
            let tags = tags.clone();
            threadpool.execute(move || {
                let client = data.lock().get::<BooruClient>().unwrap().clone();

                let b_ctx = BooruContext::new(&client, &tags, None);

                let resp = <$booru as BooruRequestor>::search(&b_ctx);
                match resp {
                    Ok(r) => void!(send_message(channel_id, |m| m.embed(|e| r.generate_embed(e)))),
                    Err(e) => void!(say(channel_id, format!("{}: {}", stringify!($booru), e))),
                }
            })
        } )
    }

    run_booru!(DanBooru);
    run_booru!(GelBooru);
    run_booru!(SafeBooru);
    run_booru!(Yandere);
    run_booru!(genericBooru);
});


pub fn setup_booru(client: &mut Client, frame: StandardFramework) -> StandardFramework {
    {
        let mut data = client.data.lock();
        data.insert::<BooruClient>(BooruClient::generate());
    }

    frame
        .bucket("booru_bucket", 3, 10, 2)
        .group("Booru",
               |g| g
               .bucket("booru_bucket")
               .command("booru", |c| c
                        .cmd(ninja_cmd)
                        .desc("Search cure.ninja for images.")
               )
               .command("danbooru", |c| c
                        .cmd(danbooru_cmd)
                        .desc("Search danbooru for images.")
                        .check(nsfw_check)
                        .batch_known_as(&["db"])
               )
               .command("e621", |c| c
                        .cmd(e621_cmd)
                        .desc("Search e621 for images.")
                        .check(nsfw_check)
               )
               .command("e926", |c| c
                        .cmd(e926_cmd)
                        .desc("Search e926 for images.")
               )
               .command("gelbooru", |c| c
                        .cmd(gelbooru_cmd)
                        .desc("Search gelbooru for images.")
                        .check(nsfw_check)
                        .batch_known_as(&["gb", "gel"])
               )
               .command("safebooru", |c| c
                        .cmd(safebooru_cmd)
                        .desc("Search safebooru for images.")
                        .batch_known_as(&["sb", "safe"])
               )
               .command("yandere", |c| c
                        .cmd(yandere_cmd)
                        .desc("Search yandere for images.")
                        .check(nsfw_check)
                        .batch_known_as(&["yan"])
               )
               .command("genericbooru", |c| c
                        .cmd(genericbooru_cmd)
                        .desc("Search genericbooru for images.")
                        .check(nsfw_check)
                        .batch_known_as(&["generic"])
               )
               .command("booru_bomb", |c| c
                        .cmd(booru_bomb)
                        .desc("Search each booru for an image.")
                        .check(nsfw_check)
               )
    )
}
