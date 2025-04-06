use crate::{
    check_voice_connections,
    event_handlers::{ChannelDurationNotifier, EnhancedTrackErrorNotifier},
    suggestion2, Connection, Context, EnhancedTrackEndNotifier, IdleTimeoutInfo, TrackMetadata,
};
use ::serenity::all::{AutocompleteChoice, AutocompleteValue, CreateAutocompleteResponse};
use crack_types::CrackedError;
use poise::serenity_prelude as serenity;
use rand::seq::SliceRandom;
use serenity::Mentionable;
use songbird::{
    input::{Compose, YoutubeDl},
    tracks::Track,
    Event, TrackEvent,
};
use std::{
    borrow::Cow,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tracing::info;

/// Joins the voice channel of the user
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn join(ctx: Context<'_>) -> Result<(), crack_types::Error> {
    let guild = ctx.guild().unwrap().clone();
    let guild_id = guild.id;

    let user_id = ctx.author().id;
    let bot_id = ctx.http().get_current_user().await?.id;
    let conn_info = check_voice_connections(&guild, &user_id, &bot_id);

    let connect_to = match conn_info {
        Connection::User(channel_id) => channel_id,
        Connection::Bot(_) => {
            ctx.say("Bot is already in a voice channel").await?;
            return Ok(());
        }
        Connection::Separate(_, _) => {
            ctx.say("Bot is already in a voice channel").await?;
            return Ok(());
        }
        Connection::Neither => {
            ctx.say("Not in a voice channel").await?;
            return Ok(());
        }
        Connection::Mutual(_, _) => {
            ctx.say("Bot is already in your voice channel").await?;
            return Ok(());
        }
    };

    let songbird = ctx.data().songbird.clone();

    if let Ok(handle_lock) = songbird.join(guild_id, connect_to).await {
        ctx.say(format!("Joined {}", connect_to.mention())).await?;

        let chan_id = ctx.channel_id();
        let send_http = ctx.serenity_context().http.clone();

        let mut handle = handle_lock.lock().await;

        // Add the track error and end event handlers globally
        handle.add_global_event(
            Event::Track(TrackEvent::Error),
            EnhancedTrackErrorNotifier {
                chan_id,
                http: send_http.clone(),
                guild_id,
                data: ctx.data().clone(),
                is_looping: Arc::new(AtomicBool::new(false)),
            },
        );
        handle.add_global_event(
            Event::Track(TrackEvent::End),
            EnhancedTrackEndNotifier {
                chan_id,
                http: send_http.clone(),
                guild_id,
                data: ctx.data().clone(),
                is_looping: Arc::new(AtomicBool::new(false)),
            },
        );

        // Initialize the idle timeout info for this guild
        let _ = ctx
            .data()
            .idle_timeouts
            .entry(guild_id)
            .and_modify(|info| {
                // Initialize the last activity timestamp to the current time (0 minutes since joining)
                info.last_activity.store(0, Ordering::Relaxed);
            })
            .or_insert_with(|| {
                let info = IdleTimeoutInfo::default();
                info.last_activity.store(0, Ordering::Relaxed);
                info
            });

        // Create the channel duration notifier
        let notifier = ChannelDurationNotifier {
            chan_id,
            count: Arc::default(),
            http: send_http,
            guild_id,
            songbird: ctx.data().songbird.clone(),
            data: ctx.data().clone(),
        };

        // Add the notifier as a global event
        handle.add_global_event(Event::Periodic(Duration::from_secs(60), None), notifier);
    } else {
        ctx.say("Error joining the channel").await?;
    }

    Ok(())
}

/// Leaves the voice channel
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn leave(ctx: Context<'_>) -> Result<(), crack_types::Error> {
    let guild_id = ctx.guild_id().ok_or(CrackedError::from("No guild ID?"))?;
    let songbird = ctx.data().songbird.clone();

    if let Some(handle_lock) = songbird.get(guild_id) {
        // Remove all global events
        handle_lock.lock().await.remove_all_global_events();

        if let Err(e) = songbird.remove(guild_id).await {
            ctx.say(format!("Failed: {e:?}")).await?;
        } else {
            ctx.say("Left voice channel").await?;
        }
    } else {
        ctx.say("Not in a voice channel").await?;
    }

    Ok(())
}

/// Autocomplete to suggest a search query.
pub async fn autocomplete<'a>(
    _ctx: poise::ApplicationContext<'_, crate::Data, crate::Error>,
    searching: &'a str,
) -> CreateAutocompleteResponse<'a> {
    let choices: Vec<_> = suggestion2(searching)
        .await
        .into_iter()
        .map(|s| {
            let name = s.title;
            let value = s.url;
            AutocompleteChoice::new(name, AutocompleteValue::String(Cow::Owned(value)))
        })
        .collect();
    let res = CreateAutocompleteResponse::new();
    res.set_choices(Cow::Owned(choices.clone()))
}

