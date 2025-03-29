use crate::{check_msg, Data};
use poise::serenity_prelude as serenity;
use serenity::all::{async_trait, ChannelId, GuildId, Http};
use songbird::input::YoutubeDl;
use songbird::{Event, EventContext, EventHandler as VoiceEventHandler};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize},
    Arc,
};

/// Enhanced TrackEndNotifier with better queue handling
pub struct EnhancedTrackEndNotifier {
    pub chan_id: ChannelId,
    pub http: Arc<Http>,
    pub guild_id: GuildId,
    pub data: Arc<Data>,
    pub is_looping: Arc<AtomicBool>,
}

#[async_trait]
impl VoiceEventHandler for EnhancedTrackEndNotifier {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        // Get the custom queue for this guild
        if let Some(queue) = self.data.guild_queues.get(&self.guild_id) {
            // Check if there are more tracks in the queue
            if !queue.is_empty().await {
                // Get the handler for this guild
                // Update our metadata queue to match Songbird's state
                // by removing the track that just ended
                let _ = queue.dequeue().await;

                // Check if there are more tracks in the queue
                if !queue.is_empty().await {
                    // Get the next track from our custom queue for display purposes
                    if let Some(next_track) = queue.get(0).await {
                        // Notify that the next track is playing
                        check_msg(
                            self.chan_id
                                .say(
                                    &self.http,
                                    &format!("Now playing: {}", next_track.get_title()),
                                )
                                .await,
                        );

                        // Update activity timestamp
                        if let Some(idle_info) = self.data.idle_timeouts.get(&self.guild_id) {
                            idle_info.bump_activity();
                        }
                    }
                } else {
                    // Queue is empty
                    check_msg(self.chan_id.say(&self.http, "Queue finished.").await);
                }
            } else {
                // Queue is empty
                // Check if we're looping
                if self.is_looping.load(std::sync::atomic::Ordering::Relaxed) {
                    check_msg(
                        self.chan_id
                            .say(&self.http, "Queue ended. Restarting loop...")
                            .await,
                    );

                    // Handle looping logic by getting the original queue backup
                    // In a real implementation, you'd need to store this somewhere
                    // For now, we'll just indicate that looping would happen here

                    // This is where you'd restore the queue from a backup
                    // For example:
                    // if let Some(backup) = self.data.queue_backups.get(&self.guild_id) {
                    //     let mut original_tracks = backup.value().clone();
                    //     queue.append_vec(original_tracks).await;
                    //
                    //     // Start playing the first track
                    //     if let Some(handler_lock) = self.data.songbird.get(self.guild_id) {
                    //         // ... similar to the code above to play the next track
                    //     }
                    // }
                } else {
                    // Not looping, just notify queue is finished
                    check_msg(self.chan_id.say(&self.http, "Queue finished.").await);
                }
            }
        }

        None
    }
}

/// Enhanced TrackErrorNotifier with better queue handling
pub struct EnhancedTrackErrorNotifier {
    pub chan_id: ChannelId,
    pub http: Arc<Http>,
    pub guild_id: serenity::GuildId,
    pub data: Arc<Data>,
    pub is_looping: Arc<std::sync::atomic::AtomicBool>,
}

#[async_trait]
impl VoiceEventHandler for EnhancedTrackErrorNotifier {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track([(_, track)]) = ctx {
            // Notify about the error
            check_msg(
                self.chan_id
                    .say(
                        &self.http,
                        "Error playing track, skipping to next in queue...",
                    )
                    .await,
            );

            // Stop the current track
            let _ = track.stop();

            // Get the custom queue for this guild
            if let Some(queue) = self.data.guild_queues.get(&self.guild_id) {
                // Handle playing next track - same logic as in EnhancedTrackEndNotifier
                // This is intentionally duplicated to make the error handler independent
                if !queue.is_empty().await {
                    if let Some(handler_lock) = self.data.songbird.get(self.guild_id) {
                        let mut handler = handler_lock.lock().await;

                        if let Some(next_track) = queue.dequeue().await {
                            let src =
                                YoutubeDl::new(self.data.req_client.clone(), next_track.get_url());

                            let _song = handler.play_input(src.into());

                            // Update activity timestamp by bumping it
                            if let Some(idle_info) = self.data.idle_timeouts.get(&self.guild_id) {
                                idle_info.bump_activity();
                            }

                            check_msg(
                                self.chan_id
                                    .say(
                                        &self.http,
                                        &format!("Now playing: {}", next_track.get_title()),
                                    )
                                    .await,
                            );
                        }
                    }
                } else {
                    // Same loop handling logic as in EnhancedTrackEndNotifier
                    if self.is_looping.load(std::sync::atomic::Ordering::Relaxed) {
                        check_msg(
                            self.chan_id
                                .say(&self.http, "Queue ended. Restarting loop...")
                                .await,
                        );

                        // Loop handling code would go here
                    } else {
                        check_msg(self.chan_id.say(&self.http, "Queue finished.").await);
                    }
                }
            }
        }

        None
    }
}

pub struct ChannelDurationNotifier {
    pub chan_id: ChannelId,
    pub count: Arc<AtomicUsize>,
    pub http: Arc<Http>,
    pub guild_id: GuildId,
    pub songbird: Arc<songbird::Songbird>,
    pub data: Arc<Data>,
}

// impl ChannelDurationNotifier {
//     /// Update the last activity timestamp to the current time
//     pub fn update_activity(&self) {
//         let current_time = self.count.load(Ordering::Relaxed);

//         // Get or create the idle timeout info for this guild
//         if let Some(idle_info) = self.data.idle_timeouts.get(&self.guild_id) {
//             // Use the new helper method for setting activity to a specific time
//             idle_info.set_activity_to(current_time);
//         }
//     }
// }

#[async_trait]
impl VoiceEventHandler for ChannelDurationNotifier {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        // 1. Get if we are currently playing a track.
        // 2. If we are, update the last activity timestamp to the current time.
        // 3. If we are not, check if the last activity timestamp is older than the threshold.
        // 4. If it is, notify the channel and leave the voice channel.
        // let handler_lock = self.songbird.get(self.guild_id)?;
        // let handler = handler_lock.lock().await;
        None
    }
}

pub struct SongFader {
    pub chan_id: ChannelId,
    pub http: Arc<Http>,
}

#[async_trait]
impl VoiceEventHandler for SongFader {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(&[(state, track)]) = ctx {
            let _ = track.set_volume(state.volume / 2.0);

            if state.volume < 1e-2 {
                let _ = track.stop();
                check_msg(self.chan_id.say(&self.http, "Stopping song...").await);
                Some(Event::Cancel)
            } else {
                check_msg(self.chan_id.say(&self.http, "Volume reduced.").await);
                None
            }
        } else {
            None
        }
    }
}

pub struct SongEndNotifier {
    pub chan_id: ChannelId,
    pub http: Arc<Http>,
}

#[async_trait]
impl VoiceEventHandler for SongEndNotifier {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        check_msg(
            self.chan_id
                .say(&self.http, "Song faded out completely!")
                .await,
        );

        None
    }
}
