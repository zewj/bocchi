//! Diagnostic: runs one song through the same pipeline the bot uses, without Discord.
//! Usage: cargo run --release --example probe -- "<link>"
#[path = "../src/data.rs"]
#[allow(dead_code)]
mod data;
#[path = "../src/source.rs"]
#[allow(dead_code)]
mod source;

use songbird::input::codecs::{get_codec_registry, get_probe};

#[tokio::main]
async fn main() {
    let url = std::env::args().nth(1).unwrap_or_else(|| "https://www.youtube.com/watch?v=dQw4w9WgXcQ".into());
    println!("1. streaming through yt-dlp: {url}");
    let started = std::time::Instant::now();
    match source::YtDlpStream::input(url, None).make_playable_async(get_codec_registry(), get_probe()).await {
        Ok(playable) => println!("2. OK in {:.1}s, playable = {}", started.elapsed().as_secs_f32(), playable.is_playable()),
        Err(e) => println!("2. FAILED: {e:?}"),
    }
}
