pub mod music;
pub use music::{join, leave, pause, play, resume, show_queue, shuffle, skip, stop, volume};

use crate::Data;
use poise::Command;

/// This function returns a vector of all music-related commands.
#[must_use]
pub fn all_music_commands() -> Vec<Command<Data, crack_types::Error>> {
    vec![
        join(),
        leave(),
        play(),
        pause(),
        resume(),
        shuffle(),
        skip(),
        stop(),
        show_queue(),
        volume(),
    ]
}
