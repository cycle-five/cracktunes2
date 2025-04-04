use crate::core::{
    models::error::AppError,
    ports::{
        audio_player::{AudioPlayer, GuildId, VoiceChannelId},
        state_manager::StateManager,
    },
};
use std::sync::Arc;
use std::time::Duration;

/// Enum to represent different voice connection states
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// User is in a voice channel
    UserConnected(VoiceChannelId),

    /// Bot is in a voice channel
    BotConnected(VoiceChannelId),

    /// Both user and bot are in the same voice channel
    MutuallyConnected(VoiceChannelId),

    /// User and bot are in different voice channels
    SeparatelyConnected(VoiceChannelId, VoiceChannelId),

    /// Neither user nor bot is connected
    NeitherConnected,
}

/// Service for managing voice connections and channel monitoring
#[derive(Debug)]
pub struct ConnectionService<P, S>
where
    P: AudioPlayer,
    S: StateManager,
{
    audio_player: Arc<P>,
    state_manager: Arc<S>,
}

impl<P, S> ConnectionService<P, S>
where
    P: AudioPlayer,
    S: StateManager,
{
    pub fn new(audio_player: Arc<P>, state_manager: Arc<S>) -> Self {
        Self {
            audio_player,
            state_manager,
        }
    }

    /// Check for idle voice connections and disconnect if timeout is reached
    pub async fn check_idle_timeouts(&self, guild_id: GuildId) -> Result<bool, AppError> {
        // Check if the bot is connected to a voice channel
        if !self.audio_player.is_connected(guild_id).await {
            return Ok(false);
        }

        // Get the idle timeout for this guild
        let idle_timeout = match self.state_manager.get_idle_timeout(guild_id).await? {
            Some(timeout) => timeout,
            None => return Ok(false), // No timeout set, no need to disconnect
        };

        // If timeout is zero, it means "never disconnect"
        if idle_timeout.as_secs() == 0 {
            return Ok(false);
        }

        // Get the last activity timestamp
        let last_activity =
            if let Some(timestamp) = self.state_manager.get_last_activity(guild_id).await? {
                timestamp
            } else {
                // No activity recorded, update it now as a starting point
                self.state_manager.update_last_activity(guild_id).await?;
                return Ok(false);
            };

        // Get current time
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Calculate idle time
        let idle_time = now.saturating_sub(last_activity);

        // Check if idle timeout has been reached
        if Duration::from_secs(idle_time) >= idle_timeout {
            // Timeout reached, disconnect
            self.audio_player.leave(guild_id).await?;
            return Ok(true);
        }

        Ok(false)
    }

    /// Check connection state for a user and the bot
    pub fn check_connection_state(
        &self,
        _guild_id: GuildId,
        user_channel_id: Option<VoiceChannelId>,
        bot_channel_id: Option<VoiceChannelId>,
    ) -> ConnectionState {
        match (user_channel_id, bot_channel_id) {
            (Some(user_id), Some(bot_id)) if user_id == bot_id => {
                ConnectionState::MutuallyConnected(user_id)
            }
            (Some(user_id), Some(bot_id)) => ConnectionState::SeparatelyConnected(user_id, bot_id),
            (None, Some(bot_id)) => ConnectionState::BotConnected(bot_id),
            (Some(user_id), None) => ConnectionState::UserConnected(user_id),
            (None, None) => ConnectionState::NeitherConnected,
        }
    }

    /// Set the idle timeout for a guild
    pub async fn set_idle_timeout(
        &self,
        guild_id: GuildId,
        minutes: Option<u64>,
    ) -> Result<(), AppError> {
        let timeout = minutes.map(|m| Duration::from_secs(m * 60));
        self.state_manager
            .set_idle_timeout(guild_id, timeout)
            .await?;
        Ok(())
    }

    /// Update the last activity for a guild
    pub async fn update_last_activity(&self, guild_id: GuildId) -> Result<(), AppError> {
        self.state_manager.update_last_activity(guild_id).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ports::audio_player::AudioPlayerError;
    use crate::core::ports::state_manager::StateManagerError;
    use async_trait::async_trait;
    use mockall::mock;
    use mockall::predicate::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    // Mock the AudioPlayer
    mock! {
        pub AudioPlayerMock {}

        #[async_trait]
        impl AudioPlayer for AudioPlayerMock {
            async fn join(&self, guild_id: GuildId, channel_id: VoiceChannelId) -> Result<(), AudioPlayerError>;
            async fn leave(&self, guild_id: GuildId) -> Result<(), AudioPlayerError>;
            async fn is_connected(&self, guild_id: GuildId) -> bool;
            async fn play(&self, guild_id: GuildId, stream: crate::core::ports::audio_provider::AudioStream) -> Result<(), AudioPlayerError>;
            async fn queue_track(&self, guild_id: GuildId, stream: crate::core::ports::audio_provider::AudioStream) -> Result<usize, AudioPlayerError>;
            async fn skip(&self, guild_id: GuildId) -> Result<(), AudioPlayerError>;
            async fn pause(&self, guild_id: GuildId) -> Result<(), AudioPlayerError>;
            async fn resume(&self, guild_id: GuildId) -> Result<(), AudioPlayerError>;
            async fn stop(&self, guild_id: GuildId) -> Result<(), AudioPlayerError>;
            async fn get_queue(&self, guild_id: GuildId) -> Result<Vec<crate::core::ports::audio_player::QueuedTrack>, AudioPlayerError>;
            async fn set_volume(&self, guild_id: GuildId, volume: f32) -> Result<(), AudioPlayerError>;
            async fn get_volume(&self, guild_id: GuildId) -> Result<f32, AudioPlayerError>;
            async fn mute(&self, guild_id: GuildId) -> Result<(), AudioPlayerError>;
            async fn unmute(&self, guild_id: GuildId) -> Result<(), AudioPlayerError>;
            async fn shuffle(&self, guild_id: GuildId) -> Result<(), AudioPlayerError>;
            async fn current_track(&self, guild_id: GuildId) -> Result<Option<crate::core::ports::audio_player::QueuedTrack>, AudioPlayerError>;
            async fn register_text_channel(&self, guild_id: GuildId, channel_id: crate::core::ports::audio_player::TextChannelId) -> Result<(), AudioPlayerError>;
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
        ) -> Result<(), StateManagerError> {
            let json = serde_json::to_string(value)
                .map_err(|e| StateManagerError::StoreError(e.to_string()))?;

            let mut data = self.data.lock().unwrap();
            data.insert(key.to_string(), json);

            Ok(())
        }

        async fn get<T: serde::de::DeserializeOwned + Send + Sync + 'static>(
            &self,
            key: &str,
        ) -> Result<Option<T>, StateManagerError> {
            let data = self.data.lock().unwrap();

            if let Some(json) = data.get(key) {
                let value = serde_json::from_str(json)
                    .map_err(|e| StateManagerError::RetrieveError(e.to_string()))?;
                Ok(Some(value))
            } else {
                Ok(None)
            }
        }

        async fn delete(&self, key: &str) -> Result<(), StateManagerError> {
            let mut data = self.data.lock().unwrap();
            data.remove(key);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_check_connection_state() {
        let audio_player = MockAudioPlayerMock::new();
        let state_manager = InMemoryStateManager::new();

        let service = ConnectionService::new(Arc::new(audio_player), Arc::new(state_manager));

        let guild_id = GuildId(123);

        // Test mutually connected
        let mutual_channel = VoiceChannelId(456);
        let state =
            service.check_connection_state(guild_id, Some(mutual_channel), Some(mutual_channel));
        assert_eq!(state, ConnectionState::MutuallyConnected(mutual_channel));

        // Test separately connected
        let user_channel = VoiceChannelId(456);
        let bot_channel = VoiceChannelId(789);
        let state = service.check_connection_state(guild_id, Some(user_channel), Some(bot_channel));
        assert_eq!(
            state,
            ConnectionState::SeparatelyConnected(user_channel, bot_channel)
        );

        // Test only user connected
        let state = service.check_connection_state(guild_id, Some(user_channel), None);
        assert_eq!(state, ConnectionState::UserConnected(user_channel));

        // Test only bot connected
        let state = service.check_connection_state(guild_id, None, Some(bot_channel));
        assert_eq!(state, ConnectionState::BotConnected(bot_channel));

        // Test neither connected
        let state = service.check_connection_state(guild_id, None, None);
        assert_eq!(state, ConnectionState::NeitherConnected);
    }

    #[tokio::test]
    async fn test_check_idle_timeouts() {
        let mut audio_player = MockAudioPlayerMock::new();
        let state_manager = Arc::new(InMemoryStateManager::new());

        let guild_id = GuildId(123);

        // Setup expectations
        audio_player.expect_is_connected().returning(|_| true);

        audio_player.expect_leave().returning(|_| Ok(()));

        let service = ConnectionService::new(Arc::new(audio_player), state_manager.clone());

        // Set a small timeout (1 minute)
        service.set_idle_timeout(guild_id, Some(1)).await.unwrap();

        // Set last activity to be in the past
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Set last activity to 2 minutes ago
        let past = now - 120;
        let key = format!("guild:{}:last_activity", guild_id.0);
        state_manager.set(&key, &past).await.unwrap();

        // Check idle timeout - should disconnect
        let disconnected = service.check_idle_timeouts(guild_id).await.unwrap();
        assert!(disconnected);
    }
}
