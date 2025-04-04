use crate::core::ports::{
    audio_player::AudioPlayerError, audio_provider::AudioProviderError,
    message_handler::MessageHandlerError, state_manager::StateManagerError,
};

/// Primary application error type
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("User not found")]
    UserNotFound,

    #[error("Not in a guild")]
    NotInGuild,

    #[error("Not in a voice channel")]
    NotInVoiceChannel,

    #[error("No permission")]
    NoPermission,

    #[error("Audio provider error: {0}")]
    AudioProvider(#[from] AudioProviderError),

    #[error("Audio player error: {0}")]
    AudioPlayer(#[from] AudioPlayerError),

    #[error("Message handler error: {0}")]
    MessageHandler(#[from] MessageHandlerError),

    #[error("State manager error: {0}")]
    StateManager(#[from] StateManagerError),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Command execution error: {0}")]
    CommandExecution(String),

    #[error("Application error: {0}")]
    Other(String),
}

impl From<crack_types::CrackedError> for AppError {
    fn from(err: crack_types::CrackedError) -> Self {
        AppError::Other(err.to_string())
    }
}

impl From<serenity::Error> for AppError {
    fn from(err: serenity::Error) -> Self {
        AppError::Other(err.to_string())
    }
}
