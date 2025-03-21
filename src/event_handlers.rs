use crack_types::YoutubeDl;
use poise::serenity_prelude as serenity;
use serenity::all::{async_trait, ChannelId, Http, GuildId};
use songbird::{EventContext, Event, TrackEvent, EventHandler as VoiceEventHandler};
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};
use crate::check_msg;
use crate::Data;

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
                if let Some(handler_lock) = self.data.songbird.get(self.guild_id) {
                    let mut handler = handler_lock.lock().await;
                    
                    // Get the next track from our custom queue
                    if let Some(track) = queue.dequeue().await {
                        // Play the next track
                        let src = songbird::input::Input::from(YoutubeDl::new(self.data.http_client.clone(), track.get_url()));
                        
                        let song = handler.play_input(src);
                        
                        // Update activity timestamp in any ChannelDurationNotifier
                        for (_, event_handler) in handler.global_events.iter() {
                            if let Some(notifier) = event_handler.downcast_ref::<ChannelDurationNotifier>() {
                                notifier.update_activity();
                            }
                        }
                        
                        // Add event handlers to the new track
                        let _ = song.add_event(
                            Event::Track(TrackEvent::End),
                            EnhancedTrackEndNotifier {
                                chan_id: self.chan_id,
                                http: self.http.clone(),
                                guild_id: self.guild_id,
                                data: self.data.clone(),
                                is_looping: self.is_looping.clone(),
                            },
                        );
                        
                        let _ = song.add_event(
                            Event::Track(TrackEvent::Error),
                            EnhancedTrackErrorNotifier {
                                chan_id: self.chan_id,
                                http: self.http.clone(),
                                guild_id: self.guild_id,
                                data: self.data.clone(),
                                is_looping: self.is_looping.clone(),
                            },
                        );
                        
                        // Notify that the next track is playing
                        check_msg(
                            self.chan_id
                                .say(&self.http, &format!("Now playing: {}", track.get_title()))
                                .await,
                        );
                    }
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
                    check_msg(
                        self.chan_id
                            .say(&self.http, "Queue finished.")
                            .await,
                    );
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
                    .say(&self.http, "Error playing track, skipping to next in queue...")
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
                            let src = YoutubeDl::new(self.data.http_client.clone(), next_track.get_url());
                            // let src = match YoutubeDl::new(self.data.http_client.clone(), next_track.get_url()).into_input() {
                            //     Ok(input) => input,
                            //     Err(e) => {
                            //         check_msg(
                            //             self.chan_id
                            //                 .say(&self.http, &format!("Error playing track \"{}\": {}", next_track.get_title(), e))
                            //                 .await,
                            //         );
                            //         return None;
                            //     }
                            // };
                            
                            let song = handler.play_input(src.into());
                            
                            // Update activity timestamp in any ChannelDurationNotifier
                            for (_, event_handler) in handler.global_events.iter() {
                                if let Some(notifier) = event_handler.downcast_ref::<ChannelDurationNotifier>() {
                                    notifier.update_activity();
                                }
                            }
                            
                            // Add the same event handlers to the new track
                            let _ = song.add_event(
                                Event::Track(TrackEvent::End),
                                EnhancedTrackEndNotifier {
                                    chan_id: self.chan_id,
                                    http: self.http.clone(),
                                    guild_id: self.guild_id,
                                    data: self.data.clone(),
                                    is_looping: self.is_looping.clone(),
                                },
                            );
                            
                            let _ = song.add_event(
                                Event::Track(TrackEvent::Error),
                                EnhancedTrackErrorNotifier {
                                    chan_id: self.chan_id,
                                    http: self.http.clone(),
                                    guild_id: self.guild_id,
                                    data: self.data.clone(),
                                    is_looping: self.is_looping.clone(),
                                },
                            );
                            
                            check_msg(
                                self.chan_id
                                    .say(&self.http, &format!("Now playing: {}", next_track.get_title()))
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
                        check_msg(
                            self.chan_id
                                .say(&self.http, "Queue finished.")
                                .await,
                        );
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
    pub idle_timeout: usize, // In minutes, 0 means never leave
    pub last_activity: Arc<AtomicUsize>, // Timestamp of last activity in minutes
}

impl ChannelDurationNotifier {
    /// Update the last activity timestamp to the current time
    pub fn update_activity(&self) {
        let current_time = self.count.load(Ordering::Relaxed);
        self.last_activity.store(current_time, Ordering::Relaxed);
    }
}

#[async_trait]
impl VoiceEventHandler for ChannelDurationNotifier {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        let count_before = self.count.fetch_add(1, Ordering::Relaxed);
        
        // Check if we should leave due to inactivity
        if self.idle_timeout > 0 {
            let current_time = count_before + 1; // Current time in minutes since joining
            let last_activity = self.last_activity.load(Ordering::Relaxed);
            let idle_time = current_time.saturating_sub(last_activity);
            
            if idle_time >= self.idle_timeout {
                check_msg(
                    self.chan_id
                        .say(
                            &self.http,
                            &format!(
                                "Leaving channel due to inactivity for {} minutes.",
                                idle_time
                            ),
                        )
                        .await,
                );
                
                // Leave the channel
                let _ = self.songbird.remove(self.guild_id).await;
                return Some(Event::Cancel); // Cancel this event handler
            }
        }
        
        check_msg(
            self.chan_id
                .say(
                    &self.http,
                    &format!(
                        "I've been in this channel for {} minutes!",
                        count_before + 1
                    ),
                )
                .await,
        );

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
