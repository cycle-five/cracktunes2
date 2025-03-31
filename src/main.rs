use std::{
    process::exit,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use ::serenity::all::Token;
use cracktunes::{get_reqwest_client, get_youtube_client, Connection, CrackData};
use dashmap::DashMap;
use poise::serenity_prelude as serenity;
use rand::seq::SliceRandom;
use serenity::{
    all::EventHandler,
    async_trait,
    client::Context as SerenityContext,
    prelude::{GatewayIntents, Mentionable},
    FullEvent,
};

use cracktunes::{
    event_handlers::{ChannelDurationNotifier, EnhancedTrackErrorNotifier},
    EnhancedTrackEndNotifier,
};

use crack_types::CrackedError;
use cracktunes::Data;
use songbird::{
    input::{Compose, YoutubeDl},
    tracks::Track,
    Event, TrackEvent,
};
use tracing::{debug, error, info};
// Define the context type for poise
type Context<'a> = poise::Context<'a, Data, serenity::Error>;

struct Handler {
    _data: Arc<Data>,
    commands: Vec<poise::Command<Data, serenity::Error>>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn dispatch(&self, ctx: &SerenityContext, event: &FullEvent) {
        match event {
            FullEvent::Ready { data_about_bot, .. } => {
                info!("{} is connected!", data_about_bot.user.name);

                if let Err(err) =
                    poise::builtins::register_globally(&ctx.http, &self.commands).await
                {
                    error!("Error registering commands: {}", err);
                } else {
                    let app_id = data_about_bot.application.id;
                    let commands = data_about_bot.application.flags;
                    let data_str = format!("{app_id} - {commands:?}");
                    info!("Successfully registered commands: {data_str}");
                }
                // TODO: Load guilds from the database for persistent configurations
            }
            FullEvent::Resume { .. } => {
                // Log at the DEBUG level.
                debug!("Resumed");
            }
            _ => {}
        }
    }
}

/// Joins the voice channel of the user
#[poise::command(slash_command, prefix_command, guild_only)]
async fn join(ctx: Context<'_>) -> Result<(), serenity::Error> {
    let guild = ctx.guild().unwrap().clone();
    let guild_id = guild.id;

    let user_id = ctx.author().id;
    let bot_id = ctx.http().get_current_user().await?.id;
    let conn_info = cracktunes::check_voice_connections(&guild, &user_id, &bot_id);

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

    let manager = ctx.data().songbird.clone();

    if let Ok(handle_lock) = manager.join(guild_id, connect_to).await {
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
                info.last_activity
                    .store(0, std::sync::atomic::Ordering::Relaxed);
            })
            .or_insert_with(|| {
                let info = cracktunes::IdleTimeoutInfo::default();
                info.last_activity
                    .store(0, std::sync::atomic::Ordering::Relaxed);
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
async fn leave(ctx: Context<'_>) -> Result<(), serenity::Error> {
    let guild_id = ctx.guild_id().ok_or(CrackedError::from("No guild ID?"))?;
    let manager = ctx.data().songbird.clone();

    if let Some(handle_lock) = manager.get(guild_id) {
        // Remove all global events
        handle_lock.lock().await.remove_all_global_events();

        if let Err(e) = manager.remove(guild_id).await {
            ctx.say(format!("Failed: {e:?}")).await?;
        } else {
            ctx.say("Left voice channel").await?;
        }
    } else {
        ctx.say("Not in a voice channel").await?;
    }

    Ok(())
}

/// Plays a track from a URL
#[poise::command(slash_command, prefix_command, guild_only)]
async fn play(
    ctx: Context<'_>,
    #[description = "URL to media or search term"] url: String,
) -> Result<(), serenity::Error> {
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
        let metadata = src.aux_metadata().await.unwrap_or_default();
        // We store the user ID in the track data and the track title in the metadata
        let track_data = Arc::new(cracktunes::TrackMetadata {
            requesting_user_id,
            requesting_user,
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

        tracing::info!("State: {}", state);

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
async fn skip(ctx: Context<'_>) -> Result<(), serenity::Error> {
    // Immediately acknowledge the interaction to prevent timeout
    ctx.defer().await?;

    let guild_id = ctx.guild_id().unwrap();
    let manager = ctx.data().songbird.clone();

    if let Some(handler_lock) = manager.get(guild_id) {
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
async fn stop(ctx: Context<'_>) -> Result<(), serenity::Error> {
    // Immediately acknowledge the interaction to prevent timeout
    ctx.defer().await?;

    let guild_id = ctx.guild_id().unwrap();
    let manager = ctx.data().songbird.clone();

    if let Some(handler_lock) = manager.get(guild_id) {
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
async fn show_queue(ctx: Context<'_>) -> Result<(), serenity::Error> {
    // Immediately acknowledge the interaction to prevent timeout
    ctx.defer().await?;

    ctx.say("Not yet implemented").await?;

    Ok(())
}

/// Shuffles the queue
#[poise::command(slash_command, prefix_command, guild_only)]
async fn shuffle(ctx: Context<'_>) -> Result<(), serenity::Error> {
    // Immediately acknowledge the interaction to prevent timeout
    ctx.defer().await?;

    let guild_id = ctx.guild_id().unwrap();
    let manager = ctx.data().songbird.clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;

        handler.queue().current_queue().shuffle(&mut rand::rng());

        ctx.say("Queue shuffled!").await?;
    } else {
        ctx.say("Not in a voice channel.").await?;
    }

    Ok(())
}

/// Pings the bot
#[tracing::instrument(skip(ctx))]
#[poise::command(slash_command, prefix_command)]
async fn ping(ctx: Context<'_>) -> Result<(), serenity::Error> {
    ctx.say("Pong!").await?;
    Ok(())
}

/// Mutes the bot
#[poise::command(slash_command, prefix_command, guild_only)]
async fn mute(ctx: Context<'_>) -> Result<(), serenity::Error> {
    let guild_id = ctx.guild_id().unwrap();
    let manager = ctx.data().songbird.clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        if handler.is_mute() {
            ctx.say("Already muted").await?;
        } else if let Err(e) = handler.mute(true).await {
            ctx.say(format!("Failed: {e:?}")).await?;
        } else {
            ctx.say("Now muted").await?;
        }
    } else {
        ctx.say("Not in a voice channel").await?;
    }

    Ok(())
}

/// Unmutes the bot
#[poise::command(slash_command, prefix_command, guild_only)]
async fn unmute(ctx: Context<'_>) -> Result<(), serenity::Error> {
    let guild_id = ctx.guild_id().unwrap();
    let manager = ctx.data().songbird.clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        if let Err(e) = handler.mute(false).await {
            ctx.say(format!("Failed: {e:?}")).await?;
        } else {
            ctx.say("Unmuted").await?;
        }
    } else {
        ctx.say("Not in a voice channel to unmute in").await?;
    }

    Ok(())
}

/// Deafens the bot
#[poise::command(slash_command, prefix_command, guild_only)]
async fn deafen(ctx: Context<'_>) -> Result<(), serenity::Error> {
    let guild_id = ctx.guild_id().unwrap();
    let manager = ctx.data().songbird.clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        if handler.is_deaf() {
            ctx.say("Already deafened").await?;
        } else if let Err(e) = handler.deafen(true).await {
            ctx.say(format!("Failed: {e:?}")).await?;
        } else {
            ctx.say("Deafened").await?;
        }
    } else {
        ctx.say("Not in a voice channel").await?;
    }

    Ok(())
}

/// Sets the idle timeout in minutes (0 = never leave)
#[poise::command(slash_command, prefix_command, guild_only)]
async fn set_idle_timeout(
    ctx: Context<'_>,
    #[description = "Timeout in minutes (0 = never leave)"] minutes: usize,
) -> Result<(), serenity::Error> {
    let guild_id = ctx.guild_id().unwrap();

    // Get or create the idle timeout info for this guild
    let _ = ctx
        .data()
        .idle_timeouts
        .entry(guild_id)
        .and_modify(|info| {
            // Update the timeout
            info.timeout_minutes
                .store(minutes, std::sync::atomic::Ordering::Relaxed);
        })
        .or_insert_with(|| {
            let info = cracktunes::IdleTimeoutInfo::default();
            info.timeout_minutes
                .store(minutes, std::sync::atomic::Ordering::Relaxed);
            info
        });

    if minutes == 0 {
        ctx.say("Idle timeout disabled. Bot will not automatically leave the channel.")
            .await?;
    } else {
        ctx.say(format!("Idle timeout set to {minutes} minutes."))
            .await?;
    }

    Ok(())
}

/// Undeafens the bot
#[poise::command(slash_command, prefix_command, guild_only)]
async fn undeafen(ctx: Context<'_>) -> Result<(), serenity::Error> {
    let guild_id = ctx.guild_id().unwrap();
    let manager = ctx.data().songbird.clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        if let Err(e) = handler.deafen(false).await {
            ctx.say(format!("Failed: {e:?}")).await?;
        } else {
            ctx.say("Undeafened").await?;
        }
    } else {
        ctx.say("Not in a voice channel!").await?;
    }

    Ok(())
}

/// Define commands
fn get_commands() -> Vec<poise::Command<Data, serenity::Error>> {
    vec![
        ping(),
        join(),
        leave(),
        play(),
        skip(),
        stop(),
        show_queue(),
        shuffle(),
        mute(),
        unmute(),
        deafen(),
        undeafen(),
        set_idle_timeout(),
    ]
}

#[allow(clippy::too_many_lines)]
#[tokio::main]
async fn main() {
    // Initialize the enhanced logging system
    if let Err(e) = cracktunes::logging::init() {
        eprintln!("Failed to initialize logging: {e}");
    }

    // Configure the client with your Discord bot token in the environment.
    let token = Token::from_env("DISCORD_TOKEN").expect("Expected a token in the environment");

    let intents = GatewayIntents::non_privileged();

    let manager: Arc<songbird::Songbird> = songbird::Songbird::serenity();
    let manager_clone: Arc<songbird::Songbird> = Arc::clone(&manager);

    let req_client = get_reqwest_client();
    let yt_client = get_youtube_client();

    // Create the CrackTrackClient and wrap it in Data
    let client_data = Data(CrackData {
        req_client,
        yt_client,
        songbird: manager_clone,
        idle_timeouts: DashMap::default(),
    });

    // Set up the poise framework with command hooks for logging
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: get_commands(),
            // Add pre-command hook for logging command start
            pre_command: |ctx| {
                Box::pin(async move {
                    cracktunes::logging::log_command_start(ctx);
                })
            },
            // Add post-command hook for logging command end
            post_command: |ctx| {
                Box::pin(async move {
                    cracktunes::logging::log_command_end(ctx);
                })
            },
            // Maybe one day
            // prefix_options: poise::PrefixFrameworkOptions {
            //     prefix: Some("~".into()),
            //     ..Default::default()
            // },
            on_error: |error| {
                Box::pin(async move {
                    // Log the error using our logging system
                    cracktunes::logging::log_command_error(&error);

                    // Still handle the error for user feedback
                    match error {
                        poise::FrameworkError::Command { error, ctx, .. } => {
                            let cmd_name = &ctx.command().name;
                            error!("Error in command `{cmd_name}`: {error:?}");

                            if let Err(e) = ctx.say(format!("An error occurred: {error}")).await {
                                error!("Error while sending error message: {e:?}");
                            }
                        }
                        poise::FrameworkError::CommandCheckFailed { error, ctx, .. } => {
                            error!("Command check failed: {error:?}");

                            if let Some(error) = error {
                                if let Err(e) =
                                    ctx.say(format!("Command check failed: {error}")).await
                                {
                                    error!("Error while sending check failure message: {:?}", e);
                                }
                            }
                        }
                        err => {
                            error!("Other framework error: {:?}", err);
                        }
                    }
                })
            },
            ..Default::default()
        })
        .build();

    let arc_data = Arc::new(client_data);
    // Create an event handler that will register commands and has access to the data
    let handler = Handler {
        _data: arc_data.clone(),
        commands: get_commands(),
    };

    let mut client = serenity::ClientBuilder::new(token, intents)
        .data::<Data>(arc_data)
        .event_handler(handler)
        .framework(framework)
        .voice_manager::<songbird::Songbird>(manager)
        .await
        .expect("Error creating client");

    tokio::spawn(async move {
        info!("Starting client");
        let _ = client
            .start_autosharded()
            .await
            .map_err(|why| println!("Client ended: {why:?}"));
    });

    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix as signal;

            let [mut s1, mut s2, mut s3] = [
                signal::signal(signal::SignalKind::hangup()).unwrap(),
                signal::signal(signal::SignalKind::interrupt()).unwrap(),
                signal::signal(signal::SignalKind::terminate()).unwrap(),
            ];

            tokio::select!(
                v = s1.recv() => v.unwrap(),
                v = s2.recv() => v.unwrap(),
                v = s3.recv() => v.unwrap(),
            );
        }
        #[cfg(windows)]
        {
            let (mut s1, mut s2) = (
                tokio::signal::windows::ctrl_c().unwrap(),
                tokio::signal::windows::ctrl_break().unwrap(),
            );

            tokio::select!(
                v = s1.recv() => v.unwrap(),
                v = s2.recv() => v.unwrap(),
            );
        }

        info!("Received shutdown signal");
        exit(0);
    });
}
