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
use utils::{nsfw_check, send_message};

struct BooruClient;

impl Key for BooruClient {
    type Value = reqwest::Client;
}

impl BooruClient {
    fn generate() -> reqwest::Client {
        let mut headers = header::Headers::new();
        headers.set(header::UserAgent::new("genericBot Discord Bot: https://github.com/nitros12/genericbot-rs"));
        reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap()
    }
}


struct BooruResponse<T: BooruRequestor> {
    image_url: String,
    tags: String,
    source_url: Option<String>,
    _marker: marker::PhantomData<T>,  // thonk
}


impl<T: BooruRequestor> BooruResponse<T> {
    fn new(image_url: String, tags: String, source_url: Option<String>) -> BooruResponse<T> {

        debug!("creating with image_url: {}", image_url);

        BooruResponse {
            image_url,
            tags,
            source_url,
            _marker: marker::PhantomData,
        }
    }

    fn generate_embed(&self, e: CreateEmbed) -> CreateEmbed {
        let tags = self.tags
                       .chars()
                       .take(1024)
                       .collect::<String>()
            .replace("_", "\\_");

        let mut e = e.colour(0xc3569f)
                     .title(format!("Booru response for {}", T::booru_name()))
                     .image(&self.image_url)
                     .field("Tags", &tags, true);

        if let Some(s) = self.source_url.as_ref() {
            e = e.url(s);
        }

        if let Some(s) = T::thumburl() {
            e = e.thumbnail(s);
        }

        return e;
    }
}


trait BooruRequestor
    where Self: Sized,
{
    fn search_for(client: &reqwest::Client, tags: &Vec<String>, extra_params: Option<Vec<(&'static str, String)>>) -> Result<String, &'static str> {
        let mut params = Self::params(&tags);
        if let Some(p) = extra_params {
            params.extend(p);
        }

        let resp = client.get(&Self::full_url())
                         .query(&params)
                         .send().map_err(|_| "No response from api")?
                         .text().map_err(|_| "Failed api response")?;
        if resp.is_empty() {
            Err("No images found.")
        } else {
            Ok(resp)
        }
    }

    fn booru_name() -> &'static str;
    fn base_url() -> &'static str;
    fn extension() -> &'static str;
    fn thumburl() -> Option<&'static str>;

    fn full_url() -> String {
        format!("{}{}", Self::base_url(), Self::extension())
    }

    fn params(tags: &Vec<String>) -> Vec<(&'static str, String)> {
        vec![("tags", tags.iter().join(" "))]
    }

    fn select_response<'a>(client: &reqwest::Client, resps: &'a Vec<Value>,
                       key: &'static str) -> Result<&'a Value, &'static str>
    {
        let mut urls: Vec<_> = resps
            .iter()
            .map(|v| v[&key].as_str())
            .enumerate()
            .collect();

        thread_rng().shuffle(&mut urls);

        for (index, url) in urls {
            if let Some(url) = url {
                if let Ok(r) = client.head(url).send() {
                    if r.status().is_success() {
                        return Ok(&resps[index]);
                    }
                }
            }
        }

        Err("No valid responses")
    }

    fn parse_response(val: &str) -> Result<Vec<Value>, &'static str> {
        serde_json::from_str(val).map_err(|_| "Failed to parse response")
    }

    fn url_key() -> &'static str {
        "file_url"
    }

    fn response_keys() -> (&'static str, &'static str, Option<&'static str>) {
        (Self::url_key(), "tags", Some("source"))
    }

    fn search(client: &reqwest::Client, tags: &Vec<String>,
              extra_params: Option<Vec<(&'static str, String)>>
    ) -> Result<BooruResponse<Self>, &'static str>
    {
        let resp = Self::search_for(&client, &tags, extra_params)?;
        let parsed = Self::parse_response(&resp)?;
        let selected = Self::select_response(&client, &parsed, Self::url_key())?;

        let (image_url_key, tag_key, source_key) = Self::response_keys();

        Ok(BooruResponse::new(
            selected[&image_url_key]
                .as_str()
                .map(|s| s.to_owned())
                .ok_or("No image url found")?,
            selected[&tag_key]
                .as_str()
                .map(|s| s.to_owned())
                .ok_or("No tags found")?,
            source_key
                .and_then(|k| selected[&k]
                          .as_str()
                          .map(|s| s.to_owned()))
        ))

    }
}


struct Ninja;


impl BooruRequestor for Ninja {
    fn booru_name() -> &'static str {
        "Cure.Ninja"
    }

    fn base_url() -> &'static str {
        "https://cure.ninja"
    }

    fn extension() -> &'static str {
        "/booru/api/json"
    }

    fn thumburl() -> Option<&'static str> {
        None
    }

    fn url_key() -> &'static str {
        "url"
    }

    fn response_keys() -> (&'static str, &'static str, Option<&'static str>) {
        (Self::url_key(), "tag", Some("sourceURL"))
    }

    fn params(tags: &Vec<String>) -> Vec<(&'static str, String)> {
        vec![("q", tags.iter().join(" ")), ("o", "r".to_owned())]
    }

    fn parse_response(val: &str) -> Result<Vec<Value>, &'static str> {
        // We used to have to remove some extra crap at the start
        // we don't now
        // fn cut_misformed(val: &str) -> Result<&str, &'static str> {
        //     let mut count = 0;

        //     for (i, c) in val.chars().enumerate() {
        //         if c == '{' {
        //             count += 1;
        //         } else if c == '}' {
        //             count -= 1;

        //             if count == 0 {
        //                 // i + 2, to move over the }\n
        //                 return Ok(&val[(i + 2)..]);
        //             }
        //         }
        //     }

        //     Err("Couldn't fixup json")
        // }

        // let fixed = cut_misformed(val)?;
        let parsed: Value = serde_json::from_str(val).map_err(|_| "Failed to parse response")?;
        parsed["results"].as_array().map(|v| v.to_owned()).ok_or("No results found")
    }
}