/// Plays a track from a URL
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn play(
    ctx: Context<'_>,
    #[autocomplete = "autocomplete"]
    #[description = "URL to media or search term"]
    url: String,
) -> Result<(), crack_types::Error> {
    // Immediately acknowledge the interaction to prevent timeout
    ctx.defer().await?;

    let guild_id = ctx.guild_id().ok_or(CrackedError::from("No guild ID?"))?;
    let data = ctx.data();
    let do_search = !url.starts_with("http");

    if let Some(handler_lock) = data.songbird.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        let mut src = if do_search {
            YoutubeDl::new_search(data.req_client.clone(), url)
        } else {
            // Create a resolved track from the URL
            //let query = QueryType::VideoLink(url);
            //let track = ResolvedTrack::new(query).with_user_id(ctx.author().id);
            YoutubeDl::new(data.req_client.clone(), url)
        };

        let requesting_user = ctx.author().name.clone().to_string();
        let requesting_user_id = ctx.author().id.to_string();
        let metadata = src.aux_metadata().await.ok();
        // We store the user ID in the track data and the track title in the metadata
        let track_data = Arc::new(TrackMetadata {
            requesting_user,
            requesting_user_id,
            metadata,
        });
        let track = Track::new_with_data(src.into(), track_data);

        // This handler object will allow you to, as needed,
        // control the audio track via events and further commands.
        //let _ = handler.play_input(src.into());
        let x = handler.enqueue(track).await;
        let state = match x.get_info().await {
            Ok(state) => format!("{:?}", state.playing),
            Err(e) => {
                format!("Failed to play: {e:?}")
            }
        };

        info!("State: {}", state);

        // Update activity timestamp by bumping it
        if let Some(idle_info) = ctx.data().idle_timeouts.get(&guild_id) {
            idle_info.bump_activity();
        }

        let queue_len = handler.queue().len();
        if queue_len > 0 {
            ctx.say(format!("Added song to queue: position {queue_len}"))
                .await?;
        } else {
            ctx.say("Playing song").await?;
        }
    } else {
        ctx.say("Not in a voice channel").await?;
    }

    Ok(())
}

/// Skips the current song
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn skip(ctx: Context<'_>) -> Result<(), crack_types::Error> {
    // Immediately acknowledge the interaction to prevent timeout
    ctx.defer().await?;

    let guild_id = ctx.guild_id().ok_or(CrackedError::NoGuildId)?;
    let songbird = ctx.data().songbird.clone();

    if let Some(handler_lock) = songbird.get(guild_id) {
        let handler = handler_lock.lock().await;

        // Skip the current song in songbird's queue
        let _ = handler.queue().skip();

        let len = handler.queue().len();

        ctx.say(format!("Song skipped: {len} in queue.")).await?;
    } else {
        ctx.say("Not in a voice channel to play in").await?;
    }

    Ok(())
}

