use crate::core::models::error::AppError;
use tracing::{debug, error, warn};

/// Error handler for the Discord bot
pub async fn handle_error(
    error: AppError,
    ctx: &poise::Context<'_, crate::infrastructure::setup::Data, AppError>,
) {
    let error_message = match &error {
        AppError::NotInGuild => "This command can only be used in a server.".to_string(),
        AppError::NotInVoiceChannel => {
            "You need to be in a voice channel to use this command.".to_string()
        }
        AppError::NoPermission => "You don't have permission to use this command.".to_string(),
        AppError::AudioProvider(provider_error) => {
            format!("Audio provider error: {}", provider_error)
        }
        AppError::AudioPlayer(player_error) => format!("Audio player error: {}", player_error),
        AppError::MessageHandler(message_error) => {
            format!("Message handling error: {}", message_error)
        }
        AppError::StateManager(state_error) => format!("State management error: {}", state_error),
        AppError::InvalidArgument(arg) => format!("Invalid argument: {}", arg),
        AppError::CommandExecution(msg) => format!("Command execution error: {}", msg),
        AppError::Other(msg) => format!("Error: {}", msg),
        AppError::UserNotFound => "User not found.".to_string(),
    };

    // Log the error with an appropriate level
    match &error {
        AppError::NotInGuild
        | AppError::NotInVoiceChannel
        | AppError::NoPermission
        | AppError::InvalidArgument(_) => {
            debug!("User error: {}", error);
        }
        AppError::CommandExecution(_) | AppError::UserNotFound => {
            warn!("Command error: {}", error);
        }
        _ => {
            error!("Application error: {}", error);
        }
    }

    // Reply to the user with the error message
    if let Err(e) = ctx.say(error_message).await {
        error!("Failed to send error message: {}", e);
    }
}

/// Convert the error type from core to framework
pub fn convert_error(e: AppError) -> String {
    // In a real implementation, this would convert to poise's FrameworkError
    // But for simplicity, we'll just return a string representation
    format!("Error: {}", e)
}
