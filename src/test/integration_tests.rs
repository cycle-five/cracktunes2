use std::{borrow::Cow, env, sync::Arc, time::Duration};

use ::serenity::all::Token;
use crack_types::CrackedError;
use dashmap::DashMap;
use poise::serenity_prelude as serenity;
use serenity::{
    all::{
        ChannelId, EventHandler, GatewayIntents, GenericChannelId, GuildId, Http, Message, Ready,
        ResumedEvent,
    },
    async_trait,
    client::Context as SerenityContext,
    Client,
};
use songbird::{Event, EventHandler as VoiceEventHandler, TrackEvent};
use tokio::{
    sync::{mpsc, Mutex},
    time::timeout,
};
use tracing::{debug, error, info};

// A simple audio receiver that listens for and analyzes audio data
pub struct AudioReceiver {
    data_channel: mpsc::Sender<Vec<i16>>,
}

impl AudioReceiver {
    pub fn new(data_channel: mpsc::Sender<Vec<i16>>) -> Self {
        Self { data_channel }
    }
}

#[async_trait]
impl VoiceEventHandler for AudioReceiver {
    async fn act(&self, ctx: &songbird::EventContext<'_>) -> Option<Event> {
        if let songbird::EventContext::RtcpPacket(voice_packet) = ctx {
            let audio_data = &voice_packet.packet;
            let audio_data = audio_data.iter().map(|&s| s as i16).collect::<Vec<i16>>();
            // Send the audio data through the channel for analysis
            let _ = self.data_channel.send(audio_data).await;
        }
        None
    }
}

// Audio analyzer that processes and verifies audio data
pub struct AudioAnalyzer {
    data_receiver: mpsc::Receiver<Vec<i16>>,
    volume_changes: Mutex<Vec<f32>>,
    detected_patterns: Mutex<Vec<String>>,
}

impl AudioAnalyzer {
    pub fn new(data_receiver: mpsc::Receiver<Vec<i16>>) -> Self {
        Self {
            data_receiver,
            volume_changes: Mutex::new(Vec::new()),
            detected_patterns: Mutex::new(Vec::new()),
        }
    }

    // Start the audio analysis process
    pub async fn start_analysis(&mut self) {
        while let Some(audio_data) = self.data_receiver.recv().await {
            self.analyze_audio_data(&audio_data).await;
        }
    }

    // Basic audio analysis
    async fn analyze_audio_data(&self, audio_data: &[i16]) {
        // Calculate average volume level
        if !audio_data.is_empty() {
            let sum: i64 = audio_data.iter().map(|&s| i64::from(s.abs())).sum();
            let avg_volume = sum as f32 / audio_data.len() as f32 / i16::MAX as f32;

            // Store volume level for later verification
            let mut volumes = self.volume_changes.lock().await;
            volumes.push(avg_volume);

            // Detect significant changes in volume (for volume command testing)
            if volumes.len() > 1
                && (volumes[volumes.len() - 1] / volumes[volumes.len() - 2]).abs() > 1.5
            {
                let mut patterns = self.detected_patterns.lock().await;
                patterns.push("volume_change".to_string());
            }

            // Here we could add more sophisticated audio pattern detection
            // For example, detect specific sound clips, silence, etc.
        }
    }

    // Get the detected volume changes
    pub async fn get_volume_changes(&self) -> Vec<f32> {
        self.volume_changes.lock().await.clone()
    }

    // Get the detected audio patterns
    pub async fn get_detected_patterns(&self) -> Vec<String> {
        self.detected_patterns.lock().await.clone()
    }
}

// Helper struct to manage test expectations and results
#[derive(Clone)]
pub struct TestExpectation {
    command: String,
    expected_response: String,
    actual_response: Option<String>,
    passed: bool,
}

impl TestExpectation {
    pub fn new(command: &str, expected_response: &str) -> Self {
        Self {
            command: command.to_string(),
            expected_response: expected_response.to_string(),
            actual_response: None,
            passed: false,
        }
    }

    pub fn set_response(&mut self, response: String) {
        self.passed = response.contains(&self.expected_response);
        self.actual_response = Some(response);
    }
}

