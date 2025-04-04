use async_trait::async_trait;
use poise::{serenity_prelude as serenity, CreateReply};
use serenity::all::{CreateEmbed, MessageId};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use crate::core::models::error::AppError;
use crate::core::ports::{
    audio_player::{GuildId as AppGuildId, TextChannelId as AppTextChannelId},
    contextual_message_handler::{ContextualMessageError, ContextualMessageHandler, ContextualMessageResult},
};
use crate::infrastructure::setup::Data;

/// Type alias for our application's context type
pub type PoiseContext<'a> = poise::Context<'a, Data, AppError>;

/// Implementation of `ContextualMessageHandler` for Poise
#[derive(Debug)]
pub struct PoiseContextualHandler {
    // Store guild notification channels
    notification_channels: RwLock<HashMap<u64, u64>>,
}

impl PoiseContextualHandler {
    /// Create a new Poise-based contextual message handler
    #[must_use]
    pub fn new() -> Self {
        Self {
            notification_channels: RwLock::new(HashMap::new()),
        }
    }

    /// Create a Poise reply builder from parameters
    #[must_use]
    fn create_reply<'a>(
        content: &'a str,
        title: Option<&'a str>,
        description: Option<&'a str>,
        fields: &'a [(String, String, bool)],
        thumbnail: Option<&'a str>,
        color: Option<u32>,
    ) -> CreateReply<'a> {
        let mut reply = CreateReply::default();
        
        if title.is_some() || description.is_some() || !fields.is_empty() || thumbnail.is_some() {
            // We have rich content, create an embed
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
                embed = embed.field(name, value, *inline);
            }
            
            reply = reply.embed(embed);
        } else {
            // Just a simple text message
            reply = reply.content(content);
        }
        
        reply
    }
}

impl Default for PoiseContextualHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ContextualMessageHandler<PoiseContext<'_>> for PoiseContextualHandler {
    async fn send_message_with_context(
        &self,
        ctx: &PoiseContext<'_>,
        content: &str,
    ) -> ContextualMessageResult<u64> {
        // Use Poise's context directly to send the message
        let reply = ctx.say(content).await
            .map_err(|e| ContextualMessageError::SendError(e.to_string()))?;
            
        // Return the message ID for future reference
        Ok(reply.message().await
            .map_err(|e| ContextualMessageError::SendError(e.to_string()))?
            .id.get())
    }

    async fn send_rich_message_with_context<'a>(
        &self,
        ctx: &PoiseContext<'_>,
        title: Option<&'a str>,
        description: Option<&'a str>,
        fields: Vec<(String, String, bool)>,
        thumbnail: Option<&'a str>,
        color: Option<u32>,
    ) -> ContextualMessageResult<u64> {
        // Create a rich reply using our helper
        let reply_builder = Self::create_reply(
            "", // No content for rich messages
            title,
            description,
            &fields,
            thumbnail,
            color,
        );
        
        // Send the message using Poise
        let reply = ctx.send(reply_builder).await
            .map_err(|e| ContextualMessageError::SendError(e.to_string()))?;
            
        // Return the message ID
        Ok(reply.message().await
            .map_err(|e| ContextualMessageError::SendError(e.to_string()))?
            .id.get())
    }

    async fn edit_message_with_context(
        &self,
        ctx: &PoiseContext<'_>,
        message_id: u64,
        new_content: &str,
    ) -> ContextualMessageResult<()> {
        // Convert to a Serenity message ID
        let message_id = MessageId::new(message_id);
        
        // Edit the message
        ctx.channel_id()
            .edit_message(&ctx.serenity_context().http, message_id, 
                serenity::EditMessage::new().content(new_content))
            .await
            .map_err(|e| ContextualMessageError::EditError(e.to_string()))?;
            
        Ok(())
    }

    async fn add_reaction_with_context(
        &self,
        ctx: &PoiseContext<'_>,
        message_id: u64,
        emoji: &str,
    ) -> ContextualMessageResult<()> {
        // Parse the emoji
        let reaction_type = emoji
            .parse::<serenity::ReactionType>()
            .map_err(|_| ContextualMessageError::ReactionError("Invalid emoji".to_string()))?;
            
        // Convert to a Serenity message ID
        let message_id = MessageId::new(message_id);
        
        // Add the reaction
        ctx.channel_id()
            .create_reaction(&ctx.serenity_context().http, message_id, reaction_type)
            .await
            .map_err(|e| ContextualMessageError::ReactionError(e.to_string()))?;
            
        Ok(())
    }

    async fn delete_message_with_context(
        &self,
        ctx: &PoiseContext<'_>,
        message_id: u64,
    ) -> ContextualMessageResult<()> {
        // Convert to a Serenity message ID
        let message_id = MessageId::new(message_id);
        
        // Delete the message
        ctx.channel_id()
            .delete_message(&ctx.serenity_context().http, message_id, None)
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

    fn get_channel_id_from_context(&self, ctx: &PoiseContext<'_>) -> ContextualMessageResult<AppTextChannelId> {
        Ok(AppTextChannelId(ctx.channel_id().get()))
    }

    fn get_guild_id_from_context(&self, ctx: &PoiseContext<'_>) -> Option<AppGuildId> {
        ctx.guild_id().map(|id| AppGuildId(id.get()))
    }
}