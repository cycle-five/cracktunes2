use crate::core::{
    models::{error::AppError, user::User},
    ports::audio_player::{GuildId, TextChannelId, VoiceChannelId},
};
use crate::infrastructure::setup::Data;

// --
// Utility functions for commands
// --

/// Get the current guild ID or return an error
pub fn get_guild_id(ctx: poise::Context<'_, Data, AppError>) -> Result<GuildId, AppError> {
    ctx.guild_id()
        .map(|id| GuildId(id.get()))
        .ok_or(AppError::NotInGuild)
}

/// Get the current text channel ID
pub fn get_text_channel_id(ctx: poise::Context<'_, Data, AppError>) -> TextChannelId {
    TextChannelId(ctx.channel_id().get())
}

/// Get the user's voice channel ID or return an error
pub async fn get_user_voice_channel(
    ctx: poise::Context<'_, Data, AppError>,
) -> Result<VoiceChannelId, AppError> {
    // let guild_id = get_guild_id(ctx)?;

    // Get the guild
    let guild = ctx.guild().ok_or(AppError::NotInGuild)?;

    // Get the user's voice state
    let user_id = ctx.author().id;

    guild
        .voice_states
        .get(&user_id)
        .and_then(|voice_state| voice_state.channel_id)
        .map(|id| VoiceChannelId(id.get()))
        .ok_or(AppError::NotInVoiceChannel)
}

/// Get information about the current user
pub fn get_user(ctx: poise::Context<'_, Data, AppError>) -> User {
    let author = ctx.author();
    User::new(author.id.get().to_string(), author.name.clone())
}

#[allow(dead_code)]
/// Get the bot's voice channel in the current guild, if any
pub fn get_bot_voice_channel(
    ctx: poise::Context<'_, Data, AppError>,
) -> Result<Option<VoiceChannelId>, AppError> {
    //let guild_id = get_guild_id(ctx)?;

    // Get the guild
    let guild = ctx.guild().ok_or(AppError::NotInGuild)?;

    // Get bot's voice state
    let bot_id = ctx.framework().bot_id();

    Ok(guild
        .voice_states
        .get(&bot_id)
        .and_then(|voice_state| voice_state.channel_id)
        .map(|id| VoiceChannelId(id.get())))
}
