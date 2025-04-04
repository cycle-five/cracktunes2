// Original command modules
pub mod music;
pub use music::{join, leave, play, show_queue, shuffle, skip, stop};

// New hexagonal architecture command modules
pub mod v2;

use crate::Data;
use poise::Command;

/// This function returns a vector of all music-related commands from the original implementation.
/// Will be replaced by the new architecture commands over time.
#[must_use]
pub fn all_music_commands() -> Vec<Command<Data, crack_types::Error>> {
    vec![
        join(),
        leave(),
        play(),
        //pause(),
        //resume(),
        shuffle(),
        skip(),
        stop(),
        show_queue(),
    ]
}
