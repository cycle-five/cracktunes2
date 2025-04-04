use async_trait::async_trait;
use std::fmt::Debug;

use crate::core::ports::audio_player::{GuildId, TextChannelId};

/// Type alias for results from the ContextualMessageHandler
pub type ContextualMessageResult<T> = Result<T, ContextualMessageError>;

/// Errors that can occur when handling messages
#[derive(Debug, thiserror::Error)]
pub enum ContextualMessageError {
    #[error("Failed to send message: {0}")]
    SendError(String),

    #[error("Failed to edit message: {0}")]
    EditError(String),

    #[error("Failed to add reaction: {0}")]
    ReactionError(String),

    #[error("Failed to delete message: {0}")]
    DeleteError(String),

    #[error("No notification channel set for guild")]
    NoNotificationChannel,

    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("Operation not supported for this context type")]
    UnsupportedOperation,
}

/// Interface for sending messages to users on the platform with a specific context type
/// This generic approach allows direct usage of framework-specific context objects
#[async_trait]
pub trait ContextualMessageHandler<C>: Send + Sync + Debug + 'static {
    /// Send a text message to a channel using the provided context
    async fn send_message_with_context(
        &self,
        ctx: &C,
        content: &str,
    ) -> ContextualMessageResult<u64>; // Returns message ID

    /// Send a rich message (with embeds) to a channel using the provided context
    async fn send_rich_message_with_context<'a>(
        &self,
        ctx: &C,
        title: Option<&'a str>,
        description: Option<&'a str>,
        fields: Vec<(String, String, bool)>,
        thumbnail: Option<&'a str>,
        color: Option<u32>,
    ) -> ContextualMessageResult<u64>; // Returns message ID

    /// Edit a message using the provided context
    async fn edit_message_with_context(
        &self,
        ctx: &C,
        message_id: u64,
        new_content: &str,
    ) -> ContextualMessageResult<()>;

    /// Add a reaction to a message using the provided context
    async fn add_reaction_with_context(
        &self,
        ctx: &C,
        message_id: u64,
        emoji: &str,
    ) -> ContextualMessageResult<()>;

    /// Delete a message using the provided context
    async fn delete_message_with_context(
        &self,
        ctx: &C,
        message_id: u64,
    ) -> ContextualMessageResult<()>;

    /// Get the notification channel for a guild
    async fn get_notification_channel(&self, guild_id: GuildId) -> Option<TextChannelId>;

    /// Set the notification channel for a guild
    async fn set_notification_channel(
        &self,
        guild_id: GuildId,
        channel_id: TextChannelId,
    ) -> ContextualMessageResult<()>;

    /// Get the text channel ID from the context
    fn get_channel_id_from_context(&self, ctx: &C) -> ContextualMessageResult<TextChannelId>;

    /// Get the guild ID from the context if available
    fn get_guild_id_from_context(&self, ctx: &C) -> Option<GuildId>;
}