// Test bot handler that interacts with the CrackTunes bot
pub struct TestHandler {
    http: Arc<Http>,
    songbird: Arc<songbird::Songbird>,
    runtime: Arc<tokio::runtime::Runtime>,
    music_channel_id: Mutex<Option<GenericChannelId>>,
    voice_channel_id: Mutex<Option<ChannelId>>,
    target_bot_id: Mutex<Option<serenity::UserId>>,
    test_expectations: DashMap<String, TestExpectation>,
    audio_analyzer: Mutex<Option<Arc<AudioAnalyzer>>>,
    guild_id: Mutex<Option<GuildId>>,
}

impl TestHandler {
    pub fn new(http: Arc<Http>, songbird: Arc<songbird::Songbird>) -> Self {
        Self {
            http,
            songbird,
            runtime: Arc::new(tokio::runtime::Runtime::new().unwrap()),
            music_channel_id: Mutex::new(None),
            voice_channel_id: Mutex::new(None),
            target_bot_id: Mutex::new(None),
            test_expectations: DashMap::new(),
            audio_analyzer: Mutex::new(None),
            guild_id: Mutex::new(None),
        }
    }

    // Set the target bot ID
    pub async fn set_target_bot_id(&self, bot_id: serenity::UserId) {
        *self.target_bot_id.lock().await = Some(bot_id);
    }

    // Set the music channel ID for text commands and responses
    pub async fn set_music_channel_id(&self, channel_id: GenericChannelId) {
        *self.music_channel_id.lock().await = Some(channel_id);
    }

    // Set the voice channel ID for voice tests
    pub async fn set_voice_channel_id(&self, channel_id: ChannelId) {
        *self.voice_channel_id.lock().await = Some(channel_id);
    }

    // Set the guild ID
    pub async fn set_guild_id(&self, guild_id: GuildId) {
        *self.guild_id.lock().await = Some(guild_id);
    }

    // Initialize audio analysis
    pub async fn init_audio_analysis(&self) -> mpsc::Sender<Vec<i16>> {
        let (tx, rx) = mpsc::channel(100);
        let analyzer = AudioAnalyzer::new(rx);
        *self.audio_analyzer.lock().await = Some(Arc::new(analyzer));
        tx
    }

    // Add a test expectation
    pub async fn add_expectation(&self, test_id: &str, command: &str, expected_response: &str) {
        let expectation = TestExpectation::new(command, expected_response);
        self.test_expectations
            .insert(test_id.to_string(), expectation);
    }

