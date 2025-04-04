use async_trait::async_trait;
use poise::serenity_prelude as serenity;
use rand::seq::SliceRandom;
use songbird::{
    input::Input as SongbirdInput,
    tracks::{PlayMode, TrackHandle},
    Call, Songbird,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::ports::{
    audio_player::{
        AudioPlayer, AudioPlayerError, AudioPlayerResult, GuildId, PlaybackStatus, QueuedTrack,
        TextChannelId, VoiceChannelId,
    },
    audio_provider::AudioStream,
};

/// Custom track metadata for storing requester information
#[derive(Debug, Clone)]
struct TrackRequestInfo {
    requester_name: String,
    requester_id: String,
}

/// Implementation of `AudioPlayer` using Songbird
#[derive(Debug)]
pub struct SongbirdPlayer {
    songbird: Arc<Songbird>,
    // Maps guild ID to notification channel
    notification_channels: Arc<RwLock<HashMap<u64, u64>>>,
}

impl SongbirdPlayer {
    /// Create a new Songbird player
    pub fn new(songbird: Arc<Songbird>) -> Self {
        Self {
            songbird,
            notification_channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Helper method to convert `AudioStream` to Songbird input
    fn convert_to_songbird_input(stream: AudioStream) -> Result<SongbirdInput, AudioPlayerError> {
        // Extract the provider data which should be a rusty_ytdl::Video
        let provider_data = stream.provider_data;

        // Check if this is a rusty_ytdl::Video
        if let Some(video) = provider_data.downcast_ref::<rusty_ytdl::Video>() {
            // Get the URL from the video
            let url = video.get_video_url();
            if url.is_empty() {
                return Err(AudioPlayerError::PlayError(
                    "No video URL available".to_string(),
                ));
            }

            // Create a YoutubeDl input source
            let yt_input = songbird::input::YoutubeDl::new(reqwest::Client::new(), url);

            // Convert to a generic SongbirdInput
            Ok(yt_input.into())
        } else {
            Err(AudioPlayerError::PlayError(
                "Unsupported audio stream type".to_string(),
            ))
        }
    }

    /// Helper method to convert Songbird track to our domain model
    async fn convert_to_queued_track(
        &self,
        track_handle: &TrackHandle,
        _guild_id: GuildId,
    ) -> AudioPlayerResult<QueuedTrack> {
        // Get track state
        let track_state = track_handle
            .get_info()
            .await
            .map_err(|e| AudioPlayerError::Other(format!("Failed to get track info: {e}")))?;

        // We would get the metadata from the track state, but that requires changes to the songbird API
        // For now, create basic metadata
        let metadata = crate::core::ports::audio_provider::AudioMetadata {
            title: Some("Unknown Track".to_string()),
            artist: None,
            album: None,
            duration: None,
            source_url: None,
            thumbnail_url: None,
        };

        // In the actual implementation, we would get the requester info from the track
        // But for now, just create a default
        let requester_info = TrackRequestInfo {
            requester_name: "Unknown".to_string(),
            requester_id: "0".to_string(),
        };

        // Determine track status
        let status = match track_state.playing {
            PlayMode::Play => PlaybackStatus::Playing,
            PlayMode::Pause => PlaybackStatus::Paused,
            PlayMode::Stop => PlaybackStatus::Stopped,
            _ => PlaybackStatus::Errored,
        };

        Ok(QueuedTrack {
            metadata,
            requester_name: requester_info.requester_name,
            requester_id: requester_info.requester_id,
            status,
            position: Some(track_state.position),
            duration: None, // Songbird doesn't provide this directly
        })
    }

    /// Helper method to add event handlers to a track
    #[allow(dead_code)] // This will be used in the future
    async fn add_track_event_handlers(&self, _track_handle: &TrackHandle, guild_id: GuildId) {
        // For now, just log the events instead of trying to add event handlers
        // This will be implemented properly once we have the correct event handler trait implementation
        let guild_id_u64 = guild_id.0;
        tracing::info!("Would add track event handlers for guild {}", guild_id_u64);

        // The proper implementation would look something like this:
        // track_handle.add_event(
        //     Event::Track(TrackEvent::End),
        //     songbird::tracks::TrackEndNotifier::new(/* parameters */)
        // );
        //
        // track_handle.add_event(
        //     Event::Track(TrackEvent::Error),
        //     songbird::tracks::TrackErrorNotifier::new(/* parameters */)
        // );
    }

    /// Helper method to get a call reference and ensure it exists
    async fn get_call(
        &self,
        guild_id: GuildId,
    ) -> Result<Arc<tokio::sync::Mutex<Call>>, AudioPlayerError> {
        self.songbird
            .get(serenity::GuildId::new(guild_id.0))
            .ok_or(AudioPlayerError::NotConnected)
    }
}

#[async_trait]
impl AudioPlayer for SongbirdPlayer {
    async fn join(&self, guild_id: GuildId, channel_id: VoiceChannelId) -> AudioPlayerResult<()> {
        // Check if already connected
        if let Some(call) = self.songbird.get(serenity::GuildId::new(guild_id.0)) {
            let channel = call.lock().await.current_channel();

            if let Some(current) = channel {
                // Simply compare the string representation of both channel IDs
                let current_str = format!("{current:?}");
                let channel_id_str = format!("{}", channel_id.0);
                if current_str.contains(&channel_id_str) {
                    // Already connected to this channel
                    return Ok(());
                }

                // Connected to a different channel, so leave first
                self.leave(guild_id).await?;
            }
        }

        // Join the channel
        match self
            .songbird
            .join(
                serenity::GuildId::new(guild_id.0),
                serenity::ChannelId::new(channel_id.0),
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(AudioPlayerError::JoinError(e.to_string())),
        }
    }

    async fn leave(&self, guild_id: GuildId) -> AudioPlayerResult<()> {
        match self
            .songbird
            .remove(serenity::GuildId::new(guild_id.0))
            .await
        {
            Ok(()) => Ok(()),
            Err(e) => Err(AudioPlayerError::LeaveError(e.to_string())),
        }
    }

    async fn is_connected(&self, guild_id: GuildId) -> bool {
        self.songbird
            .get(serenity::GuildId::new(guild_id.0))
            .is_some()
    }

    async fn play(&self, guild_id: GuildId, stream: AudioStream) -> AudioPlayerResult<()> {
        let call = self.get_call(guild_id).await?;
        let mut handler = call.lock().await;

        // Clear the queue
        handler.stop();

        // Convert the stream to a songbird input
        let input = Self::convert_to_songbird_input(stream)?;

        // Play the track
        let _track_handle = handler.play_input(input);

        // In a real implementation, we would add event handlers
        tracing::info!("Playing track in guild {}", guild_id.0);

        Ok(())
    }

    async fn queue_track(
        &self,
        guild_id: GuildId,
        stream: AudioStream,
    ) -> AudioPlayerResult<usize> {
        let call = self.get_call(guild_id).await?;
        let mut handler = call.lock().await;

        // Convert the stream to a songbird input
        let input = Self::convert_to_songbird_input(stream.clone())?;

        // Log requester info from the stream before creating the track
        let requester_name = stream
            .metadata
            .as_ref()
            .and_then(|m| m.artist.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        // Add event handlers - this just logs for now
        tracing::info!("Track queued: {}", requester_name);

        // Get the queue length before enqueueing the track
        let queue_len = handler.queue().len();

        // Create the track after we're done with the stream
        let _handle = handler.enqueue_input(input);

        // Calculate position using the length we got before
        let position = queue_len;

        Ok(position)
    }

    async fn skip(&self, guild_id: GuildId) -> AudioPlayerResult<()> {
        let call = self.get_call(guild_id).await?;
        let handler = call.lock().await;

        handler
            .queue()
            .skip()
            .map_err(|e| AudioPlayerError::Other(format!("Failed to skip track: {e}")))?;

        Ok(())
    }

    async fn pause(&self, guild_id: GuildId) -> AudioPlayerResult<()> {
        let call = self.get_call(guild_id).await?;
        let handler = call.lock().await;

        if let Some(track) = handler.queue().current() {
            track
                .pause()
                .map_err(|e| AudioPlayerError::Other(format!("Failed to pause track: {e}")))?;
        }

        Ok(())
    }

    async fn resume(&self, guild_id: GuildId) -> AudioPlayerResult<()> {
        let call = self.get_call(guild_id).await?;
        let handler = call.lock().await;

        if let Some(track) = handler.queue().current() {
            track
                .play()
                .map_err(|e| AudioPlayerError::Other(format!("Failed to resume track: {e}")))?;
        }

        Ok(())
    }

    async fn stop(&self, guild_id: GuildId) -> AudioPlayerResult<()> {
        let call = self.get_call(guild_id).await?;
        let mut handler = call.lock().await;

        handler.stop();

        Ok(())
    }

    async fn get_queue(&self, guild_id: GuildId) -> AudioPlayerResult<Vec<QueuedTrack>> {
        let call = self.get_call(guild_id).await?;
        let handler = call.lock().await;

        let queue = handler.queue().current_queue();

        // Convert Songbird tracks to our domain model
        let mut tracks = Vec::new();
        for track in queue {
            match self.convert_to_queued_track(&track, guild_id).await {
                Ok(queued_track) => tracks.push(queued_track),
                Err(e) => tracing::warn!("Failed to convert track: {:?}", e),
            }
        }

        Ok(tracks)
    }

    async fn set_volume(&self, guild_id: GuildId, volume: f32) -> AudioPlayerResult<()> {
        let call = self.get_call(guild_id).await?;
        let handler = call.lock().await;

        if let Some(track) = handler.queue().current() {
            track
                .set_volume(volume)
                .map_err(|e| AudioPlayerError::Other(format!("Failed to set volume: {e}")))?;
        }

        Ok(())
    }

    async fn get_volume(&self, guild_id: GuildId) -> AudioPlayerResult<f32> {
        let call = self.get_call(guild_id).await?;
        let handler = call.lock().await;

        if let Some(track) = handler.queue().current() {
            let state = track
                .get_info()
                .await
                .map_err(|e| AudioPlayerError::Other(format!("Failed to get track info: {e}")))?;
            Ok(state.volume)
        } else {
            Ok(1.0) // Default volume
        }
    }

    async fn mute(&self, guild_id: GuildId) -> AudioPlayerResult<()> {
        let call = self.get_call(guild_id).await?;
        let mut handler = call.lock().await;

        handler
            .mute(true)
            .await
            .map_err(|e| AudioPlayerError::Other(format!("Failed to mute: {e}")))?;

        Ok(())
    }

    async fn unmute(&self, guild_id: GuildId) -> AudioPlayerResult<()> {
        let call = self.get_call(guild_id).await?;
        let mut handler = call.lock().await;

        handler
            .mute(false)
            .await
            .map_err(|e| AudioPlayerError::Other(format!("Failed to unmute: {e}")))?;

        Ok(())
    }

    async fn shuffle(&self, guild_id: GuildId) -> AudioPlayerResult<()> {
        let call = self.get_call(guild_id).await?;
        let handler = call.lock().await;

        let queue = handler.queue().current_queue();

        // Skip the first track (currently playing)
        if queue.len() > 1 {
            let (current, rest) = queue.split_at(1);
            let mut rest = rest.to_vec();

            // Shuffle the rest of the queue
            rest.shuffle(&mut rand::rng());

            // Recreate the queue with the current track and shuffled rest
            handler
                .queue()
                .modify_queue(|_| current.to_vec().into_iter().chain(rest).collect::<Vec<_>>());
        }

        Ok(())
    }

    async fn current_track(&self, guild_id: GuildId) -> AudioPlayerResult<Option<QueuedTrack>> {
        let call = self.get_call(guild_id).await?;
        let handler = call.lock().await;

        if let Some(track) = handler.queue().current() {
            let queued_track = self.convert_to_queued_track(&track, guild_id).await?;
            Ok(Some(queued_track))
        } else {
            Ok(None)
        }
    }

    async fn register_text_channel(
        &self,
        guild_id: GuildId,
        channel_id: TextChannelId,
    ) -> AudioPlayerResult<()> {
        let mut channels = self.notification_channels.write().await;
        channels.insert(guild_id.0, channel_id.0);
        Ok(())
    }
}
