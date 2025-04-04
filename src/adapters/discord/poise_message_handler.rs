use async_trait::async_trait;
use poise::{CreateReply, serenity_prelude as serenity};
use serenity::all::{CreateEmbed, GuildId as SerenityGuildId, Http, MessageId};
use poise::serenity_prelude::GuildId;
use std::fmt::Debug;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::debug;

use crate::core::ports::{
    audio_player::GuildId as AppGuildId,
    audio_player::TextChannelId as AppTextChannelId,
    message_handler::{MessageHandler, MessageHandlerError, MessageHandlerResult},
};
use crate::infrastructure::setup::Data;

// Type alias for our application's context type
type PoiseContext<'a> = poise::Context<'a, Data, crate::core::models::error::AppError>;

/// Implementation of `MessageHandler` using Poise context
pub struct PoiseMessageHandler {
    // We need to keep track of active contexts to reuse them
    active_contexts: RwLock<HashMap<u64, PoiseContextWrapper>>,
    // Fallback HTTP client for when no context is available
    http: Arc<Http>,
    // Store guild notification channels
    notification_channels: RwLock<HashMap<u64, u64>>,
}

/// Wrapper for Poise context that can be stored and shared
#[allow(clippy::struct_field_names)]
struct PoiseContextWrapper {
    guild_id: Option<u64>,
    channel_id: u64,
    // We'll store message IDs instead of ReplyHandles to avoid lifetime issues
    last_message_id: Option<u64>,
}

impl Debug for PoiseContextWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PoiseContextWrapper")
            .field("guild_id", &self.guild_id)
            .field("channel_id", &self.channel_id)
            .field("has_message", &self.last_message_id.is_some())
            .finish()
    }
}

impl Debug for PoiseMessageHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PoiseMessageHandler")
            .field("active_contexts", &"RwLock<HashMap<...>>")
            .field("http", &"Arc<Http>")
            .finish()
    }
}

impl PoiseMessageHandler {
    /// Create a new Poise-based message handler
    pub fn new(http: Arc<Http>) -> Self {
        Self {
            active_contexts: RwLock::new(HashMap::new()),
            http,
            notification_channels: RwLock::new(HashMap::new()),
        }
    }
}

impl From<Arc<Http>> for PoiseMessageHandler {
    fn from(http: Arc<Http>) -> Self {
        Self::new(http)
    }
}

impl PoiseMessageHandler {
    /// Register a Poise context for future use
    pub async fn register_context(&self, ctx: &PoiseContext<'_>) {
        let channel_id = ctx.channel_id().get();
        let guild_id = ctx.guild_id().map(GuildId::get);
        
        debug!("Registering Poise context for channel {}", channel_id);
        
        let wrapper = PoiseContextWrapper {
            guild_id,
            channel_id,
            last_message_id: None,
        };
        
        let mut contexts = self.active_contexts.write().await;
        contexts.insert(channel_id, wrapper);
    }
    
    /// Register context with just the IDs (useful for hooks where you can't clone the context)
    pub async fn register_channel(&self, channel_id: u64, guild_id: Option<u64>) {
        debug!("Registering channel {} for future Poise interactions", channel_id);
        
        let wrapper = PoiseContextWrapper {
            guild_id,
            channel_id,
            last_message_id: None,
        };
        
        let mut contexts = self.active_contexts.write().await;
        contexts.insert(channel_id, wrapper);
    }
    
    // For future implementation, these utility methods will be useful
    
    /// Create a `PoiseContext` reply builder from parameters
    #[must_use]
    #[allow(dead_code)] // Will be used in future implementation
    pub fn create_reply<'a>(
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
    
    /// Convert our `AppGuildId` to Serenity's `GuildId`
    #[allow(dead_code)] // Will be used in future implementation
    fn to_serenity_guild_id(guild_id: AppGuildId) -> SerenityGuildId {
        SerenityGuildId::new(guild_id.0)
    }
}

#[async_trait]
impl MessageHandler for PoiseMessageHandler {
    async fn send_message(
        &self,
        channel_id: AppTextChannelId,
        content: &str,
    ) -> MessageHandlerResult<()> {
        // Check if we have an active context for this channel
        let mut contexts = self.active_contexts.write().await;
        
        if let Some(_ctx_wrapper) = contexts.get_mut(&channel_id.0) {
            // We have a context, update the last reply
            // In a real implementation, we would need to find a way to actually use the ctx here
            // For now we'll use the HTTP client directly but track the message ID
            
            // Use direct HTTP client for now
            let serenity_channel_id = serenity::ChannelId::new(channel_id.0);
            let msg = serenity_channel_id
                .say(&self.http, content)
                .await
                .map_err(|e| MessageHandlerError::SendError(e.to_string()))?;
                
            debug!("Sent message to channel {} with ID {}", channel_id.0, msg.id);
            
            // In a real implementation, this would use the Poise context
            // ctx_wrapper.last_reply = Some(ctx.say(content).await?);
            
            Ok(())
        } else {
            // No context, use HTTP client directly
            debug!("No Poise context available, using HTTP client for channel {}", channel_id.0);
            let serenity_channel_id = serenity::ChannelId::new(channel_id.0);
            serenity_channel_id
                .say(&self.http, content)
                .await
                .map(|_| ())
                .map_err(|e| MessageHandlerError::SendError(e.to_string()))
        }
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

        // Check if we have an active context for this channel
        let contexts = self.active_contexts.read().await;
        
        if contexts.contains_key(&channel_id.0) {
            // We have a context, but can't use it directly here
            // In a real implementation, we would need to find a way to actually use the ctx here
            debug!("Poise context available but not using it for rich message to channel {}", channel_id.0);
        }
        
        // Use HTTP client directly for now
        let serenity_channel_id = serenity::ChannelId::new(channel_id.0);
        let builder = serenity::CreateMessage::new().embed(embed);
        
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
        let serenity_channel_id = serenity::ChannelId::new(channel_id.0);
        let serenity_message_id = MessageId::new(message_id);

        // Edit the message directly
        let builder = serenity::EditMessage::new().content(new_content);
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
        let serenity_channel_id = serenity::ChannelId::new(channel_id.0);
        let serenity_message_id = MessageId::new(message_id);

        let into_emoji = emoji
            .parse::<serenity::ReactionType>()
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
        let serenity_channel_id = serenity::ChannelId::new(channel_id.0);
        let serenity_message_id = MessageId::new(message_id);

        // Delete the message
        serenity_channel_id
            .delete_message(&self.http, serenity_message_id, None)
            .await
            .map_err(|e| MessageHandlerError::SendError(e.to_string()))
    }

    async fn defer_response(&self, interaction_id: u64) -> MessageHandlerResult<()> {
        debug!("Deferring response for interaction {}", interaction_id);
        // Poise handles this automatically in most cases
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
    ) -> MessageHandlerResult<()> {
        let mut notifications = self.notification_channels.write().await;
        notifications.insert(guild_id.0, channel_id.0);
        debug!("Set notification channel {} for guild {}", channel_id.0, guild_id.0);
        Ok(())
    }
}

