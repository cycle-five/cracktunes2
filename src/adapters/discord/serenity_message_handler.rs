use async_trait::async_trait;
use serenity::all::{ChannelId, CreateEmbed, CreateMessage, EditMessage, Http, MessageId};
use std::{fmt::Debug, sync::Arc};

use crate::core::ports::{
    audio_player::GuildId as AppGuildId,
    audio_player::TextChannelId as AppTextChannelId,
    message_handler::{MessageHandler, MessageHandlerError, MessageHandlerResult},
};

/// Implementation of MessageHandler using Serenity
pub struct SerenityMessageHandler {
    http: Arc<Http>,
    interaction_tokens: dashmap::DashMap<u64, String>, // Store interaction tokens for deferred responses
}

impl Debug for SerenityMessageHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SerenityMessageHandler")
            .field("http", &self.http)
            .field("interaction_tokens", &self.interaction_tokens)
            .finish()
    }
}

impl SerenityMessageHandler {
    /// Create a new Serenity-based message handler
    pub fn new(http: Arc<Http>) -> Self {
        Self {
            http,
            interaction_tokens: dashmap::DashMap::new(),
        }
    }
}

#[async_trait]
impl MessageHandler for SerenityMessageHandler {
    async fn send_message(
        &self,
        channel_id: AppTextChannelId,
        content: &str,
    ) -> MessageHandlerResult<()> {
        // Convert to Serenity channel ID
        let serenity_channel_id = ChannelId::new(channel_id.0);

        // Send the message
        serenity_channel_id
            .say(&self.http, content)
            .await
            .map(|_| ())
            .map_err(|e| MessageHandlerError::SendError(e.to_string()))
    }

    async fn send_rich_message<'a>(
        &self,
        channel_id: AppTextChannelId,
        title: Option<&'a str>,
        description: Option<&'a str>,
        fields: Vec<(String, String, bool)>,
        thumbnail: Option<&'a str>,
        color: Option<u32>,
    ) -> MessageHandlerResult<()> {
        // Convert to Serenity channel ID
        let serenity_channel_id = ChannelId::from(channel_id.0);

        // Create the embed
        let mut embed = CreateEmbed::new();

        if let Some(title) = title {
            embed = embed.title(title);
        }

        if let Some(description) = description {
            embed = embed.description(description);
        }

        if let Some(thumbnail_url) = thumbnail {
            embed = embed.thumbnail(thumbnail_url);
        }

        if let Some(color_code) = color {
            embed = embed.color(color_code);
        }

        // Add fields
        for (name, value, inline) in fields {
            embed = embed.field(name, value, inline);
        }

        // Send the message with the embed
        let builder = CreateMessage::new().embed(embed);
        serenity_channel_id
            .send_message(&self.http, builder)
            .await
            .map(|_| ())
            .map_err(|e| MessageHandlerError::SendError(e.to_string()))
    }

    async fn edit_message(
        &self,
        channel_id: AppTextChannelId,
        message_id: u64,
        new_content: &str,
    ) -> MessageHandlerResult<()> {
        // Convert to Serenity IDs
        let serenity_channel_id = ChannelId::from(channel_id);
        let serenity_message_id = serenity::all::MessageId::new(message_id);

        // Edit the message
        let builder = EditMessage::new().content(new_content);
        serenity_channel_id
            .edit_message(&self.http, serenity_message_id, builder)
            .await
            .map(|_| ())
            .map_err(|e| MessageHandlerError::SendError(e.to_string()))
    }

    async fn add_reaction(
        &self,
        channel_id: AppTextChannelId,
        message_id: u64,
        emoji: &str,
    ) -> MessageHandlerResult<()> {
        // Convert to Serenity IDs
        let serenity_channel_id = ChannelId::from(channel_id);
        let serenity_message_id = MessageId::new(message_id);

        let into_emoji = emoji
            .parse::<serenity::all::ReactionType>()
            .map_err(|_| MessageHandlerError::SendError("Invalid emoji".to_string()))?;

        // Add the reaction
        serenity_channel_id
            .create_reaction(&self.http, serenity_message_id, into_emoji)
            .await
            .map_err(|e| MessageHandlerError::SendError(e.to_string()))
    }

    async fn delete_message(
        &self,
        channel_id: AppTextChannelId,
        message_id: u64,
    ) -> MessageHandlerResult<()> {
        // Convert to Serenity IDs
        let serenity_channel_id = ChannelId::from(channel_id);
        let serenity_message_id = MessageId::new(message_id);

        // Delete the message
        serenity_channel_id
            .delete_message(&self.http, serenity_message_id, None)
            .await
            .map_err(|e| MessageHandlerError::SendError(e.to_string()))
    }

    async fn defer_response(&self, interaction_id: u64) -> MessageHandlerResult<()> {
        // For poise this is generally handled automatically, but we'll keep the function
        // in case we need to use it directly with Serenity later
        tracing::warn!("defer_response not implemented {interaction_id:?}");
        Ok(())
    }

    async fn get_notification_channel(&self, guild_id: AppGuildId) -> Option<AppTextChannelId> {
        // In a real implementation, this would fetch from a database
        // For now, we'll just return None
        tracing::warn!("get_notification_channel not implemented {guild_id:?}");
        None
    }

    async fn set_notification_channel(
        &self,
        guild_id: AppGuildId,
        channel_id: AppTextChannelId,
    ) -> MessageHandlerResult<()> {
        // In a real implementation, this would save to a database
        // For now, we'll just return success
        tracing::warn!("set_notification_channel not implemented {guild_id:?} {channel_id:?}");
        Ok(())
    }
}
