//! Example demonstrating how to make use of individual track audio events,
//! and how to use the custom `CrackTrackQueue` system with poise.
//!
//! Requires the "cache", "voice", and "poise" features be enabled in your
//! Cargo.toml.
use std::{
    env, sync::{
        atomic::AtomicBool, Arc
    }, time::Duration
};

use reqwest::Client as HttpClient;
use poise::serenity_prelude as serenity;
use serenity::{
    async_trait,
    model::gateway::Ready,
    prelude::{GatewayIntents, Mentionable},
};

use cracktunes::{event_handlers::{ChannelDurationNotifier, EnhancedTrackErrorNotifier, SongEndNotifier, SongFader}, EnhancedTrackEndNotifier};

use songbird::{
    input::YoutubeDl, Call, Event, TrackEvent
};
use cracktunes::{check_msg, CrackTrackQueue, Data, DataInner, ResolvedTrack};
use crack_types::QueryType;
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

// Add this to improve the play_next_from_queue function to handle track failures
async fn play_next_from_queue(
    ctx: Context<'_>,
    queue: CrackTrackQueue,
    mut handler: Call,
) -> Result<(), serenity::Error> {
    // Get the next track from our custom queue
    if let Some(track) = queue.dequeue().await {
        // Try to play it with songbird
        // let src = match YoutubeDl::new(ctx.data().http_client.clone(), track.get_url()).into_input() {
        //     Ok(input) => input,
        //     Err(e) => {
        //         // Failed to create input for this track
        //         ctx.say(format!("Error playing track \"{}\": {}", track.get_title(), e)).await?;
                
        //         // Try the next track
        //         return play_next_from_queue(ctx, queue, handler).await;
        //     }
        // };
        let _data = Arc::new(ctx.data().clone());
        let src = YoutubeDl::new(ctx.data().http_client.clone(), track.get_url());
        
        let song = handler.play_input(src.into());
        
        // Update activity timestamp in any ChannelDurationNotifier
        for (_, event_handler) in handler.global_events.iter() {
            if let Some(notifier) = event_handler.downcast_ref::<ChannelDurationNotifier>() {
                notifier.update_activity();
            }
        }
        
        // Add the track end event to handle auto-playing the next song
        let chan_id = ctx.channel_id();
        let http = ctx.serenity_context().http.clone();
        
        let _ = song.add_event(
            Event::Track(TrackEvent::End),
            EnhancedTrackEndNotifier {
                chan_id,
                http: http.clone(),
                guild_id: ctx.guild_id().unwrap(),
                data: Arc::new(ctx.data().clone()),
                is_looping: Arc::new(AtomicBool::new(false)),
            },
        );
        
        // Also add an error handler to skip to next track on failure
        let _ = song.add_event(
            Event::Track(TrackEvent::Error),
            EnhancedTrackErrorNotifier {
                chan_id,
                http: http.clone(),
                guild_id: ctx.guild_id().unwrap(),
                data: Arc::new(ctx.data().clone()),
                is_looping: Arc::new(AtomicBool::new(false)),
            },
        );
        
        // Notify that the track is playing
        check_msg(
            chan_id
                .say(http.clone(), &format!("Now playing: {}", track.get_title()))
                .await,
        );
    }
    
    Ok(())
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

        // Create the channel duration notifier
        let notifier = ChannelDurationNotifier {
            chan_id,
            count: Default::default(),
            http: send_http,
            guild_id,
            songbird: ctx.data().songbird.clone(),
            idle_timeout: 5, // Default to 5 minutes of inactivity before leaving
            last_activity: Arc::new(AtomicUsize::new(0)),
        };
        
        // Update the last activity timestamp to the current time
        notifier.update_activity();
        
        // Add the notifier as a global event
        handle.add_global_event(
            Event::Periodic(Duration::from_secs(60), None),
            notifier,
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
        
        // Update activity timestamp in any ChannelDurationNotifier
        for (_, event_handler) in handler.global_events.iter() {
            if let Some(notifier) = event_handler.downcast_ref::<ChannelDurationNotifier>() {
                notifier.update_activity();
            }
        }
        
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
        let handler = handler_lock.lock().await;

        // Create a resolved track from the URL
        let query = QueryType::VideoLink(url);
        let track = ResolvedTrack::new(query).with_user_id(ctx.author().id);

        // Add to our custom queue
        queue.enqueue(track.clone()).await;

        // Check if we need to start playing (if this is the first track)
        let queue_len = queue.len().await;
        if queue_len == 1 {
            // This is the first track, so start playing
            play_next_from_queue(ctx, queue.clone(), handler.clone()).await?;
        }

        // Build the display for the queue
        let mut queue_clone = queue.clone();
        queue_clone
            .build_display()
            .await
            .map_err(|_| serenity::Error::Other("Failed to build queue display"))?;

        ctx.say(format!(
            "Added song to queue: position {queue_len}",
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

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        
        // Skip the current song in songbird's queue
        handler.stop();

        // Also dequeue from our custom queue
        let custom_queue = get_queue(ctx).await.map_err(|e| {
            println!("Error getting queue: {}", e);
            serenity::Error::Other("Failed to get queue")
        })?;

        let _ = custom_queue.dequeue().await;

        // Play the next song from our custom queue
        if !custom_queue.is_empty().await {
            play_next_from_queue(ctx, custom_queue.clone(), handler.clone()).await?;
        }

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

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        
        // Stop the songbird queue
        handler.stop();

        // Clear our custom queue
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

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        
        // Get our custom queue
        let custom_queue = get_queue(ctx).await.map_err(|e| {
            println!("Error getting queue: {}", e);
            serenity::Error::Other("Failed to get queue")
        })?;

        // Save the current playing track if there is one
        let current_track = if !custom_queue.is_empty().await {
            custom_queue.dequeue().await
        } else {
            None
        };

        // Shuffle our custom queue
        custom_queue.shuffle().await;

        // If we had a current track, put it back at the front
        if let Some(track) = current_track {
            custom_queue.push_front(track).await;
        }

        // We need to rebuild the songbird queue to match our shuffled queue
        handler.stop();

        // Play the next track from our shuffled queue
        if !custom_queue.is_empty().await {
            play_next_from_queue(ctx, custom_queue.clone(), handler.clone()).await?;
        }

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

/// Sets the idle timeout in minutes (0 = never leave)
#[poise::command(slash_command, prefix_command, guild_only)]
async fn set_idle_timeout(
    ctx: Context<'_>,
    #[description = "Timeout in minutes (0 = never leave)"] minutes: usize,
) -> Result<(), serenity::Error> {
    let guild_id = ctx.guild_id().unwrap();
    let manager = ctx.data().songbird.clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        
        // Find the ChannelDurationNotifier and update its idle_timeout
        let mut updated = false;
        for (_, event_handler) in handler.global_events.iter() {
            if let Some(notifier) = event_handler.downcast_ref::<ChannelDurationNotifier>() {
                // We can't directly modify the notifier because it's behind a shared reference
                // So we need to create a new one with the updated timeout
                let chan_id = notifier.chan_id;
                let count = notifier.count.clone();
                let http = notifier.http.clone();
                let guild_id = notifier.guild_id;
                let songbird = notifier.songbird.clone();
                let last_activity = notifier.last_activity.clone();
                
                // Remove the old event handler
                handler.remove_global_event(event_handler.uuid());
                
                // Add a new one with the updated timeout
                handler.add_global_event(
                    Event::Periodic(Duration::from_secs(60), None),
                    ChannelDurationNotifier {
                        chan_id,
                        count,
                        http,
                        guild_id,
                        songbird,
                        idle_timeout: minutes,
                        last_activity,
                    },
                );
                
                updated = true;
                break;
            }
        }
        
        if updated {
            if minutes == 0 {
                ctx.say("Idle timeout disabled. Bot will not automatically leave the channel.").await?;
            } else {
                ctx.say(format!("Idle timeout set to {} minutes.", minutes)).await?;
            }
        } else {
            ctx.say("No channel duration notifier found. Join a voice channel first.").await?;
        }
    } else {
        ctx.say("Not in a voice channel.").await?;
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

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let intents = GatewayIntents::non_privileged();

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
                set_idle_timeout(),
            ],
            // Maybe one day
            // prefix_options: poise::PrefixFrameworkOptions {
            //     prefix: Some("~".into()),
            //     ..Default::default()
            // },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data(DataInner{
                    songbird: Arc::clone(&manager_clone),
                    http_client: HttpClient::new(),
                    guild_queues: dashmap::DashMap::new(),
                }))
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
