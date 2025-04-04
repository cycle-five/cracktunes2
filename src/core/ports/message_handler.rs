use super::audio_player::{GuildId, TextChannelId};
use async_trait::async_trait;

/// Error type for MessageHandler operations
#[derive(Debug, thiserror::Error)]
pub enum MessageHandlerError {
    #[error("Failed to send message: {0}")]
    SendError(String),

    #[error("Channel not found")]
    ChannelNotFound,

    #[error("Message too long")]
    MessageTooLong,

    #[error("Rate limited")]
    RateLimited,

    #[error("Message handler error: {0}")]
    Other(String),
}

/// Result type for MessageHandler operations
pub type MessageHandlerResult<T> = Result<T, MessageHandlerError>;

/// Interface for sending messages to users on the platform
#[async_trait]
pub trait MessageHandler: Send + Sync + 'static {
    /// Send a text message to a channel
    async fn send_message(
        &self,
        channel_id: TextChannelId,
        content: &str,
    ) -> MessageHandlerResult<()>;

    /// Send a message with an embed/rich content
    async fn send_rich_message<'a>(
        &self,
        channel_id: TextChannelId,
        title: Option<&'a str>,
        description: Option<&'a str>,
        fields: Vec<(String, String, bool)>,
        thumbnail: Option<&'a str>,
        color: Option<u32>,
    ) -> MessageHandlerResult<()>;

    /// Edit a previously sent message
    async fn edit_message(
        &self,
        channel_id: TextChannelId,
        message_id: u64,
        new_content: &str,
    ) -> MessageHandlerResult<()>;

    /// React to a message with an emoji
    async fn add_reaction(
        &self,
        channel_id: TextChannelId,
        message_id: u64,
        emoji: &str,
    ) -> MessageHandlerResult<()>;

    /// Delete a message
    async fn delete_message(
        &self,
        channel_id: TextChannelId,
        message_id: u64,
    ) -> MessageHandlerResult<()>;

    /// Defer a response to a message (for interactions/slash commands)
    async fn defer_response(&self, interaction_id: u64) -> MessageHandlerResult<()>;

    /// Get the registered notification channel for a guild, if any
    async fn get_notification_channel(&self, guild_id: GuildId) -> Option<TextChannelId>;

    /// Set the notification channel for a guild
    async fn set_notification_channel(
        &self,
        guild_id: GuildId,
        channel_id: TextChannelId,
    ) -> MessageHandlerResult<()>;
}
