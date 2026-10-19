//! Every command. Each works with the prefix (`!play`) and as a slash command (`/play`).

mod general;
mod playback;
mod queue;

use crate::data::{Data, Error};

pub fn all() -> Vec<poise::Command<Data, Error>> {
    vec![
        // Playing
        playback::play(),
        playback::playnext(),
        playback::pause(),
        playback::resume(),
        playback::skip(),
        playback::skipto(),
        playback::stop(),
        playback::seek(),
        playback::replay(),
        // Queue
        queue::queue(),
        queue::nowplaying(),
        queue::shuffle(),
        queue::remove(),
        queue::clear(),
        queue::r#loop(),
        // General
        general::join(),
        general::leave(),
        general::volume(),
        general::help(),
        general::ping(),
    ]
}