/// Stops playback and clears the queue
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn stop(ctx: Context<'_>) -> Result<(), crack_types::Error> {
    // Immediately acknowledge the interaction to prevent timeout
    ctx.defer().await?;

    let guild_id = ctx.guild_id().ok_or(CrackedError::NoGuildId)?;
    let songbird = ctx.data().songbird.clone();

    if let Some(handler_lock) = songbird.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        // Stop the songbird queue
        handler.stop();

        ctx.say("Queue cleared.").await?;
    } else {
        ctx.say("Not in a voice channel to play in").await?;
    }

    Ok(())
}

/// Helper function to format track information for the queue display
async fn format_track_info(track: &songbird::tracks::TrackHandle, index: Option<usize>) -> String {
    let prefix = if let Some(i) = index {
        format!("{i}. ")
    } else {
        "▶️ **Currently Playing:** ".to_string()
    };

    // Much simpler approach - just get basic info about track status
    if let Ok(track_info) = track.get_info().await {
        let data = track.data::<TrackMetadata>();
        // Format play time
        let play_time = track_info.play_time;
        let position_str = {
            let minutes = play_time.as_secs() / 60;
            let seconds = play_time.as_secs() % 60;
            let duration = data.get_duration_as_secs();
            let total_minutes = duration / 60;
            let total_seconds = duration % 60;
            format!("{minutes:02}:{seconds:02} out of {total_minutes:02}:{total_seconds:02}")
        };

        let status = match track_info.playing {
            songbird::tracks::PlayMode::Play => "▶️",
            songbird::tracks::PlayMode::Pause => "⏸️",
            _ => "⏹️", // Default case for Stop and any future variants
        };

        // Simple format with just track position and status
        format!("{prefix}{status} Track [{position_str}]")
    } else {
        format!("{prefix}Unknown track")
    }
}

/// Displays the current queue with pagination support
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn show_queue(ctx: Context<'_>) -> Result<(), crack_types::Error> {
    use std::fmt::Write;

    // Immediately acknowledge the interaction to prevent timeout
    ctx.defer().await?;

    let guild_id = ctx.guild_id().ok_or(CrackedError::from("No guild ID?"))?;
    let songbird = ctx.data().songbird.clone();

    if let Some(handler_lock) = songbird.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        let current_queue = queue.current_queue();

        if current_queue.is_empty() {
            ctx.say(crate::EMPTY_QUEUE).await?;
            return Ok(());
        }

        // Create paginated response
        let tracks_per_page = 10;
        #[allow(clippy::manual_div_ceil)]
        let total_pages = (current_queue.len() + tracks_per_page - 1) / tracks_per_page;

        // Generate pages
        let mut pages = Vec::with_capacity(total_pages);
        for page_idx in 0..total_pages {
            let start_idx = page_idx * tracks_per_page;
            let end_idx = (start_idx + tracks_per_page).min(current_queue.len());

            // Build the page content
            let mut content = String::new();

            // Always include currently playing track on every page
            if let Some(current) = current_queue.first() {
                content.push_str(&format_track_info(current, None).await);
                content.push_str("\n\n");
            }

            // Add the tracks for this page
            if start_idx > 0 || end_idx > 1 {
                content.push_str("**Up Next:**\n");
                let range_start = if start_idx == 0 { 1 } else { start_idx };

                for (i, track) in current_queue[range_start..end_idx].iter().enumerate() {
                    content.push_str(&format_track_info(track, Some(range_start + i)).await);
                    content.push('\n');
                }
            }

            // Add page info
            let _ = write!(
                content,
                "\n**Page {}/{}** · {} tracks total",
                page_idx + 1,
                total_pages,
                current_queue.len()
            );

            pages.push(content);
        }

        // Create a vector of string slices for paginate
        let page_refs: Vec<&str> = pages.iter().map(String::as_str).collect();

        // Use Poise's pagination
        // We'll probably need to customize this eventually.
        poise::builtins::paginate(ctx, &page_refs).await?;
    } else {
        ctx.say("Not in a voice channel").await?;
    }

    Ok(())
}

