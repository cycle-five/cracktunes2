pub mod music;
pub use music::{join, play, show_queue, skip, stop};

use crate::Data;
use poise::Command;

pub fn all_music_commands() -> Vec<Command<Data, serenity::Error>> {
    vec![
        join(),
        play(),
        //pause(),
        //resume(),
        skip(),
        stop(),
        show_queue(),
    ]
}
