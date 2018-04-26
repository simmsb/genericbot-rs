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

struct ImageClient;

impl Key for ImageClient {
    type Value = reqwest::Client;
}

fn make_gimage_client() -> reqwest::Client {
    // let mut headers = header::Headers::new();
    reqwest::Client::builder()
        .build()
        .unwrap()
}

struct ImageResponse {
    url: String,
}

impl ImageResponse {
    fn search(client: &Client, value: &str) -> Self {
        // TODO client.get()
    }
}


pub fn setup_gimage(client: &mut Client, frame: StandardFramework) -> StandardFramework {
    {
        let mut data = client.data.lock();
        data.insert::<ImageClient>(make_gimage_client());
    }

    frame
}
