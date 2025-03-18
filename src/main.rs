//! Example demonstrating how to make use of individual track audio events,
//! and how to use the custom `CrackTrackQueue` system with poise.
//!
//! Requires the "cache", "voice", and "poise" features be enabled in your
//! Cargo.toml.
use std::{
    env,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use reqwest::Client as HttpClient;

use poise::serenity_prelude as serenity;
use serenity::{
    async_trait,
    http::Http,
    model::{gateway::Ready, prelude::ChannelId},
    prelude::{GatewayIntents, Mentionable},
    Result as SerenityResult,
};

use songbird::{
    input::YoutubeDl, Event, EventContext, EventHandler as VoiceEventHandler, TrackEvent,
};

use cracktunes::{CrackTrackQueue, ResolvedTrack};
use crack_types::QueryType;

// Define the user data structure that will be available in all command contexts
struct Data {
    songbird: Arc<songbird::Songbird>,
    http_client: HttpClient,
    // Map of guild IDs to queues
    guild_queues: dashmap::DashMap<serenity::GuildId, CrackTrackQueue>,
}

// Define the context type for poise
type Context<'a> = poise::Context<'a, Data, serenity::Error>;

struct Handler;

#[async_trait]
impl serenity::EventHandler for Handler {
    async fn ready(&self, _: serenity::Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

// Helper function to get or create a queue for a guild
async fn get_queue(ctx: Context<'_>) -> Result<CrackTrackQueue, String> {
    let guild_id = ctx.guild_id().ok_or("Not in a guild")?;
    let queues = &ctx.data().guild_queues;

    if !queues.contains_key(&guild_id) {
        queues.insert(guild_id, CrackTrackQueue::new());
    }

    Ok(queues.get(&guild_id).unwrap().clone())
}

/// Joins the voice channel of the user
#[poise::command(slash_command, prefix_command, guild_only)]
async fn join(ctx: Context<'_>) -> Result<(), serenity::Error> {
    let guild = ctx.guild().unwrap().clone();
    let guild_id = guild.id;

    let channel_id = guild
        .voice_states
        .get(&ctx.author().id)
        .and_then(|voice_state| voice_state.channel_id);

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            ctx.say("Not in a voice channel").await?;
            return Ok(());
        },
    };

    let manager = ctx.data().songbird.clone();

    if let Ok(handle_lock) = manager.join(guild_id, connect_to).await {
        ctx.say(format!("Joined {}", connect_to.mention())).await?;

        let chan_id = ctx.channel_id();
        let send_http = ctx.serenity_context().http.clone();

        let mut handle = handle_lock.lock().await;

        handle.add_global_event(
            Event::Track(TrackEvent::End),
            TrackEndNotifier {
                chan_id,
                http: send_http.clone(),
            },
        );

        handle.add_global_event(
            Event::Periodic(Duration::from_secs(60), None),
            ChannelDurationNotifier {
                chan_id,
                count: Default::default(),
                http: send_http,
            },
        );
    } else {
        ctx.say("Error joining the channel").await?;
    }

    Ok(())
}

/// Leaves the voice channel
#[poise::command(slash_command, prefix_command, guild_only)]
async fn leave(ctx: Context<'_>) -> Result<(), serenity::Error> {
    let guild_id = ctx.guild_id().unwrap();
    let manager = ctx.data().songbird.clone();

    if manager.get(guild_id).is_some() {
        if let Err(e) = manager.remove(guild_id).await {
            ctx.say(format!("Failed: {:?}", e)).await?;
        } else {
            ctx.say("Left voice channel").await?;
        }
    } else {
        ctx.say("Not in a voice channel").await?;
    }

    Ok(())
}

/// Plays a song with a fade effect
#[poise::command(slash_command, prefix_command, guild_only)]
async fn play_fade(
    ctx: Context<'_>,
    #[description = "URL to a video or audio"] url: String,
) -> Result<(), serenity::Error> {
    if !url.starts_with("http") {
        ctx.say("Must provide a valid URL").await?;
        return Ok(());
    }

    let guild_id = ctx.guild_id().unwrap();
    let data = ctx.data();

    if let Some(handler_lock) = data.songbird.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        let src = YoutubeDl::new(data.http_client.clone(), url);

        // This handler object will allow you to, as needed,
        // control the audio track via events and further commands.
        let song = handler.play_input(src.into());
        let send_http = ctx.serenity_context().http.clone();
        let chan_id = ctx.channel_id();

        // This shows how to periodically fire an event, in this case to
        // periodically make a track quieter until it can be no longer heard.
        let _ = song.add_event(
            Event::Periodic(Duration::from_secs(5), Some(Duration::from_secs(7))),
            SongFader {
                chan_id,
                http: send_http.clone(),
            },
        );

        // This shows how to fire an event once an audio track completes,
        // either due to hitting the end of the bytestream or stopped by user code.
        let _ = song.add_event(
            Event::Track(TrackEvent::End),
            SongEndNotifier {
                chan_id,
                http: send_http,
            },
        );

        ctx.say("Playing song").await?;
    } else {
        ctx.say("Not in a voice channel to play in").await?;
    }

    Ok(())
}

