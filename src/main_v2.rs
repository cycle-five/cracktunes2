#![feature(iter_chain)]

use poise::serenity_prelude as serenity;
use serenity::{
    all::{EventHandler, Http, Token},
    async_trait,
    client::Context as SerenityContext,
    prelude::GatewayIntents,
    FullEvent,
};
use std::{process::exit, sync::Arc};

use tracing::{debug, error, info};

use cracktunes::{
    commands::v2::get_all_v2_commands,
    core::models::error::AppError,
    infrastructure::setup::{initialize_services, initialize_services_with_poise, get_global_poise_handler, Data},
    logging::{log_command_start_v2, log_command_end_v2, log_command_error_v2},
    adapters::discord::poise_message_handler::PoiseMessageHandler,
};

struct Handler {
    commands: Vec<poise::Command<Data, AppError>>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn dispatch(&self, ctx: &SerenityContext, event: &FullEvent) {
        match event {
            FullEvent::Ready { data_about_bot, .. } => {
                info!("{} is connected!", data_about_bot.user.name);

                // Register commands globally
                if let Err(err) =
                    poise::builtins::register_globally(&ctx.http, &self.commands).await
                {
                    error!("Error registering commands: {}", err);
                } else {
                    let command_names = self
                        .commands
                        .iter()
                        .map(|cmd| format!("`{}`", cmd.name))
                        .collect::<Vec<_>>()
                        .join(", ");
                    info!("Successfully registered commands: {command_names}");
                }
            }
            FullEvent::Resume { .. } => {
                debug!("Resumed");
            }
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize logging
    cracktunes::logging::init()?;

    // Get Discord token from environment
    let token = Token::from_env("DISCORD_TOKEN").expect("Expected a token in the environment");

    // Set up gateway intents
    let intents = GatewayIntents::non_privileged();

    // Create Songbird voice client
    let songbird = songbird::Songbird::serenity();

    // Generate command lists for the event handler and framework
    let handler_commands = get_all_v2_commands();
    let framework_commands = get_all_v2_commands();

    // Create event handler
    let handler = Handler {
        commands: handler_commands,
    };

    // Set up the poise framework with command hooks for logging
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: framework_commands,
            // Add pre-command hook for logging command start and registering context
            pre_command: |ctx| {
                Box::pin(async move {
                    // Log command start
                    log_command_start_v2(ctx);
                    
                    // Register this channel with the global PoiseMessageHandler
                    if let Some(handler) = get_global_poise_handler() {
                        // Get the channel and guild IDs
                        let channel_id = ctx.channel_id().get();
                        let guild_id = ctx.guild_id().map(|id| id.get());
                        
                        // Register this channel with the handler
                        tokio::spawn(async move {
                            handler.register_channel(channel_id, guild_id).await;
                        });
                    } else {
                        debug!("No global PoiseMessageHandler available for context registration");
                    }
                })
            },
            // Add post-command hook for logging command end
            post_command: |ctx| {
                Box::pin(async move {
                    log_command_end_v2(ctx);
                })
            },
            on_error: |error| {
                Box::pin(async move {
                    // Log the error using our logging system
                    log_command_error_v2(&error);
                    
                    // Handle the error for user feedback
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

    // Initialize services with Poise message handler for better Discord integration
    let services = initialize_services_with_poise(Arc::new(Http::new(token.clone())), songbird.clone())?;

    // Create Data container for commands
    let data = Data::new(services);
    let arc_data = Arc::new(data);

    // Create the client with both the framework and regular event handlers
    let mut client = serenity::ClientBuilder::new(token, intents)
        .event_handler(handler)
        .framework(framework)
        .data(arc_data)
        .voice_manager::<songbird::Songbird>(songbird)
        .await?;

    // Set up signal handlers for graceful shutdown
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

    // Log the start of the client
    info!("Starting client");

    // Run the client with autosharding
    let _ = client
        .start_autosharded()
        .await
        .map_err(|why| error!("Client ended: {:?}", why));

    Ok(())
}
