# CrackTunes Development Guide

## Build & Test Commands
- **Build**: `cargo build`
- **Run**: `cargo run`
- **Test all**: `cargo test`
- **Test single test**: `cargo test test_name`
- **Test specific module**: `cargo test module_name`
- **Test with output**: `cargo test -- --nocapture`
- **Format code**: `cargo fmt`
- **Lint code**: `cargo clippy -- -D clippy::all -D warnings -W clippy::pedantic`
- **Fix lints**: `cargo clippy --fix`

## Code Style Guidelines
- **Error Handling**: Use `thiserror` for error types, return `Result<T, crack_types::Error>`
- **Logging**: Use `tracing` (`debug`, `info`, `error`) with appropriate targets
- **Formatting**: Follow standard Rust formatting (via `rustfmt`)
- **Imports**: Group imports by crate, alphabetically, with internal crates first
- **Naming**: Snake case for variables/functions, CamelCase for types, SCREAMING_SNAKE for constants
- **Comments**: Document public functions with `///` doc comments and `# Errors` sections
- **Async**: Use `async/await` consistently, leverage `tokio` features
- **Types**: Prefer owned types when possible, use references when needed for performance
- **Modules**: Keep module structure clean with proper re-exports in mod.rs files