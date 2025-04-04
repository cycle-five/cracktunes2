use crate::core::models::error::AppError;
use serenity::all::{AutocompleteChoice, CreateAutocompleteResponse};

use crate::commands::v2::utils::{
    get_guild_id, get_text_channel_id, get_user, get_user_voice_channel,
};
// This import would be needed for the full implementation
use crate::infrastructure::setup::Data;

/// Autocomplete handler for search queries
async fn autocomplete<'a>(
    ctx: poise::Context<'_, Data, AppError>,
    partial: &'a str,
) -> CreateAutocompleteResponse<'a> {
    // Get suggestions from the music service
    let choices = match ctx
        .data()
        .services
        .music_service
        .get_search_suggestions(partial)
        .await
    {
        Ok(suggestions) => suggestions
            .into_iter()
            .map(|suggestion| {
                AutocompleteChoice::new(suggestion.title.clone(), suggestion.url.clone())
            })
            .collect(),
        Err(e) => {
            tracing::error!("Error getting suggestions: {}", e);
            vec![]
        }
    };

    CreateAutocompleteResponse::new().set_choices(choices)
}

/// Play music from a URL or search query
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn play(
    ctx: poise::Context<'_, Data, AppError>,
    #[description = "URL or search term"]
    #[autocomplete = "autocomplete"]
    query: String,
) -> Result<(), AppError> {
    // Defer the response to prevent timeout
    ctx.defer().await?;

    // Get required IDs
    let guild_id = get_guild_id(ctx)?;
    let text_channel_id = get_text_channel_id(ctx);
    let voice_channel_id = get_user_voice_channel(ctx).await.ok(); // Optional
    let user = get_user(ctx);

    // Use the music service to handle playback
    ctx.data()
        .services
        .music_service
        .play_search(guild_id, voice_channel_id, text_channel_id, &query, &user)
        .await?;

    Ok(())
}

/// Skip the current track
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn skip(ctx: poise::Context<'_, Data, AppError>) -> Result<(), AppError> {
    // Get required IDs
    let guild_id = get_guild_id(ctx)?;
    let text_channel_id = get_text_channel_id(ctx);

    // Skip using the music service
    ctx.data()
        .services
        .music_service
        .skip(guild_id, text_channel_id)
        .await?;

    Ok(())
}

/// Stop playback and clear the queue
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn stop(ctx: poise::Context<'_, Data, AppError>) -> Result<(), AppError> {
    // Get required IDs
    let guild_id = get_guild_id(ctx)?;
    let text_channel_id = get_text_channel_id(ctx);

    // Stop using the music service
    ctx.data()
        .services
        .music_service
        .stop(guild_id, text_channel_id)
        .await?;

    Ok(())
}

/// Pause playback
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn pause(ctx: poise::Context<'_, Data, AppError>) -> Result<(), AppError> {
    // Get required IDs
    let guild_id = get_guild_id(ctx)?;
    let text_channel_id = get_text_channel_id(ctx);

    // Pause using the music service
    ctx.data()
        .services
        .music_service
        .pause(guild_id, text_channel_id)
        .await?;

    Ok(())
}

/// Resume playback
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn resume(ctx: poise::Context<'_, Data, AppError>) -> Result<(), AppError> {
    // Get required IDs
    let guild_id = get_guild_id(ctx)?;
    let text_channel_id = get_text_channel_id(ctx);

    // Resume using the music service
    ctx.data()
        .services
        .music_service
        .resume(guild_id, text_channel_id)
        .await?;

    Ok(())
}

/// Show the current queue
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn queue(ctx: poise::Context<'_, Data, AppError>) -> Result<(), AppError> {
    // Get required IDs
    let guild_id = get_guild_id(ctx)?;
    let text_channel_id = get_text_channel_id(ctx);

    // Show queue using the music service
    ctx.data()
        .services
        .music_service
        .show_queue(guild_id, text_channel_id)
        .await?;

    Ok(())
}

/// Shuffle the queue
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn shuffle(ctx: poise::Context<'_, Data, AppError>) -> Result<(), AppError> {
    // Get required IDs
    let guild_id = get_guild_id(ctx)?;
    let text_channel_id = get_text_channel_id(ctx);

    // Shuffle using the music service
    ctx.data()
        .services
        .music_service
        .shuffle(guild_id, text_channel_id)
        .await?;

    Ok(())
}

/// Returns all music-related commands
pub fn get_music_commands() -> Vec<poise::Command<Data, AppError>> {
    vec![
        play(),
        skip(),
        stop(),
        pause(),
        resume(),
        queue(),
        shuffle(),
    ]
}
