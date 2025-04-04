mod music;
mod connection;
mod utils;

pub use music::get_music_commands;
pub use connection::get_connection_commands;

use crate::core::models::error::AppError;
use crate::infrastructure::setup::Data;

/// Get all commands from the new architecture
#[must_use] pub fn get_all_v2_commands() -> Vec<poise::Command<Data, AppError>> {
    let mut commands = Vec::new();
    
    // Add music commands
    commands.extend(music::get_music_commands());
    
    // Add connection commands
    commands.extend(connection::get_connection_commands());
    
    commands
}