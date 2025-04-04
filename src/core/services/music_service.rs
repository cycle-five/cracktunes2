use crate::core::{
    models::{error::AppError, user::User},
    ports::{
        audio_player::{AudioPlayer, GuildId, TextChannelId, VoiceChannelId},
        audio_provider::{AudioProvider, AudioSearchResult},
        message_handler::MessageHandler,
        state_manager::StateManager,
    },
};
use std::sync::Arc;

/// Service responsible for all music-related functionality
pub struct MusicService<A, P, M, S>
where
    A: AudioProvider,
    P: AudioPlayer,
    M: MessageHandler,
    S: StateManager,
{
    audio_provider: Arc<A>,
    audio_player: Arc<P>,
    message_handler: Arc<M>,
    state_manager: Arc<S>,
}

impl<A, P, M, S> MusicService<A, P, M, S>
where
    A: AudioProvider,
    P: AudioPlayer,
    M: MessageHandler,
    S: StateManager,
{
    pub fn new(
        audio_provider: Arc<A>,
        audio_player: Arc<P>,
        message_handler: Arc<M>,
        state_manager: Arc<S>,
    ) -> Self {
        Self {
            audio_provider,
            audio_player,
            message_handler,
            state_manager,
        }
    }

    /// Join a voice channel
    pub async fn join(
        &self,
        guild_id: GuildId,
        voice_channel_id: VoiceChannelId,
        text_channel_id: TextChannelId,
    ) -> Result<(), AppError> {
        // Join the voice channel
        self.audio_player.join(guild_id, voice_channel_id).await?;

        // Register the text channel for notifications
        self.audio_player
            .register_text_channel(guild_id, text_channel_id)
            .await?;

        // Send confirmation message
        self.message_handler
            .send_message(text_channel_id, &format!("Joined voice channel"))
            .await?;

        // Mark activity for this guild
        self.state_manager.update_last_activity(guild_id).await?;

        Ok(())
    }

    /// Leave a voice channel
    pub async fn leave(
        &self,
        guild_id: GuildId,
        text_channel_id: TextChannelId,
    ) -> Result<(), AppError> {
        // Leave the voice channel
        self.audio_player.leave(guild_id).await?;

        // Send confirmation message
        self.message_handler
            .send_message(text_channel_id, "Left voice channel")
            .await?;

        Ok(())
    }

    /// Play audio from a URL
    pub async fn play_url(
        &self,
        guild_id: GuildId,
        voice_channel_id: Option<VoiceChannelId>,
        text_channel_id: TextChannelId,
        url: &str,
        _requester: &User,
    ) -> Result<(), AppError> {
        // Join voice channel if needed and provided
        if !self.audio_player.is_connected(guild_id).await {
            if let Some(channel_id) = voice_channel_id {
                self.join(guild_id, channel_id, text_channel_id).await?;
            } else {
                return Err(AppError::NotInVoiceChannel);
            }
        }

        // Defer the response (useful for slash commands)
        self.message_handler
            .send_message(text_channel_id, "Loading track...")
            .await?;

        // Get stream from URL
        let mut stream = self.audio_provider.get_stream(url).await?;

        // Add requester information to metadata
        if let Some(ref mut metadata) = stream.metadata {
            metadata.title = metadata
                .title
                .clone()
                .or_else(|| Some("Unknown Track".to_string()));
        }

        // Add to queue
        let position = self.audio_player.queue_track(guild_id, stream).await?;

        // Mark activity
        self.state_manager.update_last_activity(guild_id).await?;

        // Get queue status
        if position == 0 {
            self.message_handler
                .send_message(text_channel_id, "Playing track now")
                .await?;
        } else {
            self.message_handler
                .send_message(
                    text_channel_id,
                    &format!("Added track to queue at position {}", position + 1),
                )
                .await?;
        }

        Ok(())
    }

    /// Play audio from a search query
    pub async fn play_search(
        &self,
        guild_id: GuildId,
        voice_channel_id: Option<VoiceChannelId>,
        text_channel_id: TextChannelId,
        query: &str,
        requester: &User,
    ) -> Result<(), AppError> {
        // Check if the query is a URL
        if query.starts_with("http://") || query.starts_with("https://") {
            return self
                .play_url(
                    guild_id,
                    voice_channel_id,
                    text_channel_id,
                    query,
                    requester,
                )
                .await;
        }

        // Join voice channel if needed and provided
        if !self.audio_player.is_connected(guild_id).await {
            if let Some(channel_id) = voice_channel_id {
                self.join(guild_id, channel_id, text_channel_id).await?;
            } else {
                return Err(AppError::NotInVoiceChannel);
            }
        }

        // Defer response
        self.message_handler
            .send_message(text_channel_id, "Searching...")
            .await?;

        // Search for the query
        let results = self.audio_provider.search(query).await?;
        if results.is_empty() {
            return Err(AppError::AudioProvider(
                crate::core::ports::audio_provider::AudioProviderError::NotFound,
            ));
        }

        // Get stream from the first result
        let stream = self.audio_provider.get_stream(&results[0].url).await?;

        // Add to queue
        let position = self.audio_player.queue_track(guild_id, stream).await?;

        // Mark activity
        self.state_manager.update_last_activity(guild_id).await?;

        // Send confirmation
        let title = &results[0].title;
        if position == 0 {
            self.message_handler
                .send_message(text_channel_id, &format!("Playing: {}", title))
                .await?;
        } else {
            self.message_handler
                .send_message(
                    text_channel_id,
                    &format!("Added to queue at position {}: {}", position + 1, title),
                )
                .await?;
        }

        Ok(())
    }

    /// Skip the current track
    pub async fn skip(
        &self,
        guild_id: GuildId,
        text_channel_id: TextChannelId,
    ) -> Result<(), AppError> {
        self.audio_player.skip(guild_id).await?;

        // Mark activity
        self.state_manager.update_last_activity(guild_id).await?;

        self.message_handler
            .send_message(text_channel_id, "Skipped to next track")
            .await?;

        Ok(())
    }

    /// Stop playback and clear the queue
    pub async fn stop(
        &self,
        guild_id: GuildId,
        text_channel_id: TextChannelId,
    ) -> Result<(), AppError> {
        self.audio_player.stop(guild_id).await?;

        self.message_handler
            .send_message(text_channel_id, "Stopped playback and cleared queue")
            .await?;

        Ok(())
    }

    /// Pause the current track
    pub async fn pause(
        &self,
        guild_id: GuildId,
        text_channel_id: TextChannelId,
    ) -> Result<(), AppError> {
        self.audio_player.pause(guild_id).await?;

        // Mark activity
        self.state_manager.update_last_activity(guild_id).await?;

        self.message_handler
            .send_message(text_channel_id, "Paused playback")
            .await?;

        Ok(())
    }

    /// Resume playback
    pub async fn resume(
        &self,
        guild_id: GuildId,
        text_channel_id: TextChannelId,
    ) -> Result<(), AppError> {
        self.audio_player.resume(guild_id).await?;

        // Mark activity
        self.state_manager.update_last_activity(guild_id).await?;

        self.message_handler
            .send_message(text_channel_id, "Resumed playback")
            .await?;

        Ok(())
    }

    /// Show the current queue
    pub async fn show_queue(
        &self,
        guild_id: GuildId,
        text_channel_id: TextChannelId,
    ) -> Result<(), AppError> {
        let queue = self.audio_player.get_queue(guild_id).await?;

        if queue.is_empty() {
            self.message_handler
                .send_message(text_channel_id, "The queue is empty")
                .await?;
            return Ok(());
        }

        // Format queue as a rich message
        let current = if let Some(current) = self.audio_player.current_track(guild_id).await? {
            format!(
                "Now Playing: {}",
                current
                    .metadata
                    .title
                    .unwrap_or_else(|| "Unknown".to_string())
            )
        } else {
            "Not playing anything".to_string()
        };

        let queue_items: Vec<(String, String, bool)> = queue
            .iter()
            .enumerate()
            .map(|(i, track)| {
                let title = track
                    .metadata
                    .title
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string());
                let requester = &track.requester_name;
                let duration = track
                    .metadata
                    .duration
                    .clone()
                    .unwrap_or_else(|| "??:??".to_string());

                (
                    format!("{}. {}", i + 1, title),
                    format!("Requested by: {} | Duration: {}", requester, duration),
                    false, // Not inline
                )
            })
            .collect();

        // Send queue as rich message
        self.message_handler
            .send_rich_message(
                text_channel_id,
                Some("Music Queue"),
                Some(&current),
                queue_items,
                None,
                Some(0x3498DB), // Blue color
            )
            .await?;

        Ok(())
    }

    /// Shuffle the queue
    pub async fn shuffle(
        &self,
        guild_id: GuildId,
        text_channel_id: TextChannelId,
    ) -> Result<(), AppError> {
        self.audio_player.shuffle(guild_id).await?;

        // Mark activity
        self.state_manager.update_last_activity(guild_id).await?;

        self.message_handler
            .send_message(text_channel_id, "Queue shuffled!")
            .await?;

        Ok(())
    }

    /// Set the idle timeout for a guild
    pub async fn set_idle_timeout(
        &self,
        guild_id: GuildId,
        text_channel_id: TextChannelId,
        minutes: Option<u64>,
    ) -> Result<(), AppError> {
        let timeout = minutes.map(|m| std::time::Duration::from_secs(m * 60));
        self.state_manager
            .set_idle_timeout(guild_id, timeout)
            .await?;

        match minutes {
            Some(mins) => {
                self.message_handler
                    .send_message(
                        text_channel_id,
                        &format!("Idle timeout set to {} minutes", mins),
                    )
                    .await?;
            }
            None => {
                self.message_handler
                    .send_message(text_channel_id, "Idle timeout disabled")
                    .await?;
            }
        }

        Ok(())
    }

    /// Get autocomplete suggestions for a search query
    pub async fn get_search_suggestions(
        &self,
        partial_query: &str,
    ) -> Result<Vec<AudioSearchResult>, AppError> {
        // If the query is too short, return an empty list
        if partial_query.len() < 3 {
            return Ok(Vec::new());
        }

        // Convert string suggestions to search results
        let suggestions = self.audio_provider.get_suggestions(partial_query).await?;

        // For autocomplete, we'll create simplified search results
        // A full implementation would search for each suggestion to get complete results
        let results = suggestions
            .into_iter()
            .map(|suggestion| AudioSearchResult {
                title: suggestion.clone(),
                url: suggestion, // Use suggestion as URL placeholder
                duration: None,
                artist: None,
                thumbnail_url: None,
            })
            .collect();

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ports::audio_player::{AudioPlayer, QueuedTrack};
    use crate::core::ports::audio_provider::AudioStream;
    use async_trait::async_trait;
    use mockall::mock;
    use mockall::predicate::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    // Mock the AudioProvider
    mock! {
        pub AudioProviderMock {}

        #[async_trait]
        impl AudioProvider for AudioProviderMock {
            async fn search(&self, query: &str) -> Result<Vec<AudioSearchResult>, crate::core::ports::audio_provider::AudioProviderError>;
            async fn get_suggestions(&self, partial_query: &str) -> Result<Vec<String>, crate::core::ports::audio_provider::AudioProviderError>;
            async fn get_stream(&self, url: &str) -> Result<AudioStream, crate::core::ports::audio_provider::AudioProviderError>;
            fn can_handle_url(&self, url: &str) -> bool;
        }
    }

    // Mock the AudioPlayer
    mock! {
        pub AudioPlayerMock {}

        #[async_trait]
        impl AudioPlayer for AudioPlayerMock {
            async fn join(&self, guild_id: GuildId, channel_id: VoiceChannelId) -> Result<(), crate::core::ports::audio_player::AudioPlayerError>;
            async fn leave(&self, guild_id: GuildId) -> Result<(), crate::core::ports::audio_player::AudioPlayerError>;
            async fn is_connected(&self, guild_id: GuildId) -> bool;
            async fn play(&self, guild_id: GuildId, stream: AudioStream) -> Result<(), crate::core::ports::audio_player::AudioPlayerError>;
            async fn queue_track(&self, guild_id: GuildId, stream: AudioStream) -> Result<usize, crate::core::ports::audio_player::AudioPlayerError>;
            async fn skip(&self, guild_id: GuildId) -> Result<(), crate::core::ports::audio_player::AudioPlayerError>;
            async fn pause(&self, guild_id: GuildId) -> Result<(), crate::core::ports::audio_player::AudioPlayerError>;
            async fn resume(&self, guild_id: GuildId) -> Result<(), crate::core::ports::audio_player::AudioPlayerError>;
            async fn stop(&self, guild_id: GuildId) -> Result<(), crate::core::ports::audio_player::AudioPlayerError>;
            async fn get_queue(&self, guild_id: GuildId) -> Result<Vec<QueuedTrack>, crate::core::ports::audio_player::AudioPlayerError>;
            async fn set_volume(&self, guild_id: GuildId, volume: f32) -> Result<(), crate::core::ports::audio_player::AudioPlayerError>;
            async fn get_volume(&self, guild_id: GuildId) -> Result<f32, crate::core::ports::audio_player::AudioPlayerError>;
            async fn mute(&self, guild_id: GuildId) -> Result<(), crate::core::ports::audio_player::AudioPlayerError>;
            async fn unmute(&self, guild_id: GuildId) -> Result<(), crate::core::ports::audio_player::AudioPlayerError>;
            async fn shuffle(&self, guild_id: GuildId) -> Result<(), crate::core::ports::audio_player::AudioPlayerError>;
            async fn current_track(&self, guild_id: GuildId) -> Result<Option<QueuedTrack>, crate::core::ports::audio_player::AudioPlayerError>;
            async fn register_text_channel(&self, guild_id: GuildId, channel_id: TextChannelId) -> Result<(), crate::core::ports::audio_player::AudioPlayerError>;
        }
    }

    // Mock the MessageHandler
    mock! {
        pub MessageHandlerMock {}

        #[async_trait]
        impl MessageHandler for MessageHandlerMock {
            async fn send_message(&self, channel_id: TextChannelId, content: &str) -> Result<(), crate::core::ports::message_handler::MessageHandlerError>;
            async fn send_rich_message<'a>(
                &self,
                channel_id: TextChannelId,
                title: Option<&'a str>,
                description: Option<&'a str>,
                fields: Vec<(String, String, bool)>,
                thumbnail: Option<&'a str>,
                color: Option<u32>,
            ) -> Result<(), crate::core::ports::message_handler::MessageHandlerError>;
            async fn edit_message(&self, channel_id: TextChannelId, message_id: u64, new_content: &str) -> Result<(), crate::core::ports::message_handler::MessageHandlerError>;
            async fn add_reaction(&self, channel_id: TextChannelId, message_id: u64, emoji: &str) -> Result<(), crate::core::ports::message_handler::MessageHandlerError>;
            async fn delete_message(&self, channel_id: TextChannelId, message_id: u64) -> Result<(), crate::core::ports::message_handler::MessageHandlerError>;
            async fn defer_response(&self, interaction_id: u64) -> Result<(), crate::core::ports::message_handler::MessageHandlerError>;
            async fn get_notification_channel(&self, guild_id: GuildId) -> Option<TextChannelId>;
            async fn set_notification_channel(&self, guild_id: GuildId, channel_id: TextChannelId) -> Result<(), crate::core::ports::message_handler::MessageHandlerError>;
        }
    }

    // Simple in-memory implementation of StateManager for tests
    struct InMemoryStateManager {
        data: Mutex<HashMap<String, String>>,
    }

    impl InMemoryStateManager {
        fn new() -> Self {
            Self {
                data: Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl StateManager for InMemoryStateManager {
        async fn set<T: serde::Serialize + Send + Sync + 'static>(
            &self,
            key: &str,
            value: &T,
        ) -> Result<(), crate::core::ports::state_manager::StateManagerError> {
            let json = serde_json::to_string(value).map_err(|e| {
                crate::core::ports::state_manager::StateManagerError::StoreError(e.to_string())
            })?;

            let mut data = self.data.lock().unwrap();
            data.insert(key.to_string(), json);

            Ok(())
        }

        async fn get<T: serde::de::DeserializeOwned + Send + Sync + 'static>(
            &self,
            key: &str,
        ) -> Result<Option<T>, crate::core::ports::state_manager::StateManagerError> {
            let data = self.data.lock().unwrap();

            if let Some(json) = data.get(key) {
                let value = serde_json::from_str(json).map_err(|e| {
                    crate::core::ports::state_manager::StateManagerError::RetrieveError(
                        e.to_string(),
                    )
                })?;
                Ok(Some(value))
            } else {
                Ok(None)
            }
        }

        async fn delete(
            &self,
            key: &str,
        ) -> Result<(), crate::core::ports::state_manager::StateManagerError> {
            let mut data = self.data.lock().unwrap();
            data.remove(key);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_play_search_success() {
        // Setup mocks
        let mut audio_provider = MockAudioProviderMock::new();
        let mut audio_player = MockAudioPlayerMock::new();
        let mut message_handler = MockMessageHandlerMock::new();
        let state_manager = InMemoryStateManager::new();

        // Configure mock expectations
        audio_provider
            .expect_search()
            .with(eq("test query"))
            .returning(|_| {
                Ok(vec![AudioSearchResult {
                    title: "Test Song".to_string(),
                    url: "https://example.com/song".to_string(),
                    duration: Some("3:00".to_string()),
                    artist: Some("Test Artist".to_string()),
                    thumbnail_url: None,
                }])
            });

        audio_provider
            .expect_get_stream()
            .with(eq("https://example.com/song"))
            .returning(|_| {
                Ok(AudioStream {
                    metadata: Some(crate::core::ports::audio_provider::AudioMetadata {
                        title: Some("Test Song".to_string()),
                        artist: Some("Test Artist".to_string()),
                        album: None,
                        duration: Some("3:00".to_string()),
                        source_url: Some("https://example.com/song".to_string()),
                        thumbnail_url: None,
                    }),
                    provider_data: Arc::new(()),
                })
            });

        audio_player.expect_is_connected().returning(|_| true);

        audio_player.expect_queue_track().returning(|_, _| Ok(0));

        message_handler
            .expect_send_message()
            .returning(|_, _| Ok(()));

        // Create service
        let service = MusicService::new(
            Arc::new(audio_provider),
            Arc::new(audio_player),
            Arc::new(message_handler),
            Arc::new(state_manager),
        );

        // Test
        let guild_id = GuildId(123);
        let text_channel_id = TextChannelId(456);
        let requester = User::new("user123", "TestUser");

        let result = service
            .play_search(guild_id, None, text_channel_id, "test query", &requester)
            .await;

        assert!(result.is_ok());
    }
}