    // Get test results
    pub async fn get_test_results(&self) -> Vec<(String, TestExpectation)> {
        let map = self.test_expectations.clone();
        map.iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    // Sync state from another TestHandler instance
    // This is useful for updating a clone with the current state from the original
    pub async fn sync_from(&self, other: &TestHandler) -> Result<(), ()> {
        // Clone the values from other's mutexes to update our own
        if let Some(music_channel_id) = *other.music_channel_id.lock().await {
            *self.music_channel_id.lock().await = Some(music_channel_id);
        }

        if let Some(voice_channel_id) = *other.voice_channel_id.lock().await {
            *self.voice_channel_id.lock().await = Some(voice_channel_id);
        }

        if let Some(target_bot_id) = *other.target_bot_id.lock().await {
            *self.target_bot_id.lock().await = Some(target_bot_id);
        }

        if let Some(audio_analyzer) = other.audio_analyzer.lock().await.clone() {
            *self.audio_analyzer.lock().await = Some(audio_analyzer);
        }

        if let Some(guild_id) = *other.guild_id.lock().await {
            *self.guild_id.lock().await = Some(guild_id);
        }

        // No need to sync test_expectations as they're shared through DashMap

        Ok(())
    }

    // Send a text command to the music channel
    pub async fn send_text_command(&self, command: &str) -> Result<Message, serenity::Error> {
        if let Some(channel_id) = *self.music_channel_id.lock().await {
            channel_id.say(&self.http, command).await
        } else {
            Err(CrackedError::from("Music channel not set").into())
        }
    }

    // Send an application (slash) command to the target bot
    pub async fn send_slash_command(
        &self,
        command_name: &str,
        options: Vec<(String, String)>,
    ) -> Result<(), serenity::Error> {
        let guild_id = match *self.guild_id.lock().await {
            Some(id) => id,
            None => return Err(CrackedError::Other("Guild ID not set".into()).into()),
        };

        let channel_id = match *self.music_channel_id.lock().await {
            Some(id) => id,
            None => return Err(CrackedError::Other("Music channel not set".into()).into()),
        };

        let target_bot_id = match *self.target_bot_id.lock().await {
            Some(id) => id,
            None => return Err(CrackedError::Other("Target bot ID not set".into()).into()),
        };

        // First, retrieve the application commands available from the target bot
        let application_commands = self.http.get_guild_commands(guild_id).await?;

        // Find the command by name
        let command = application_commands
            .iter()
            .find(|cmd| cmd.name == command_name)
            .ok_or_else(|| {
                CrackedError::Other(format!("Command '{}' not found", command_name).into())
            })?;

        // Build the command options
        let mut command_options = Vec::new();
        for (name, value) in options {
            // Create application command option data
            // This is a simplified version - you would need to handle different types of options
            let option = serde_json::json!({
                "name": name,
                "value": value
            });
            command_options.push(option);
        }

        // Create the interaction data
        let interaction_data = serde_json::json!({
            "type": 2, // 2 is APPLICATION_COMMAND
            "application_id": target_bot_id,
            "guild_id": guild_id,
            "channel_id": channel_id,
            "data": {
                "id": command.id,
                "name": command.name,
                "type": 1, // 1 for CHAT_INPUT
                "options": command_options
            }
        });

        // Send the interaction
        // Note: This is a simplified approach. In reality, interactions require proper
        // cryptographic signing which Discord's API validates. This direct approach
        // might not work with Discord's production API.
        let interaction_endpoint = "/api/v10/interactions".to_string();

        let url = format!("https://discord.com{}", interaction_endpoint)
            .parse::<url::Url>()
            .unwrap();
        // This is a direct HTTP approach, but Discord will likely reject it
        // without proper interaction signing
        //.header("Authorization", format!("Bot {}", self.http.token()))
        reqwest::Client::new()
            .post(url)
            .json(&interaction_data)
            .send()
            .await?;

        // Alternative approach: Use Discord's interaction system
        // This would typically require setting up an interaction server
        // with proper webhook endpoints that Discord can call

        Ok(())
    }

    // Combined method that sends either a text command or slash command based on the prefix
    pub async fn send_command(&self, command: &str) -> Result<Message, serenity::Error> {
        // Check if it's a slash command (starts with '/')
        if command.starts_with('/') {
            // Extract command name and options
            let parts: Vec<&str> = command[1..].split_whitespace().collect();
            if parts.is_empty() {
                return Err(CrackedError::Other(Cow::Owned("Empty comman".into())).into());
            }

            let command_name = parts[0];
            let mut options = Vec::new();

            // Very basic option parsing
            // This should be improved for real production use
            let mut i = 1;
            while i < parts.len() {
                if parts[i].contains(':') {
                    let option_parts: Vec<&str> = parts[i].splitn(2, ':').collect();
                    if option_parts.len() == 2 {
                        options.push((option_parts[0].to_string(), option_parts[1].to_string()));
                    }
                } else if i + 1 < parts.len() {
                    // Assume it's a key followed by a value
                    options.push((parts[i].to_string(), parts[i + 1].to_string()));
                    // Skip the next part as we used it as a value
                    i += 1;
                }
                i += 1;
            }

            // Send the slash command
            self.send_slash_command(command_name, options).await?;

            // For consistency with text commands, return a placeholder message
            // since slash commands don't return messages directly
            Ok(Message::default())
        } else {
            // It's a regular text command
            self.send_text_command(command).await
        }
    }

    // Join a voice channel
    pub async fn join_voice_channel(&self) -> Result<(), String> {
        let guild_id = match *self.guild_id.lock().await {
            Some(id) => id,
            None => return Err("Guild ID not set".to_string()),
        };

        let connect_to = match *self.voice_channel_id.lock().await {
            Some(id) => id,
            None => return Err("Voice channel not set".to_string()),
        };

        // Join the voice channel
        let manager = self.songbird.clone();

        let (audio_tx, _audio_rx) = mpsc::channel(100);
        let receiver = AudioReceiver::new(audio_tx);

        // Create a voice connection
        if let Ok(handler_lock) = manager.join(guild_id, connect_to).await {
            // Enable the voice receiver
            let mut handler = handler_lock.lock().await;
            handler.add_global_event(Event::Core(songbird::CoreEvent::RtcpPacket), receiver);

            // Get the current bot ID
            if let Some(bot_id) = *self.target_bot_id.lock().await {
                // Listen for specific events from the target bot
                // Create a clone of self to use in the handler
                let test_handler_clone = Arc::new(self.clone());
                let source_handler = Arc::new(self.clone());

                // Use the updated TrackEndNotifier with both the cloned and source handlers
                handler.add_global_event(
                    Event::Track(TrackEvent::End),
                    TrackEndNotifier {
                        test_handler: test_handler_clone,
                        source_handler,
                        target_bot_id: bot_id,
                    },
                );
            }

            Ok(())
        } else {
            Err("Could not join voice channel".to_string())
        }
    }

    // Helper method to send a specific CrackTunes command using slash commands
    pub async fn send_cracktunes_command(
        &self,
        command_type: &str,
        options: Vec<(String, String)>,
    ) -> Result<(), serenity::Error> {
        // Map common command types to their slash command names
        let command_name = match command_type.to_lowercase().as_str() {
            "play" => "play",
            "skip" => "skip",
            "stop" => "stop",
            "pause" => "pause",
            "resume" => "resume",
            "join" => "join",
            "leave" => "leave",
            "volume" => "volume",
            "queue" => "show_queue",
            "show_queue" => "show_queue",
            "shuffle" => "shuffle",
            "mute" => "mute",
            "unmute" => "unmute",
            "deafen" => "deafen",
            "undeafen" => "undeafen",
            "ping" => "ping",
            _ => command_type, // Use as-is if not in our mapping
        };

        // Send the slash command
        self.send_slash_command(command_name, options).await
    }

    // Run voice commands test
    pub async fn run_voice_commands_test(&self) -> Result<(), String> {
        // First join the voice channel
        self.join_voice_channel().await?;

        // Setup expectations for tests
        self.add_expectation("volume_test", "/volume", "Volume set to 50%")
            .await;
        self.add_expectation("play_test", "/play", "Playing song")
            .await;
        self.add_expectation("text_volume_test", "!volume 50", "Volume set to 50%")
            .await;
        self.add_expectation("text_play_test", "!play", "Playing song")
            .await;

        // Test with text commands first
        info!("Running text command tests...");
        let text_commands = vec![
            "!join",
            "!play https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "!volume 50",
            "!pause",
            "!resume",
            "!stop",
        ];

        for cmd in text_commands {
            // Send command
            match self.send_command(cmd).await {
                Ok(_) => info!("Sent text command: {}", cmd),
                Err(e) => error!("Error sending text command: {}", e),
            }

            // Wait for a response
            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        // Now test with slash commands
        info!("Running slash command tests...");

        // Test join
        match self.send_cracktunes_command("join", vec![]).await {
            Ok(_) => info!("Sent slash command: /join"),
            Err(e) => error!("Error sending slash command: {}", e),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Test play
        let play_options = vec![(
            "url".to_string(),
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
        )];
        match self.send_cracktunes_command("play", play_options).await {
            Ok(_) => info!("Sent slash command: /play"),
            Err(e) => error!("Error sending slash command: {}", e),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Test volume
        let volume_options = vec![("volume".to_string(), "50".to_string())];
        match self.send_cracktunes_command("volume", volume_options).await {
            Ok(_) => info!("Sent slash command: /volume"),
            Err(e) => error!("Error sending slash command: {}", e),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Test pause
        match self.send_cracktunes_command("pause", vec![]).await {
            Ok(_) => info!("Sent slash command: /pause"),
            Err(e) => error!("Error sending slash command: {}", e),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Test resume
        match self.send_cracktunes_command("resume", vec![]).await {
            Ok(_) => info!("Sent slash command: /resume"),
            Err(e) => error!("Error sending slash command: {}", e),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Test stop
        match self.send_cracktunes_command("stop", vec![]).await {
            Ok(_) => info!("Sent slash command: /stop"),
            Err(e) => error!("Error sending slash command: {}", e),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;

        Ok(())
    }
}

// A test structure to handle the bot's responses efficiently
impl Clone for TestHandler {
    fn clone(&self) -> Self {
        // Create new empty mutexes instead of trying to clone the locked values
        // This avoids deadlocks by not trying to acquire locks during clone
        Self {
            http: self.http.clone(),
            songbird: self.songbird.clone(),
            runtime: self.runtime.clone(),
            music_channel_id: Mutex::new(None), // Start with empty values
            voice_channel_id: Mutex::new(None),
            target_bot_id: Mutex::new(None),
            test_expectations: self.test_expectations.clone(), // DashMap is already thread-safe
            audio_analyzer: Mutex::new(None),
            guild_id: Mutex::new(None),
        }
    }
}

// Track end event handler for monitoring the target bot's activity
pub struct TrackEndNotifier {
    pub test_handler: Arc<TestHandler>,
    pub source_handler: Arc<TestHandler>,
    pub target_bot_id: serenity::UserId,
}

#[async_trait]
impl VoiceEventHandler for TrackEndNotifier {
    async fn act(&self, _ctx: &songbird::EventContext<'_>) -> Option<Event> {
        info!(
            "Detected track end event from target bot: {}",
            self.target_bot_id
        );

        // Sync state from the source handler to ensure this clone has up-to-date data
        if let Err(_) = self.test_handler.sync_from(&self.source_handler).await {
            error!("Failed to sync handler state in TrackEndNotifier");
        }

        None
    }
}

#[async_trait]
impl EventHandler for TestHandler {
    async fn dispatch(&self, ctx: &SerenityContext, event: &serenity::FullEvent) {
        match event {
            serenity::FullEvent::Ready { data_about_bot, .. } => {
                self.ready(ctx.clone(), data_about_bot.clone()).await
            }
            serenity::FullEvent::Resume { event, .. } => {
                self.resume(ctx.clone(), event.clone()).await
            }
            serenity::FullEvent::VoiceStateUpdate { old, new, .. } => {
                self.voice_state_update(ctx.clone(), new.guild_id, old.clone(), new.clone())
                    .await
            }
            _ => {}
        }
        if let serenity::FullEvent::Message { new_message, .. } = event {
            self.message(ctx.clone(), new_message.clone()).await;
        }
    }
}

impl TestHandler {
    async fn message(&self, _ctx: serenity::Context, msg: Message) {
        // Skip messages from this bot
        if msg.author.bot() && Some(msg.author.id) != *self.target_bot_id.lock().await {
            return;
        }

        let music_channel_id = *self.music_channel_id.lock().await;

        // Only process messages in the designated music channel
        if let Some(channel_id) = music_channel_id {
            if msg.channel_id == channel_id {
                debug!("Received message: {}", msg.content.to_string());

                // Check if this is a response to one of our test commands
                for mut entry in self.test_expectations.iter_mut() {
                    let expectation = entry.value_mut();

                    // If this message might be a response to our command
                    if msg.author.bot() && Some(msg.author.id) == *self.target_bot_id.lock().await {
                        // Check if the response matches what we expect
                        expectation.set_response(msg.content.to_string());
                        info!(
                            "Test for '{}': {}",
                            expectation.command,
                            if expectation.passed {
                                "PASSED"
                            } else {
                                "FAILED"
                            }
                        );
                    }
                }
            }
        }
    }

    async fn voice_state_update(
        &self,
        _ctx: serenity::Context,
        _guild_id: Option<GuildId>,
        old: Option<serenity::VoiceState>,
        new: serenity::VoiceState,
    ) {
        // Monitor the target bot's voice state
        if let Some(target_bot_id) = *self.target_bot_id.lock().await {
            if new.user_id == target_bot_id {
                info!("Target bot voice state update: {:?}", new);

                // Bot joined a voice channel
                if old.is_none() || old.as_ref().unwrap().channel_id.is_none() {
                    if let Some(channel_id) = new.channel_id {
                        info!("Target bot joined voice channel: {}", channel_id);
                    }
                }
                // Bot left a voice channel
                else if new.channel_id.is_none() {
                    info!("Target bot left voice channel");
                }
            }
        }
    }

    async fn ready(&self, _ctx: serenity::Context, ready: Ready) {
        info!("Test bot is connected as {}", ready.user.name);

        // We could automatically start tests here if desired
    }

    async fn resume(&self, _ctx: serenity::Context, _: ResumedEvent) {
        info!("Test bot resumed");
    }
}

// Main function to run the test bot
pub async fn run_test_bot() -> Result<(), Box<dyn std::error::Error>> {
    // Get the test bot token from environment
    let token = Token::from_env("TEST_BOT_TOKEN").expect("Expected TEST_BOT_TOKEN in environment");
    let target_bot_id = env::var("TARGET_BOT_ID")
        .expect("Expected TARGET_BOT_ID in environment")
        .parse::<u64>()
        .expect("TARGET_BOT_ID must be a valid u64");

    // Set up intents - we need a lot for proper testing
    let intents = GatewayIntents::non_privileged()
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::GUILD_MESSAGES;

    // Set up our songbird instance for voice channel access
    let songbird = songbird::Songbird::serenity();
    let songbird_clone = songbird.clone();

    // Create the test handler
    let http = Http::new(token.clone());
    let test_handler = TestHandler::new(Arc::new(http), songbird.clone());
    test_handler
        .set_target_bot_id(serenity::UserId::new(target_bot_id))
        .await;

    // Create the test bot client
    let mut client = Client::builder(token.clone(), intents)
        .event_handler(test_handler.clone())
        .voice_manager::<songbird::Songbird>(songbird_clone)
        .await?;

    // Start the client
    let client_handle = client.start();

    // Wait for the client to be ready (up to 10 seconds)
    match timeout(Duration::from_secs(10), async {
        // Wait for client to be ready
        tokio::time::sleep(Duration::from_secs(5)).await;
    })
    .await
    {
        Ok(_) => info!("Test bot ready for commands"),
        Err(_) => error!("Timeout waiting for test bot to be ready"),
    }

    // Client will now run until we decide to stop it
    client_handle.await?;

    Ok(())
}

// Run a specific test scenario
pub async fn run_test_scenario(
    test_handler: Arc<TestHandler>,
    channel_id: GenericChannelId,
    voice_channel_id: ChannelId,
    guild_id: GuildId,
) -> Result<(), String> {
    // Set up test parameters
    test_handler.set_music_channel_id(channel_id).await;
    test_handler.set_voice_channel_id(voice_channel_id).await;
    test_handler.set_guild_id(guild_id).await;

    // Set up test expectations
    test_handler
        .add_expectation("join_test", "!join", "Joined")
        .await;
    test_handler
        .add_expectation("play_test", "!play", "song")
        .await;
    test_handler
        .add_expectation("queue_test", "!show_queue", "Playing")
        .await;

    // Run the test
    test_handler.run_voice_commands_test().await?;

    // Wait for all the tests to complete
    tokio::time::sleep(Duration::from_secs(15)).await;

    // Get and display test results
    let results = test_handler.get_test_results().await;
    let mut passed = 0;
    let total = results.len();

    for (id, expectation) in results {
        if expectation.passed {
            passed += 1;
            info!("Test '{}' PASSED", id);
        } else {
            error!(
                "Test '{}' FAILED - Expected: '{}', Got: '{}'",
                id,
                expectation.expected_response,
                expectation
                    .actual_response
                    .unwrap_or_else(|| "No response".to_string())
            );
        }
    }

    info!("Test results: {}/{} passed", passed, total);

    if passed == total {
        Ok(())
    } else {
        Err(format!("{}/{} tests failed", total - passed, total))
    }
}
