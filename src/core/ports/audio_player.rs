use async_trait::async_trait;
use std::fmt;

use super::audio_provider::AudioMetadata;
use super::audio_provider::AudioStream;

/// Represents a unique identifier for a voice channel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoiceChannelId(pub u64);

impl fmt::Display for VoiceChannelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Represents a unique identifier for a server/guild
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GuildId(pub u64);

impl fmt::Display for GuildId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Represents a unique identifier for a text channel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextChannelId(pub u64);

impl fmt::Display for TextChannelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<TextChannelId> for serenity::all::ChannelId {
    fn from(id: TextChannelId) -> Self {
        serenity::all::ChannelId::new(id.0)
    }
}

/// Enum for track playback status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
    Errored,
}

/// Represents information about a track in the queue
#[derive(Debug, Clone)]
pub struct QueuedTrack {
    pub metadata: AudioMetadata,
    pub requester_name: String,
    pub requester_id: String,
    pub status: PlaybackStatus,
    pub position: Option<std::time::Duration>,
    pub duration: Option<std::time::Duration>,
}

/// Error type for AudioPlayer operations
#[derive(Debug, thiserror::Error)]
pub enum AudioPlayerError {
    #[error("Not connected to a voice channel")]
    NotConnected,

    #[error("Already connected to a voice channel")]
    AlreadyConnected,

    #[error("Failed to join channel: {0}")]
    JoinError(String),

    #[error("Failed to leave channel: {0}")]
    LeaveError(String),

    #[error("Failed to play track: {0}")]
    PlayError(String),

    #[error("No tracks in queue")]
    EmptyQueue,

    #[error("Invalid track index")]
    InvalidTrackIndex,

    #[error("Audio player error: {0}")]
    Other(String),
}

/// Result type for AudioPlayer operations
pub type AudioPlayerResult<T> = Result<T, AudioPlayerError>;

/// Interface for audio playback functionality
#[async_trait]
pub trait AudioPlayer: Send + Sync + 'static {
    /// Join a voice channel
    async fn join(&self, guild_id: GuildId, channel_id: VoiceChannelId) -> AudioPlayerResult<()>;

    /// Leave a voice channel
    async fn leave(&self, guild_id: GuildId) -> AudioPlayerResult<()>;

    /// Get connection status for a guild
    async fn is_connected(&self, guild_id: GuildId) -> bool;

    /// Play a track immediately (stops current playback if any)
    async fn play(&self, guild_id: GuildId, stream: AudioStream) -> AudioPlayerResult<()>;

    /// Add a track to the queue
    async fn queue_track(&self, guild_id: GuildId, stream: AudioStream)
        -> AudioPlayerResult<usize>;

    /// Skip the current track
    async fn skip(&self, guild_id: GuildId) -> AudioPlayerResult<()>;

    /// Pause playback
    async fn pause(&self, guild_id: GuildId) -> AudioPlayerResult<()>;

    /// Resume playback
    async fn resume(&self, guild_id: GuildId) -> AudioPlayerResult<()>;

    /// Stop playback and clear the queue
    async fn stop(&self, guild_id: GuildId) -> AudioPlayerResult<()>;

    /// Get the current queue
    async fn get_queue(&self, guild_id: GuildId) -> AudioPlayerResult<Vec<QueuedTrack>>;

    /// Set volume (0.0 to 1.0)
    async fn set_volume(&self, guild_id: GuildId, volume: f32) -> AudioPlayerResult<()>;

    /// Get current volume
    async fn get_volume(&self, guild_id: GuildId) -> AudioPlayerResult<f32>;

    /// Mute audio output
    async fn mute(&self, guild_id: GuildId) -> AudioPlayerResult<()>;

    /// Unmute audio output
    async fn unmute(&self, guild_id: GuildId) -> AudioPlayerResult<()>;

    /// Shuffle the queue
    async fn shuffle(&self, guild_id: GuildId) -> AudioPlayerResult<()>;

    /// Get the current track
    async fn current_track(&self, guild_id: GuildId) -> AudioPlayerResult<Option<QueuedTrack>>;

    /// Register a text channel for sending player notifications
    async fn register_text_channel(
        &self,
        guild_id: GuildId,
        channel_id: TextChannelId,
    ) -> AudioPlayerResult<()>;
}
