use crate::{check_msg, Data};
use poise::serenity_prelude as serenity;
use serenity::all::{async_trait, ChannelId, GuildId, Http};
use songbird::{Event, EventContext, EventHandler as VoiceEventHandler};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize},
    Arc,
};

/// Enhanced track end notifier with queue checking and logging.
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
        let guild_id = self.guild_id;
        if let Some(handle_guard) = self.data.songbird.get(guild_id) {
            let call = handle_guard.lock().await;
            let queue = call.queue().clone();
            // Check if the queue is empty
            if queue.is_empty() {
                // Notify that the queue is finished
                check_msg(self.chan_id.say(&self.http, "Queue finished.").await);
            } else {
                // Notify that the next track is playing
                check_msg(
                    self.chan_id
                        .say(&self.http, "Playing next track in queue...")
                        .await,
                );
            }
        }
        None
    }
}

/// Enhanced error notifier with idle timeout handling and logging.
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
        if let EventContext::Track([(track_state, _track)]) = ctx {
            // Check if the track has an error
            let log_str = format!("{track_state:?}");
            tracing::error!("Track error: {log_str}");
            // Notify about the error
            check_msg(
                self.chan_id
                    .say(
                        &self.http,
                        "Error playing track, skipping to next in queue...",
                    )
                    .await,
            );

            // Update activity timestamp by bumping it
            self.data.bump_activity(self.guild_id);
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
