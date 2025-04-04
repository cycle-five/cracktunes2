use poise::serenity_prelude as serenity;
use tracing::{debug, error};

use crate::adapters::discord::poise_contextual_handler::{PoiseContext, PoiseContextualHandler};
use crate::core::models::error::AppError;
use crate::core::ports::audio_player::GuildId;
use crate::core::ports::contextual_message_handler::ContextualMessageHandler;
use crate::core::services::contextual_music_service::ContextualMusicService;
use crate::infrastructure::setup::Data;
use crate::logging::{log_command_end_v2, log_command_start_v2};

/// Play a song or add it to the queue
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn play(
    ctx: PoiseContext<'_>,
    #[description = "YouTube URL or search term"] query: String,
) -> Result<(), AppError> {
    // Log the command
    log_command_start_v2(ctx);

    // Get the user's voice state to find their voice channel
    let guild_id = ctx.guild_id().ok_or(AppError::NotInGuild)?.get();
    let user_id = ctx.author().id;

    let voice_state = ctx
        .guild()
        .ok_or(AppError::NotInGuild)?
        .voice_states
        .get(&user_id)
        .ok_or(AppError::UserNotInVoiceChannel)?;

    let voice_channel_id = voice_state
        .channel_id
        .ok_or(AppError::UserNotInVoiceChannel)?
        .get();

    // Get the contextual music service from app data
    let data = ctx.data();
    
    // Create a handler for this context type
    let handler = PoiseContextualHandler::new();
    
    // Here's where we'd typically get the service from data:
    // let music_service = &data.services.contextual_music_service;
    
    // For demonstration, we'll stub this part as we're just showing the approach
    debug!("Would call music_service.play_song with context here");
    
    // This is how the actual call would look once implemented:
    // music_service
    //     .play_song(&ctx, &handler, GuildId(guild_id), voice_channel_id, &query)
    //     .await?;
    
    // For now, just send a message using our handler to demonstrate the approach
    handler
        .send_message_with_context(
            &ctx,
            &format!("Would play: {query} in channel {voice_channel_id}"),
        )
        .await
        .map_err(|e| AppError::MessageSendError(e.to_string()))?;

    // Log command end
    log_command_end_v2(ctx);
    Ok(())
}

/// Skip the current song
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn skip(ctx: PoiseContext<'_>) -> Result<(), AppError> {
    // Log the command
    log_command_start_v2(ctx);

    // Get the guild ID
    let guild_id = ctx.guild_id().ok_or(AppError::NotInGuild)?.get();

    // Get the contextual music service from app data
    let data = ctx.data();
    
    // Create a handler for this context type
    let handler = PoiseContextualHandler::new();
    
    // Demonstrate the handler usage
    handler
        .send_message_with_context(&ctx, "Would skip the current song")
        .await
        .map_err(|e| AppError::MessageSendError(e.to_string()))?;

    // Log command end
    log_command_end_v2(ctx);
    Ok(())
}

/// Stop playing and clear the queue
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn stop(ctx: PoiseContext<'_>) -> Result<(), AppError> {
    // Log the command
    log_command_start_v2(ctx);

    // Get the guild ID
    let guild_id = ctx.guild_id().ok_or(AppError::NotInGuild)?.get();

    // Create a handler for this context type
    let handler = PoiseContextualHandler::new();
    
    // Demonstrate the handler usage
    handler
        .send_message_with_context(&ctx, "Would stop playing and clear the queue")
        .await
        .map_err(|e| AppError::MessageSendError(e.to_string()))?;

    // Log command end
    log_command_end_v2(ctx);
    Ok(())
}

/// Example function showing how commands would be registered
pub fn get_contextual_commands() -> Vec<poise::Command<Data, AppError>> {
    vec![play(), skip(), stop()]
}