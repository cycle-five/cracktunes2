# Discord Interaction Webhooks Integration Plan

This document outlines a plan for implementing Discord's webhook-based interaction system with our test bot framework to properly test slash commands.

## 1. Setup Discord Application Configuration

1. **Register a webhook URL**:
   - In the Discord Developer Portal for your test bot
   - Configure "Interactions Endpoint URL" to point to your server
   - Discord will send a ping to validate the URL

2. **Setup public endpoint**:
   - You'll need a public HTTPS endpoint (Discord requires HTTPS)
   - Can be deployed using services like Ngrok for development or a proper server for production

## 2. Create Interaction Receiving Server

1. **Interaction verification server**:
   ```rust
   // This would be a separate application that runs alongside your test bot
   async fn main() {
       // Setup HTTP server with route for Discord interactions
       let app = Router::new()
           .route("/interactions", post(handle_interaction));
       
       // Run the server
       axum::Server::bind(&"0.0.0.0:8080".parse().unwrap())
           .serve(app.into_make_service())
           .await
           .unwrap();
   }
   ```

2. **Implement signature verification**:
   ```rust
   async fn handle_interaction(
       headers: HeaderMap,
       Json(payload): Json<serde_json::Value>,
   ) -> impl IntoResponse {
       // Extract signature and timestamp from headers
       let signature = headers.get("X-Signature-Ed25519");
       let timestamp = headers.get("X-Signature-Timestamp");
       
       // Verify the request is from Discord
       if verify_signature(signature, timestamp, &payload) {
           // Process the interaction
           process_interaction(payload).await
       } else {
           // Return 401 Unauthorized if verification fails
           StatusCode::UNAUTHORIZED.into_response()
       }
   }
   ```

## 3. Create a Command Queuing System

1. **Interaction queue**:
   ```rust
   // Shared state between your test bot and the interaction server
   struct InteractionQueue {
       pending_commands: Arc<Mutex<HashMap<String, oneshot::Sender<InteractionResponse>>>>,
   }
   ```

2. **Register pending commands**:
   ```rust
   // In your TestHandler
   async fn register_pending_command(&self, command_id: String) -> mpsc::Receiver<InteractionResponse> {
       let (tx, rx) = oneshot::channel();
       INTERACTION_QUEUE.pending_commands.lock().await.insert(command_id, tx);
       rx
   }
   ```

## 4. Integration with Current TestHandler

1. **Modify send_slash_command approach**:
   - Instead of direct HTTP request, create a command ID and register it in the queue
   - Send message to Discord channel with the slash command syntax
   - Wait for the interaction server to receive Discord's interaction

2. **Implementation outline**:
   ```rust
   // This doesn't change your existing code, but shows how the new approach would work
   pub async fn send_slash_command_webhook(&self, 
       command_name: &str, 
       options: Vec<(String, String)>
   ) -> Result<(), serenity::Error> {
       // Generate a unique command ID
       let command_id = Uuid::new_v4().to_string();
       
       // Register this command in the waiting queue
       let response_rx = self.register_pending_command(command_id.clone()).await;
       
       // Send a message that will trigger the slash command in Discord UI
       // This could be using your existing send_text_command method
       self.send_text_command(&format!("/{} {}", command_name, command_id)).await?;
       
       // Wait for the interaction webhook to receive the response
       match tokio::time::timeout(Duration::from_secs(5), response_rx).await {
           Ok(Ok(response)) => {
               // Process the response
               Ok(())
           }
           _ => Err(serenity::Error::Other("Interaction timed out")),
       }
   }
   ```

## 5. Implement Interaction Processing

1. **Process incoming interactions**:
   ```rust
   async fn process_interaction(payload: serde_json::Value) -> impl IntoResponse {
       // Extract command details
       let command_name = payload["data"]["name"].as_str().unwrap_or_default();
       let options = payload["data"]["options"].as_array().unwrap_or(&Vec::new());
       
       // Look for our command ID in the options
       if let Some(command_id) = extract_command_id_from_options(options) {
           // If this is one of our test commands, notify the waiting handler
           if let Some(sender) = INTERACTION_QUEUE.pending_commands.lock().await.remove(&command_id) {
               let _ = sender.send(InteractionResponse { 
                   payload: payload.clone(),
               });
           }
       }
       
       // Return appropriate response to Discord
       Json(json!({
           "type": 1 // ACK the interaction
       }))
   }
   ```