/// Adds a song to the queue
#[poise::command(slash_command, prefix_command, guild_only)]
async fn queue(
    ctx: Context<'_>,
    #[description = "URL to a video or audio"] url: String,
) -> Result<(), serenity::Error> {
    if !url.starts_with("http") {
        ctx.say("Must provide a valid URL").await?;
        return Ok(());
    }

    let guild_id = ctx.guild_id().unwrap();
    let data = ctx.data();

    // Get the custom queue for this guild
    let queue = get_queue(ctx).await.map_err(|e| {
        println!("Error getting queue: {}", e);
        serenity::Error::Other("Failed to get queue")
    })?;

    if let Some(handler_lock) = data.songbird.get(guild_id) {
        let _handler = handler_lock.lock().await;

        // Create a resolved track from the URL
        let query = QueryType::VideoLink(url);
        let track = ResolvedTrack::new(query).with_user_id(ctx.author().id);

        // Add to our custom queue
        queue.enqueue(track.clone()).await;

        // Also add to songbird's queue for playback
        //let src = YoutubeDl::new(data.http_client.clone(), track.get_url());
        //handler.enqueue_input(src.into()).await;

        // Build the display for the queue
        let mut queue_clone = queue.clone();
        queue_clone
            .build_display()
            .await
            .map_err(|_| serenity::Error::Other("Failed to build queue display"))?;

        let len = queue.len().await;
        ctx.say(format!(
            "Added song to queue: position {len:?}",
        ))
        .await?;
    } else {
        ctx.say("Not in a voice channel to play in").await?;
    }

    Ok(())
}

/// Skips the current song
#[poise::command(slash_command, prefix_command, guild_only)]
async fn skip(ctx: Context<'_>) -> Result<(), serenity::Error> {
    let guild_id = ctx.guild_id().unwrap();
    let manager = ctx.data().songbird.clone();

    if let Some(_handler_lock) = manager.get(guild_id) {
        // let handler = handler_lock.lock().await;
        // handler.
        //let queue = handler.queue();
        // let _ = queue.skip();

        // Also dequeue from our custom queue
        let custom_queue = get_queue(ctx).await.map_err(|e| {
            println!("Error getting queue: {}", e);
            serenity::Error::Other("Failed to get queue")
        })?;

        let _ = custom_queue.dequeue().await;

        let len = custom_queue.len().await;

        ctx.say(format!("Song skipped: {} in queue.", len))
            .await?;
    } else {
        ctx.say("Not in a voice channel to play in").await?;
    }

    Ok(())
}

/// Stops playback and clears the queue
#[poise::command(slash_command, prefix_command, guild_only)]
async fn stop(ctx: Context<'_>) -> Result<(), serenity::Error> {
    let guild_id = ctx.guild_id().unwrap();
    let manager = ctx.data().songbird.clone();

    if let Some(_handler_lock) = manager.get(guild_id) {
        // let handler = handler_lock.lock().await;
        // let queue = handler.queue();
        // queue.stop();

        // Also clear our custom queue
        let custom_queue = get_queue(ctx).await.map_err(|e| {
            println!("Error getting queue: {}", e);
            serenity::Error::Other("Failed to get queue")
        })?;

        custom_queue.clear().await;

        ctx.say("Queue cleared.").await?;
    } else {
        ctx.say("Not in a voice channel to play in").await?;
    }

    Ok(())
}

/// Displays the current queue
#[poise::command(slash_command, prefix_command, guild_only)]
async fn show_queue(ctx: Context<'_>) -> Result<(), serenity::Error> {
    let custom_queue = get_queue(ctx).await.map_err(|e| {
        println!("Error getting queue: {}", e);
        serenity::Error::Other("Failed to get queue")
    })?;

    let mut queue_clone = custom_queue.clone();
    queue_clone
        .build_display()
        .await
        .map_err(|_| serenity::Error::Other("Failed to build queue display"))?;

    let display = queue_clone.get_display();

    if display.is_empty() {
        ctx.say("The queue is empty.").await?;
    } else {
        ctx.say(format!("**Current Queue:**\n{}", display)).await?;
    }

    Ok(())
}

