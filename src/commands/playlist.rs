// Add this to your main.rs to handle playlist resolution
#[poise::command(slash_command, prefix_command, guild_only)]
async fn playlist(
    ctx: Context<'_>,
    #[description = "URL to a YouTube playlist"] url: String,
) -> Result<(), serenity::Error> {
    if !url.starts_with("http") || !url.contains("list=") {
        ctx.say("Must provide a valid YouTube playlist URL").await?;
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

        // First notify that we're processing the playlist
        let processing_msg = ctx.say("Processing playlist, this may take a moment...").await?;
        
        // Create a client to resolve the playlist
        let client = reqwest::Client::new();
        let track_client = cracktunes::CrackTrackClient::new_with_req_client(client);
        
        // Create a playlist query
        let query = crack_types::QueryType::PlaylistLink(url.clone());
        
        // Resolve the playlist
        match track_client.resolve_query_to_tracks(query).await {
            Ok(tracks) => {
                // Add all tracks to our custom queue
                let track_count = tracks.len();
                if track_count == 0 {
                    ctx.say("No tracks found in the playlist").await?;
                    return Ok(());
                }
                
                for track in tracks {
                    // Add user ID to tracks
                    let track = track.with_user_id(ctx.author().id);
                    queue.enqueue(track).await;
                }
                
                // Check if we need to start playing (if queue was empty before)
                let was_empty = queue.len().await == track_count;
                if was_empty {
                    // Start playing the first track
                    play_next_from_queue(ctx, queue.clone(), handler.clone()).await?;
                }
                
                // Build the display for the queue
                let mut queue_clone = queue.clone();
                queue_clone
                    .build_display()
                    .await
                    .map_err(|_| serenity::Error::Other("Failed to build queue display"))?;
                
                // Update the message
                processing_msg.edit(ctx, |m| {
                    m.content(format!("Added {} tracks to the queue!", track_count))
                }).await?;
            },
            Err(e) => {
                processing_msg.edit(ctx, |m| {
                    m.content(format!("Error processing playlist: {}", e))
                }).await?;
            }
        }
    } else {
        ctx.say("Not in a voice channel to play in").await?;
    }

    Ok(())
}
