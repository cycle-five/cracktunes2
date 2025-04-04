#![feature(iter_chain)]

use std::sync::Arc;

use poise::serenity_prelude as serenity;
use serenity::{all::{EventHandler, GuildId, Token}, async_trait, client::Context as SerenityContext, prelude::GatewayIntents, FullEvent};

use tracing::{debug, error, info};

// Import our modules
mod core;
mod adapters;
mod infrastructure;
mod commands;

use crate::{
    core::models::error::AppError,
    infrastructure::{
        error_handling::handle_error,
        setup::{Data, initialize_services},
    },
    commands::v2::get_all_v2_commands,
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
                if let Err(err) = poise::builtins::register_globally(&ctx.http, &self.commands).await {
                    error!("Error registering commands: {}", err);
                } else {
                    let command_names = self.commands
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

    // Set up the poise framework
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: get_all_v2_commands(),
            on_error: |error| {
                Box::pin(async move {
                    match error {
                        poise::FrameworkError::Command { error, ctx, .. } => {
                            handle_error(error, &ctx).await;
                        }
                        err => {
                            error!("Poise framework error: {:?}", err);
                        }
                    }
                })
            },
            // Configure additional options as needed
            ..Default::default()
        })
        .token(token.clone())
        .intents(intents)
        .client_settings(|client_builder| {
            client_builder.voice_manager_arc(songbird.clone())
        })
        .setup(|ctx, ready, framework| {
            Box::pin(async move {
                // Initialize our services
                let services = initialize_services(ctx.http.clone(), songbird.clone()).await?;
                
                // Create Data container for commands
                let data = Data::new(services);
                
                Ok(data)
            })
        });

    // Generate list of commands for registering
    let commands = get_all_v2_commands();
    
    // Create event handler
    let handler = Handler { commands };

    // Create and run the client
    let mut client = serenity::ClientBuilder::new(token, intents)
        .event_handler(handler)
        .framework(framework)
        .await?;

    // Log the start of the client
    info!("Starting bot...");
    
    // Run the client until it returns (on error or shutdown)
    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }

    Ok(())
}