use crate::commands::v2::utils::{get_guild_id, get_text_channel_id, get_user_voice_channel};
use crate::core::models::error::AppError;
use crate::infrastructure::setup::Data;

/// Pings the bot
#[poise::command(slash_command, prefix_command)]
pub async fn ping(ctx: poise::Context<'_, Data, AppError>) -> Result<(), AppError> {
    ctx.say("Pong!").await?;
    Ok(())
}

/// Join the user's voice channel
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn join(ctx: poise::Context<'_, Data, AppError>) -> Result<(), AppError> {
    // Get required IDs
    let guild_id = get_guild_id(ctx)?;
    let voice_channel_id = get_user_voice_channel(ctx).await?;
    let text_channel_id = get_text_channel_id(ctx);

    // Join using the music service
    ctx.data()
        .services
        .music_service
        .join(guild_id, voice_channel_id, text_channel_id)
        .await?;

    Ok(())
}

/// Leave the voice channel
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn leave(ctx: poise::Context<'_, Data, AppError>) -> Result<(), AppError> {
    // Get required IDs
    let guild_id = get_guild_id(ctx)?;
    let text_channel_id = get_text_channel_id(ctx);

    // Leave using the music service
    ctx.data()
        .services
        .music_service
        .leave(guild_id, text_channel_id)
        .await?;

    Ok(())
}

/// Set the voice channel idle timeout (0 = never leave)
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn timeout(
    ctx: poise::Context<'_, Data, AppError>,
    #[description = "Timeout in minutes (0 = never leave)"] minutes: Option<u64>,
) -> Result<(), AppError> {
    // Get required IDs
    let guild_id = get_guild_id(ctx)?;
    let text_channel_id = get_text_channel_id(ctx);

    // Set timeout using the connection service
    ctx.data()
        .services
        .music_service
        .set_idle_timeout(guild_id, text_channel_id, minutes)
        .await?;

    Ok(())
}

/// Returns all connection-related commands
#[must_use]
pub fn get_connection_commands() -> Vec<poise::Command<Data, AppError>> {
    vec![join(), leave(), timeout(), ping()]
}