macro_rules! booru_def {
    ( booru: $booru:ident,
      cmd_name: $cmd_name:ident,
      base: $base:expr,
      ext: $ext:expr,
      thumb: $thumb:expr,
      url_key: $url_key:expr,
      response_keys: ($tag_key:expr, $source_key:expr),
      $({ $($extras:tt)* }),*
    ) => (
        struct $booru;
        impl BooruRequestor for $booru {
            fn booru_name() -> &'static str {
                stringify!($booru)
            }

            fn base_url() -> &'static str {
                $base
            }

            fn extension() -> &'static str {
                $ext
            }

            fn thumburl() -> Option<&'static str> {
                $thumb
            }

            fn url_key() -> &'static str {
                $url_key
            }

            fn response_keys() -> (&'static str, &'static str, Option<&'static str>) {
                (Self::url_key(), $tag_key, $source_key)
            }

            $($($extras)*)*
        }

        command!($cmd_name(ctx, msg, args) {
            let client = {
                let lock = ctx.data.lock();
                lock.get::<BooruClient>().unwrap().clone()
            };

            let tags = args.multiple::<String>().unwrap_or_else(|_| Vec::new());

            let response = <$booru as BooruRequestor>::search(&client, &tags, None)?;

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
    response_keys: ("tag_string", Some("source")),

    {
        fn params(tags: &Vec<String>) -> Vec<(&'static str, String)> {
            vec![("tags", tags.iter().join(" ")), ("random", "true".to_owned())]
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
    response_keys: ("tags", Some("source")),
);


booru_def!(
    booru: E926,
    cmd_name: e926_cmd,
    base: "https://e926.net",
    ext: "/post/index.json",
    thumb: Some("http://emblemsbf.com/img/63681.jpg"),
    url_key: "file_url",
    response_keys: ("tags", Some("source")),
);


booru_def!(
    booru: GelBooru,
    cmd_name: gelbooru_cmd,
    base: "https://gelbooru.com",
    ext: "/index.php",
    thumb: Some("https://i.imgur.com/Aeabusr.png"),
    url_key: "file_url",
    response_keys: ("tags", None),

    {
        fn params(tags: &Vec<String>) -> Vec<(&'static str, String)> {
            vec![("page", "dapi".to_owned()),
                 ("s", "post".to_owned()),
                 ("q", "index".to_owned()),
                 ("json", "1".to_owned()),
                 ("tags", tags.iter().join(" "))
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
    response_keys: ("tags", None),

    {
        fn params(tags: &Vec<String>) -> Vec<(&'static str, String)> {
            vec![("page", "dapi".to_owned()),
                 ("s", "post".to_owned()),
                 ("q", "index".to_owned()),
                 ("json", "1".to_owned()),
                 ("tags", tags.iter().join(" "))
            ]
        }

        fn parse_response(val: &str) -> Result<Vec<Value>, &'static str> {
            let mut parsed: Vec<Value> = serde_json::from_str(val).map_err(|_| "Failed to parse response")?;

            for mut elem in &mut parsed {

                let fixed = json!(format!("{}/images/{}/{}",
                                          Self::base_url(),
                                          elem["directory"].as_str().ok_or("Failed to get a valid response")?,
                                          elem["image"].as_str().ok_or("Failed to get a valid response")?));

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
    response_keys: ("tags", Some("source")),
);


booru_def!(
    booru: genericBooru,
    cmd_name: genericbooru_cmd,
    base: "https://genericbooru.moe",
    ext: "/post/index.json",
    thumb: None,
    url_key: "file_url",
    response_keys: ("tags", Some("source")),

    {
        fn parse_response(val: &str) -> Result<Vec<Value>, &'static str> {
            let mut parsed: Vec<Value> = serde_json::from_str(val).map_err(|_| "Failed to parse response")?;

            for mut elem in &mut parsed {

                let mut ext = elem[Self::url_key()].as_str()
                                               .ok_or("Failed to get a valid response")?
                                               .chars()
                                               .rev()
                                               .take(3)
                                               .collect::<Vec<_>>();
                ext.reverse();

                let ext = ext.into_iter().collect::<String>();

                let fixed = json!(format!("{}/image/{}.{}",
                                          Self::base_url(),
                                          elem["md5"].as_str().ok_or("Failed to get a valid response")?,
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

    let is_nsfw = msg.channel_id.find().map_or(false, |c| c.is_nsfw());
    let nsfw_key = if is_nsfw { "a" } else { "s" }; // a = any, s = safe

    let response = Ninja::search(&client, &tags, Some(vec![("f", nsfw_key.to_owned())]))?;

    void!(send_message(msg.channel_id, |m| m.embed(|e| response.generate_embed(e))));
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
    )
}
