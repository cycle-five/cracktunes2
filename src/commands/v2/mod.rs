mod connection;
mod music;
mod utils;
mod contextual_music;

pub use connection::get_connection_commands;
pub use music::get_music_commands;
pub use contextual_music::get_contextual_commands;

use crate::core::models::error::AppError;
use crate::infrastructure::setup::Data;

/// Get all commands from the new architecture
#[must_use]
pub fn get_all_v2_commands() -> Vec<poise::Command<Data, AppError>> {
    let mut commands = Vec::new();

    // Add music commands
    commands.extend(music::get_music_commands());

    // Add connection commands
    commands.extend(connection::get_connection_commands());

    // Add utility commands if implemented
    // commands.extend(utils::get_utility_commands());
    
    // Contextual commands are commented out until fully integrated
    // commands.extend(contextual_music::get_contextual_commands());

    commands
}