/// Pauses the current track
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn pause(ctx: Context<'_>) -> Result<(), crack_types::Error> {
    // Immediately acknowledge the interaction to prevent timeout
    ctx.defer().await?;

    let guild_id = ctx.guild_id().ok_or(CrackedError::from("No guild ID?"))?;
    let songbird = ctx.data().songbird.clone();

    if let Some(handler_lock) = songbird.get(guild_id) {
        let handler = handler_lock.lock().await;

        // Check if there's a track playing
        if handler.queue().is_empty() {
            ctx.say("Nothing is playing to pause.").await?;
            return Ok(());
        }

        // Pause the queue
        let _ = handler.queue().pause();

        // Update activity timestamp
        if let Some(idle_info) = ctx.data().idle_timeouts.get(&guild_id) {
            idle_info.bump_activity();
        }

        ctx.say("Playback paused.").await?;
    } else {
        ctx.say("Not in a voice channel.").await?;
    }

    Ok(())
}

/// Resumes playback if paused
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn resume(ctx: Context<'_>) -> Result<(), crack_types::Error> {
    // Immediately acknowledge the interaction to prevent timeout
    ctx.defer().await?;

    let guild_id = ctx.guild_id().ok_or(CrackedError::NoGuildId)?;
    let songbird = ctx.data().songbird.clone();

    if let Some(handler_lock) = songbird.get(guild_id) {
        let handler = handler_lock.lock().await;

        // Check if there's a track in the queue
        if handler.queue().is_empty() {
            ctx.say("Nothing to resume.").await?;
            return Ok(());
        }

        // Resume the queue
        let _ = handler.queue().resume();

        // Update activity timestamp
        if let Some(idle_info) = ctx.data().idle_timeouts.get(&guild_id) {
            idle_info.bump_activity();
        }

        ctx.say("Playback resumed.").await?;
    } else {
        ctx.say("Not in a voice channel.").await?;
    }

    Ok(())
}

/// Adjusts the volume (0-100)
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn volume(
    ctx: Context<'_>,
    #[description = "Volume level (0-100)"]
    #[min = 0.0]
    #[max = 100.0]
    volume: f32,
) -> Result<(), crack_types::Error> {
    // Immediately acknowledge the interaction to prevent timeout
    ctx.defer().await?;

    // For prefix commands, we still need to validate the range
    // (the min/max constraints only apply to slash commands)
    if !(0.0..=100.0).contains(&volume) {
        ctx.say("Volume must be between 0 and 100.").await?;
        return Ok(());
    }

    let guild_id = ctx.guild_id().ok_or(CrackedError::NoGuildId)?;
    let songbird = ctx.data().songbird.clone();

    if let Some(handler_lock) = songbird.get(guild_id) {
        let handler = handler_lock.lock().await;

        // Convert volume to a decimal (0.0 - 1.0)
        let decimal_volume = volume / 100.0;

        // Set the volume for all tracks in the queue
        handler.queue().modify_queue(|queue| {
            // Apply volume to all tracks
            for track in queue {
                let _ = track.set_volume(decimal_volume);
            }
        });

        // Update activity timestamp
        if let Some(idle_info) = ctx.data().idle_timeouts.get(&guild_id) {
            idle_info.bump_activity();
        }

        ctx.say(format!("Volume set to {volume:02}%")).await?;
    } else {
        ctx.say("Not in a voice channel.").await?;
    }

    Ok(())
}

/// Shuffles the queue
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn shuffle(ctx: Context<'_>) -> Result<(), crack_types::Error> {
    // Immediately acknowledge the interaction to prevent timeout
    ctx.defer().await?;

    let guild_id = ctx.guild_id().unwrap();
    let songbird = ctx.data().songbird.clone();

    if let Some(handler_lock) = songbird.get(guild_id) {
        let handler = handler_lock.lock().await;

        handler.queue().current_queue().shuffle(&mut rand::rng());

        ctx.say("Queue shuffled!").await?;
    } else {
        ctx.say("Not in a voice channel.").await?;
    }

    Ok(())
}
