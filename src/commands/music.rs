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

    let guild_id = ctx.guild_id().unwrap();
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

    let guild_id = ctx.guild_id().unwrap();
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

/// Displays the current queue
#[poise::command(slash_command, prefix_command, guild_only)]
pub async fn show_queue(ctx: Context<'_>) -> Result<(), crack_types::Error> {
    // Immediately acknowledge the interaction to prevent timeout
    ctx.defer().await?;

    ctx.say("Not yet implemented").await?;

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
