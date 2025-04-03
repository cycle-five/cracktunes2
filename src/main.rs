#![feature(iter_chain)]
use std::{iter::chain, process::exit, sync::Arc};

use ::serenity::all::Token;
use cracktunes::{all_music_commands, get_reqwest_client, get_youtube_client, CrackData};
use dashmap::DashMap;
use poise::serenity_prelude as serenity;
use serenity::{
    all::EventHandler, async_trait, client::Context as SerenityContext, prelude::GatewayIntents,
    FullEvent,
};

use cracktunes::Data;
use tracing::{debug, error, info};
// Define the context type for poise
pub type Context<'a> = poise::Context<'a, Data, crack_types::Error>;

struct Handler {
    _data: Arc<Data>,
    commands: Vec<poise::Command<Data, crack_types::Error>>,
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
                    let data_str = self
                        .commands
                        .iter()
                        .map(|cmd| format!("`{}`", cmd.name))
                        .collect::<Vec<_>>()
                        .join(", ");
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

/// Pings the bot
#[tracing::instrument(skip(ctx))]
#[poise::command(slash_command, prefix_command)]
async fn ping(ctx: Context<'_>) -> Result<(), crack_types::Error> {
    ctx.say("Pong!").await?;
    Ok(())
}

/// Mutes the bot
#[poise::command(slash_command, prefix_command, guild_only)]
async fn mute(ctx: Context<'_>) -> Result<(), crack_types::Error> {
    let guild_id = ctx.guild_id().unwrap();
    let songbird = ctx.data().songbird.clone();

    if let Some(handler_lock) = songbird.get(guild_id) {
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
async fn unmute(ctx: Context<'_>) -> Result<(), crack_types::Error> {
    let guild_id = ctx.guild_id().unwrap();
    let songbird = ctx.data().songbird.clone();

    if let Some(handler_lock) = songbird.get(guild_id) {
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
async fn deafen(ctx: Context<'_>) -> Result<(), crack_types::Error> {
    let guild_id = ctx.guild_id().unwrap();
    let songbird = ctx.data().songbird.clone();

    if let Some(handler_lock) = songbird.get(guild_id) {
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
) -> Result<(), crack_types::Error> {
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
async fn undeafen(ctx: Context<'_>) -> Result<(), crack_types::Error> {
    let guild_id = ctx.guild_id().unwrap();
    let songbird = ctx.data().songbird.clone();

    if let Some(handler_lock) = songbird.get(guild_id) {
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
fn get_commands() -> Vec<poise::Command<Data, crack_types::Error>> {
    chain(
        vec![
            ping(),
            mute(),
            unmute(),
            deafen(),
            undeafen(),
            set_idle_timeout(),
        ],
        all_music_commands(),
    )
    .collect()
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

    let songbird: Arc<songbird::Songbird> = songbird::Songbird::serenity();
    let songbird_clone: Arc<songbird::Songbird> = Arc::clone(&songbird);

    let req_client = get_reqwest_client();
    let yt_client = get_youtube_client();

    // Create the CrackTrackClient and wrap it in Data
    let client_data = Data(CrackData {
        req_client,
        yt_client,
        songbird: songbird_clone,
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
        .voice_manager::<songbird::Songbird>(songbird)
        .await
        .expect("Error creating client");

    info!("Starting signal handlers");
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

    info!("Starting client");
    let _ = client
        .start_autosharded()
        .await
        .map_err(|why| println!("Client ended: {why:?}"));
}
