use crack_testing::run;
use tracing::warn;

/// Initialize logging and tracing.
fn init_logging() {
    tracing_subscriber::fmt().init();
    warn!("Hello, world!");
}

/// Main function
#[tokio::main]
async fn main() {
    #[cfg(feature = "crack-tracing")]
    init_logging();
    #[cfg(not(feature = "crack-tracing"))]
    println!("Starting...");

    let _ = run().await;
}
