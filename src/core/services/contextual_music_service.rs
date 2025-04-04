use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::core::models::error::AppError;
use crate::core::ports::{
    audio_player::{AudioPlayer, GuildId, TextChannelId},
    audio_provider::{AudioProvider, StreamType},
    contextual_message_handler::{ContextualMessageHandler, ContextualMessageResult},
    state_manager::{QueueItem, StateManager},
};

/// Contextual music service that uses generic context type
/// This is a template showing how the service would be structured
#[derive(Debug)]
pub struct ContextualMusicService<P, A, C, S>
where
    P: AudioProvider,
    A: AudioPlayer,
    C: 'static,
    S: StateManager,
{
    audio_provider: Arc<P>,
    audio_player: Arc<A>,
    state_manager: Arc<S>,
    _context_type: std::marker::PhantomData<C>,
}

impl<P, A, C, S> ContextualMusicService<P, A, C, S>
where
    P: AudioProvider,
    A: AudioPlayer,
    C: 'static,
    S: StateManager,
{
    /// Create a new music service with the given dependencies
    #[must_use]
    pub fn new(
        audio_provider: Arc<P>,
        audio_player: Arc<A>,
        state_manager: Arc<S>,
    ) -> Self {
        Self {
            audio_provider,
            audio_player,
            state_manager,
            _context_type: std::marker::PhantomData,
        }
    }

    /// Play a song in a voice channel, notifying in a text channel
    /// This method accepts a generic context that's passed to the handler
    pub async fn play_song<H>(
        &self,
        ctx: &C, 
        handler: &H,
        guild_id: GuildId,
        voice_channel_id: u64,
        query: &str,
    ) -> Result<(), AppError>
    where
        H: ContextualMessageHandler<C>,
    {
        // Get text channel ID from context
        let text_channel_id = handler.get_channel_id_from_context(ctx)
            .map_err(|e| AppError::MessageSendError(e.to_string()))?;

        // Notify that we're searching
        let message_id = handler
            .send_message_with_context(ctx, &format!("🔍 Searching for: {query}"))
            .await
            .map_err(|e| AppError::MessageSendError(e.to_string()))?;

        // Search for the song
        debug!("Searching for song: {}", query);
        let song_info = match self.audio_provider.search(query).await {
            Ok(info) => {
                // Edit the message to show we found the song
                let _ = handler
                    .edit_message_with_context(
                        ctx,
                        message_id,
                        &format!("Found: **{}** ({})", info.title, info.uploader),
                    )
                    .await;
                info
            }
            Err(e) => {
                // Edit the message to show the error
                let _ = handler
                    .edit_message_with_context(
                        ctx,
                        message_id,
                        &format!("❌ Error searching for {query}: {e}"),
                    )
                    .await;
                return Err(AppError::ProviderError(e.to_string()));
            }
        };

        // Send a rich message with song details
        let _ = handler
            .send_rich_message_with_context(
                ctx,
                Some("Adding to Queue"),
                Some(&song_info.title),
                vec![
                    ("Duration".to_string(), song_info.duration.to_string(), true),
                    ("Uploader".to_string(), song_info.uploader.clone(), true),
                ],
                song_info.thumbnail.as_deref(),
                Some(0x3498DB), // Blue color
            )
            .await;

        // Connect to voice channel if needed
        if !self.audio_player.is_connected(guild_id).await? {
            info!("Connecting to voice channel {}", voice_channel_id);
            self.audio_player.connect(guild_id, voice_channel_id).await?;
        }

        // Get audio stream
        let stream = self
            .audio_provider
            .get_stream(&song_info.source_url, StreamType::Opus)
            .await?;

        // Add to queue
        let queue_item = QueueItem {
            title: song_info.title,
            duration: song_info.duration,
            uploader: song_info.uploader,
            url: song_info.source_url,
            thumbnail: song_info.thumbnail,
        };

        // Add to queue in state manager
        self.state_manager.add_to_queue(guild_id, queue_item).await?;

        // Play the song
        self.audio_player
            .play(guild_id, stream, text_channel_id)
            .await?;

        Ok(())
    }

    /// Skip the current song
    pub async fn skip_song<H>(
        &self,
        ctx: &C,
        handler: &H,
        guild_id: GuildId,
    ) -> Result<(), AppError>
    where
        H: ContextualMessageHandler<C>,
    {
        // Skip the current song
        self.audio_player.skip(guild_id).await?;

        // Send confirmation message
        handler
            .send_message_with_context(ctx, "⏭️ Skipped to the next song")
            .await
            .map_err(|e| AppError::MessageSendError(e.to_string()))?;

        Ok(())
    }

    // Other methods would follow similar patterns, passing context to handler
}