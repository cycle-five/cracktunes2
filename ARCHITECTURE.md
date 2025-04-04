# CrackTunes Architecture

## Hexagonal Architecture Overview

CrackTunes employs a hexagonal architecture (also known as ports and adapters) to separate business logic from external concerns. This architecture provides several benefits:

1. **Separation of Concerns**: Core business logic is isolated from external dependencies
2. **Testability**: Core services can be tested without external dependencies
3. **Flexibility**: External implementations can be swapped without changing core logic
4. **Maintainability**: Clear boundaries make the codebase easier to understand and maintain

## Core Components

### Ports (Interfaces)
- Define boundaries between the application core and external world
- Specified as Rust traits in the `core/ports` directory
- Examples: `AudioPlayer`, `AudioProvider`, `MessageHandler`, `StateManager`

### Core Services
- Implement business logic using ports for external dependencies
- Found in the `core/services` directory
- Examples: `MusicService`, `ConnectionService`
- Only depend on other core services and ports, never on concrete adapters

### Adapters (Implementations)
- Implement the port interfaces to connect with external technologies
- Located in the `adapters` directory, organized by external system
- Examples: `SerenityMessageHandler`, `PoiseMessageHandler`, `SongbirdPlayer`, `RustyYtdlProvider`

### Domain Models
- Represent the core entities and value objects of the application
- Located in the `core/models` directory
- Used throughout the application for consistency

## Context Passing Strategy for Message Handlers

### Motivation

When implementing message handlers for different Discord client libraries (Serenity direct vs. Poise), we encounter challenges with context objects:

1. **Lifetime Parameters**: Poise's `Context<'_, Data, Error>` has a lifetime parameter that makes storing or passing it around challenging.

2. **Rich Context Features**: Poise's context provides powerful features not available with simple HTTP clients:
   - Rich message building
   - Edit tracking
   - Reply threading
   - Deferred responses
   - Command context awareness

3. **Lost Functionality**: Abstracting too much leads to losing framework-specific capabilities.

### Previous Approach and Limitations

Our initial approach tried to abstract Discord interactions behind a unified `MessageHandler` trait:

```rust
#[async_trait]
pub trait MessageHandler: Send + Sync + 'static {
    async fn send_message(&self, channel_id: TextChannelId, content: &str) -> Result<(), Error>;
    // Other methods...
}
```

This approach had limitations:
- Poise contexts couldn't be stored due to lifetime parameters
- Required falling back to raw HTTP calls, losing Poise features
- Created complexity with context registration and storage

### Context Passing Strategy

Instead of abstracting away the context, we now embrace it with a generic context-passing strategy:

1. **Contextual Message Handler Trait**:
```rust
#[async_trait]
pub trait ContextualMessageHandler<C> {
    async fn send_message_with_context(&self, ctx: &C, content: &str) -> Result<(), Error>;
    async fn send_rich_message_with_context(&self, ctx: &C, /*params*/) -> Result<(), Error>;
    // Other methods
}
```

2. **Implementations for Specific Contexts**:
- Direct implementation for Poise contexts
- Implementation for Serenity contexts
- Testing implementation using mock contexts

3. **Service Integration**:
- Services accept the context in methods that need to respond to users
- Command handlers pass their context directly to service methods

### Benefits of Context Passing

1. **Framework Feature Utilization**: Directly leverage all features of the underlying framework
2. **Simplified Implementation**: No need for complex context storage or registration
3. **Type Safety**: Compiler ensures compatible context types are used
4. **Flexibility**: Easy to adapt to future Discord API changes or framework updates
5. **True to Hexagonal Architecture**: External dependencies still only cross the boundary at well-defined points

## Command Structure

The command structure is organized into:

1. **Command Modules**: Located in `commands/` directory
2. **Command Handlers**: Process user inputs and call appropriate services
3. **V2 Architecture**: Updated command system in `commands/v2/` supports both frameworks

## Audio Processing Flow

1. User requests a song via command
2. Command handler calls `MusicService`
3. `MusicService` uses `AudioProvider` to fetch audio
4. `MusicService` uses `AudioPlayer` to play in voice channel
5. `MusicService` uses `MessageHandler` to communicate with user
6. `StateManager` maintains state across commands

## Next Steps for Architecture Evolution

1. Complete transition to context-passing strategy
2. Unify command handlers to use consistent approach
3. Implement more specialized adapters (e.g., Spotify integration)
4. Expand test coverage with more mock adapters
5. Consider adding a dependency injection container