/// Shuffles the queue
#[poise::command(slash_command, prefix_command, guild_only)]
async fn shuffle(ctx: Context<'_>) -> Result<(), serenity::Error> {
    let guild_id = ctx.guild_id().unwrap();
    let manager = ctx.data().songbird.clone();

    if let Some(_handler_lock) = manager.get(guild_id) {
        // Get our custom queue
        let custom_queue = get_queue(ctx).await.map_err(|e| {
            println!("Error getting queue: {}", e);
            serenity::Error::Other("Failed to get queue")
        })?;

        // Shuffle our custom queue
        custom_queue.shuffle().await;

        // We need to rebuild the songbird queue to match our shuffled queue
        // let mut handler = handler_lock.lock().await;
        // let songbird_queue = handler.queue();
        // songbird_queue.stop();

        // Get the tracks from our custom queue
        //let tracks = custom_queue.get_queue().await;

        // Re-add all tracks to songbird queue
        // for track in tracks {
        //     let src = YoutubeDl::new(ctx.data().http_client.clone(), track.get_url());
        //     handler.enqueue_input(src.into()).await;
        // }

        // Build the display for the queue
        let mut queue_clone = custom_queue.clone();
        queue_clone
            .build_display()
            .await
            .map_err(|_| serenity::Error::Other("Failed to build queue display"))?;

        ctx.say("Queue shuffled!").await?;
    } else {
        ctx.say("Not in a voice channel.").await?;
    }

    Ok(())
}

/// Pings the bot
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
        } else {
            if let Err(e) = handler.mute(true).await {
                ctx.say(format!("Failed: {:?}", e)).await?;
            } else {
                ctx.say("Now muted").await?;
            }
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
            ctx.say(format!("Failed: {:?}", e)).await?;
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
        } else {
            if let Err(e) = handler.deafen(true).await {
                ctx.say(format!("Failed: {:?}", e)).await?;
            } else {
                ctx.say("Deafened").await?;
            }
        }
    } else {
        ctx.say("Not in a voice channel").await?;
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
            ctx.say(format!("Failed: {:?}", e)).await?;
        } else {
            ctx.say("Undeafened").await?;
        }
    } else {
        ctx.say("Not in a voice channel to undeafen in").await?;
    }

    Ok(())
}

struct TrackEndNotifier {
    chan_id: ChannelId,
    http: Arc<Http>,
}

#[async_trait]
impl VoiceEventHandler for TrackEndNotifier {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(track_list) = ctx {
            check_msg(
                self.chan_id
                    .say(&self.http, &format!("Tracks ended: {}.", track_list.len()))
                    .await,
            );
        }

        None
    }
}

struct ChannelDurationNotifier {
    chan_id: ChannelId,
    count: Arc<AtomicUsize>,
    http: Arc<Http>,
}

#[async_trait]
impl VoiceEventHandler for ChannelDurationNotifier {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        let count_before = self.count.fetch_add(1, Ordering::Relaxed);
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

struct SongFader {
    chan_id: ChannelId,
    http: Arc<Http>,
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

struct SongEndNotifier {
    chan_id: ChannelId,
    http: Arc<Http>,
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

/// Checks that a message successfully sent; if not, then logs why to stdout.
fn check_msg(result: SerenityResult<serenity::Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;

    let manager = songbird::Songbird::serenity();

    let manager_clone = Arc::clone(&manager);
    // Set up the poise framework
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                ping(),
                join(),
                leave(),
                play_fade(),
                queue(),
                skip(),
                stop(),
                show_queue(),
                shuffle(),
                mute(),
                unmute(),
                deafen(),
                undeafen(),
            ],
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("~".into()),
                ..Default::default()
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {
                    songbird: Arc::clone(&manager_clone),
                    http_client: HttpClient::new(),
                    guild_queues: dashmap::DashMap::new(),
                })
            })
        })
        .build();

    let mut client = serenity::ClientBuilder::new(token, intents)
        .event_handler(Handler)
        .framework(framework)
        .voice_manager_arc(manager)
        .await
        .expect("Error creating client");

    let _ = client
        .start()
        .await
        .map_err(|why| println!("Client ended: {:?}", why));
}
