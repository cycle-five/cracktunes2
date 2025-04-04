use async_trait::async_trait;
use serenity::all::{
    Context as SerenityContext, CreateEmbed, CreateMessage, EditMessage, GuildId as SerenityGuildId,
    Http, MessageId, ReactionType,
};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use crate::core::ports::{
    audio_player::{GuildId as AppGuildId, TextChannelId as AppTextChannelId},
    contextual_message_handler::{ContextualMessageError, ContextualMessageHandler, ContextualMessageResult},
};

/// Implementation of `ContextualMessageHandler` for Serenity
#[derive(Debug)]
pub struct SerenityContextualHandler {
    // We need HTTP for operations that don't use context
    http: Arc<Http>,
    // Store guild notification channels
    notification_channels: RwLock<HashMap<u64, u64>>,
}

impl SerenityContextualHandler {
    /// Create a new Serenity-based contextual message handler
    #[must_use]
    pub fn new(http: Arc<Http>) -> Self {
        Self {
            http,
            notification_channels: RwLock::new(HashMap::new()),
        }
    }

    /// Convert our AppGuildId to Serenity's GuildId
    fn to_serenity_guild_id(guild_id: AppGuildId) -> SerenityGuildId {
        SerenityGuildId::new(guild_id.0)
    }
}

#[async_trait]
impl ContextualMessageHandler<SerenityContext> for SerenityContextualHandler {
    async fn send_message_with_context(
        &self,
        ctx: &SerenityContext,
        content: &str,
    ) -> ContextualMessageResult<u64> {
        // Get the channel ID from the context (requires additional work in Serenity)
        // This is a simplification - in a real implementation you would need to get the channel ID somehow
        // from the context, which might require passing it separately
        let channel_id = self.get_channel_id_from_context(ctx)?;
        
        // Send the message
        let message = ctx
            .http
            .get_channel(channel_id.0)
            .await
            .map_err(|e| ContextualMessageError::ChannelNotFound(e.to_string()))?
            .id()
            .create_message(&ctx.http, CreateMessage::new().content(content))
            .await
            .map_err(|e| ContextualMessageError::SendError(e.to_string()))?;
            
        Ok(message.id.get())
    }

    async fn send_rich_message_with_context<'a>(
        &self,
        ctx: &SerenityContext,
        title: Option<&'a str>,
        description: Option<&'a str>,
        fields: Vec<(String, String, bool)>,
        thumbnail: Option<&'a str>,
        color: Option<u32>,
    ) -> ContextualMessageResult<u64> {
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

        // Get the channel ID from the context (requires additional work in Serenity)
        let channel_id = self.get_channel_id_from_context(ctx)?;
        
        // Send the message with the embed
        let message = ctx
            .http
            .get_channel(channel_id.0)
            .await
            .map_err(|e| ContextualMessageError::ChannelNotFound(e.to_string()))?
            .id()
            .create_message(&ctx.http, CreateMessage::new().embed(embed))
            .await
            .map_err(|e| ContextualMessageError::SendError(e.to_string()))?;
            
        Ok(message.id.get())
    }

    async fn edit_message_with_context(
        &self,
        ctx: &SerenityContext,
        message_id: u64,
        new_content: &str,
    ) -> ContextualMessageResult<()> {
        // Get the channel ID from the context
        let channel_id = self.get_channel_id_from_context(ctx)?;
        
        // Convert to Serenity IDs
        let serenity_message_id = MessageId::new(message_id);

        // Edit the message
        ctx
            .http
            .get_channel(channel_id.0)
            .await
            .map_err(|e| ContextualMessageError::ChannelNotFound(e.to_string()))?
            .id()
            .edit_message(
                &ctx.http,
                serenity_message_id,
                EditMessage::new().content(new_content),
            )
            .await
            .map_err(|e| ContextualMessageError::EditError(e.to_string()))?;
            
        Ok(())
    }

    async fn add_reaction_with_context(
        &self,
        ctx: &SerenityContext,
        message_id: u64,
        emoji: &str,
    ) -> ContextualMessageResult<()> {
        // Get the channel ID from the context
        let channel_id = self.get_channel_id_from_context(ctx)?;
        
        // Convert to Serenity message ID
        let serenity_message_id = MessageId::new(message_id);

        // Parse the emoji
        let reaction_type = emoji
            .parse::<ReactionType>()
            .map_err(|_| ContextualMessageError::ReactionError("Invalid emoji".to_string()))?;

        // Add the reaction
        ctx
            .http
            .get_channel(channel_id.0)
            .await
            .map_err(|e| ContextualMessageError::ChannelNotFound(e.to_string()))?
            .id()
            .create_reaction(&ctx.http, serenity_message_id, reaction_type)
            .await
            .map_err(|e| ContextualMessageError::ReactionError(e.to_string()))?;
            
        Ok(())
    }

    async fn delete_message_with_context(
        &self,
        ctx: &SerenityContext,
        message_id: u64,
    ) -> ContextualMessageResult<()> {
        // Get the channel ID from the context
        let channel_id = self.get_channel_id_from_context(ctx)?;
        
        // Convert to Serenity message ID
        let serenity_message_id = MessageId::new(message_id);

        // Delete the message
        ctx
            .http
            .get_channel(channel_id.0)
            .await
            .map_err(|e| ContextualMessageError::ChannelNotFound(e.to_string()))?
            .id()
            .delete_message(&ctx.http, serenity_message_id, None)
            .await
            .map_err(|e| ContextualMessageError::DeleteError(e.to_string()))?;
            
        Ok(())
    }

    async fn get_notification_channel(&self, guild_id: AppGuildId) -> Option<AppTextChannelId> {
        let notifications = self.notification_channels.read().await;
        notifications
            .get(&guild_id.0)
            .map(|channel_id| AppTextChannelId(*channel_id))
    }

    async fn set_notification_channel(
        &self,
        guild_id: AppGuildId,
        channel_id: AppTextChannelId,
    ) -> ContextualMessageResult<()> {
        let mut notifications = self.notification_channels.write().await;
        notifications.insert(guild_id.0, channel_id.0);
        debug!("Set notification channel {} for guild {}", channel_id.0, guild_id.0);
        Ok(())
    }

    fn get_channel_id_from_context(&self, _ctx: &SerenityContext) -> ContextualMessageResult<AppTextChannelId> {
        // This is a limitation of the Serenity context - it doesn't directly store channel ID
        // In a real implementation, you would either pass this separately or extract it from context
        Err(ContextualMessageError::UnsupportedOperation)
    }

    fn get_guild_id_from_context(&self, _ctx: &SerenityContext) -> Option<AppGuildId> {
        // Similar limitation as above
        None
    }
}