## 6. Testing Workflow

1. The test bot sets up expectations
2. It registers a command in the interaction queue with a unique ID
3. It triggers a message in Discord that looks like a slash command with this ID
4. Discord's client turns this into an interaction and sends it to your webhook
5. Your webhook server verifies and processes the interaction
6. The webhook server notifies the test bot that the interaction was received
7. The test bot can then verify that the command was properly processed

## Discord Test Mode (Guild Applications)

Discord provides a test mode for applications in development that can be more suitable for integration testing. Here's how it works:

### 1. Guild-Based Application Commands

When you create slash commands, there are two types of registration:
- **Global commands**: Available in all servers where your bot is installed
- **Guild commands**: Available only in specific servers (guilds)

Guild commands have several advantages for testing:
- They update instantly (unlike global commands that can take up to an hour)
- They can be created and modified without affecting production users
- They can be created with different options for testing

### 2. Testing in Development Server

1. **Create a dedicated development server**:
   - Set up a server specifically for bot testing
   - Add your test bot and the bot-under-test to this server

2. **Register guild commands**:
   ```rust
   // Register test commands to a specific guild
   async fn register_test_commands(guild_id: GuildId) {
       http.create_guild_application_command(guild_id, |command| {
           command
               .name("test_command")
               .description("Command for testing")
               // Add test parameters
       })
       .await?;
   }
   ```

### 3. Interaction Payload Security

For test environments, Discord's interaction security requirements are easier to manage:

1. **Relaxed verification in test guilds**:
   - When using test guild command interactions, some of Discord's normal security requirements are relaxed

2. **No public endpoint needed**:
   - For testing within a guild, you can use local endpoints with tunneling services like ngrok
   - The HTTPS requirement remains, but it's easier to set up for development

### 4. Command Testing Process

1. **Create test guild commands**:
   ```rust
   // In your test setup
   async fn setup_test_environment(&self) -> Result<(), Error> {
       // Register guild-specific commands for testing
       self.register_guild_test_commands(self.guild_id).await?;
       // Wait for commands to register (usually instant for guild commands)
       tokio::time::sleep(Duration::from_secs(1)).await;
       Ok(())
   }
   ```

2. **Trigger command interactions**:
   - Use your test bot to send messages that trigger the commands
   - Discord will convert these to interactions and send them to your webhook

3. **Verify responses**:
   - Your webhook server can validate that the correct response was sent
   - Your test bot can verify the visible result in the Discord channel

### 5. Benefits for Integration Testing

1. **Isolated environment**: Changes don't affect production users
2. **Faster iteration**: Guild commands update immediately
3. **Realistic testing**: Uses actual Discord interactions rather than simulations
4. **Lower setup barrier**: Less stringent requirements for testing environments

### 6. Implementing with Current Test Bot

To incorporate Discord's test mode with your current test bot:

1. Add a setup phase that registers guild-specific test commands
2. Use your existing webhook server to handle interactions
3. Modify the test scenarios to use the specific test commands
4. Verify responses through the webhook and visible channel messages

This approach provides a more realistic testing environment while reducing the complexity of implementing a full production-grade interaction system.

## Implementation Challenges

1. **Security**: Properly implementing the Ed25519 signature verification
2. **Infrastructure**: Maintaining a public HTTPS endpoint
3. **Command Triggering**: Finding a reliable way to trigger the slash commands from your test bot

## Alternatives

If this approach seems complex, consider:

1. **Simulated Interactions**: Instead of real Discord interactions, create a simulated environment for testing
2. **Headless Discord Client**: Use a browser automation tool (like Playwright) to control a real Discord client
3. **Directly Test Bot Logic**: Test the command handling logic directly rather than through the Discord API