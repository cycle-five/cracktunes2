#[cfg(test)]
mod integration_tests {
    use mockall::mock;
    use mockall::predicate::*;
    use poise::serenity_prelude::GuildId;
    use std::sync::Arc;
    use std::time::Duration;

    use crate::CrackTrackQueue;
    use crate::ResolvedTrack;
    use crate::REQ_CLIENT;
    use crack_types::{QueryType, UserId};

    // Create mock for Context
    mock! {
        pub Context {}
        impl Clone for Context {
            fn clone(&self) -> Self;
        }
    }

    // Create mock for the Call struct
    mock! {
        pub Call {}
        impl Clone for Call {
            fn clone(&self) -> Self;
        }
    }

    // Helper function to create a new test queue with tracks
    async fn setup_test_queue() -> CrackTrackQueue {
        let queue = CrackTrackQueue::new(REQ_CLIENT.clone());

        // Add some tracks
        let track1 = ResolvedTrack::new(QueryType::VideoLink(
            "https://www.youtube.com/watch?v=hdA8uvHYrYE".to_string(),
        ))
        .with_user_id(UserId::new(1));

        let track2 = ResolvedTrack::new(QueryType::VideoLink(
            "https://www.youtube.com/watch?v=EkKYIg_qubA".to_string(),
        ))
        .with_user_id(UserId::new(1));

        let track3 = ResolvedTrack::new(QueryType::VideoLink(
            "https://www.youtube.com/watch?v=rENr1sxQUo8".to_string(),
        ))
        .with_user_id(UserId::new(1));

        queue.enqueue(track1, None).await;
        queue.enqueue(track2, None).await;
        queue.enqueue(track3, None).await;

        queue
    }

    #[tokio::test]
    async fn test_queue_concurrent_access() {
        let queue = Arc::new(setup_test_queue().await);

        // Create multiple tasks that access the queue concurrently
        let mut handles = Vec::new();

        // Task 1: Dequeue items
        let queue_clone1 = queue.clone();
        let handle1 = tokio::spawn(async move {
            for _ in 0..2 {
                let _ = queue_clone1.dequeue().await;
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            vec![]
        });
        handles.push(handle1);

        // Task 2: Enqueue more items
        let queue_clone2 = queue.clone();
        let handle2 = tokio::spawn(async move {
            for i in 10..15 {
                let track = ResolvedTrack::new(QueryType::VideoLink(format!(
                    "https://youtube.com/watch?v={}",
                    i
                )))
                .with_user_id(UserId::new(1));
                queue_clone2.enqueue(track, None).await;
                tokio::time::sleep(Duration::from_millis(15)).await;
            }
            vec![]
        });
        handles.push(handle2);

        // Task 3: Check length
        let queue_clone3 = queue.clone();
        let handle3 = tokio::spawn(async move {
            let mut lengths = Vec::new();
            for _ in 0..6 {
                lengths.push(queue_clone3.len().await);
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            lengths
        });
        handles.push(handle3);

        // Wait for all tasks to complete
        for handle in handles {
            let _ = handle.await;
        }

        // After all tasks, queue should have 1 original track + 5 new tracks - 2 dequeued = 4 tracks
        assert_eq!(queue.len().await, 6);
    }

    #[tokio::test]
    async fn test_shuffle_keeps_all_tracks() {
        // Create a queue with a lot of tracks to make shuffle more likely to change order
        let queue = CrackTrackQueue::new(REQ_CLIENT.clone());

        // Add 20 tracks
        for i in 1..21 {
            let track = ResolvedTrack::new(QueryType::VideoLink(format!(
                "https://youtube.com/watch?v={}",
                i
            )))
            .with_user_id(UserId::new(1));
            queue.push_back(track).await;
        }

        // Get all tracks before shuffle
        let tracks_before = queue.get_queue().await;
        let urls_before: Vec<String> = tracks_before.iter().map(|t| t.get_url()).collect();

        // Shuffle multiple times
        for _ in 0..5 {
            queue.shuffle().await;
        }

        // Get all tracks after shuffle
        let tracks_after = queue.get_queue().await;
        let urls_after: Vec<String> = tracks_after.iter().map(|t| t.get_url()).collect();

        // Check that all tracks are still present
        assert_eq!(tracks_before.len(), tracks_after.len());

        for url in &urls_before {
            assert!(
                urls_after.contains(url),
                "Track {} was lost during shuffle",
                url
            );
        }
    }

    #[tokio::test]
    async fn test_multiple_queue_instances() {
        // Test that we can have separate queues for different guilds
        let guild1 = GuildId::new(1);
        let guild2 = GuildId::new(2);

        let queues = dashmap::DashMap::new();

        // Create queues for each guild
        queues.insert(guild1, CrackTrackQueue::new(REQ_CLIENT.clone()));
        queues.insert(guild2, CrackTrackQueue::new(REQ_CLIENT.clone()));

        // Add different tracks to each queue
        let queue1 = queues.get(&guild1).unwrap();
        let track1 = ResolvedTrack::new(QueryType::VideoLink(
            "https://www.youtube.com/watch?v=guild1".to_string(),
        ))
        .with_user_id(UserId::new(1));
        queue1.enqueue(track1, None).await;

        let queue2 = queues.get(&guild2).unwrap();
        let track2 = ResolvedTrack::new(QueryType::VideoLink(
            "https://www.youtube.com/watch?v=guild2".to_string(),
        ))
        .with_user_id(UserId::new(2));
        queue2.enqueue(track2, None).await;

        // Verify each queue has its own content
        assert_eq!(queue1.len().await, 1);
        assert_eq!(queue2.len().await, 1);

        let track1_url = queue1.get(0).await.unwrap().get_url();
        let track2_url = queue2.get(0).await.unwrap().get_url();

        assert_eq!(track1_url, "https://www.youtube.com/watch?v=guild1");
        assert_eq!(track2_url, "https://www.youtube.com/watch?v=guild2");

        // Changes to one queue *don't* affect the other
        queue1.clear().await;
        assert_eq!(queue1.len().await, 0);
        assert_eq!(queue2.len().await, 1); // Still has its track
    }

    #[tokio::test]
    async fn test_queue_stress_test() {
        // Add and remove a large number of tracks to test performance and correctness
        let queue = CrackTrackQueue::new(REQ_CLIENT.clone());

        // Add 1000 tracks
        let start_time = std::time::Instant::now();

        for i in 1..1001 {
            let track = ResolvedTrack::new(QueryType::VideoLink(format!(
                "https://youtube.com/watch?v={}",
                i
            )))
            .with_user_id(UserId::new(1));
            queue.push_back(track).await;
        }

        let add_time = start_time.elapsed();
        println!("Time to add 1000 tracks: {:?}", add_time);

        assert_eq!(queue.len().await, 1000);

        // Test random access is fast
        let random_access_start = std::time::Instant::now();
        for i in (0..1000).step_by(100) {
            let _ = queue.get(i).await;
        }
        let random_access_time = random_access_start.elapsed();
        println!("Time to access 10 random tracks: {:?}", random_access_time);

        // Remove all tracks and verify
        let remove_start = std::time::Instant::now();
        while !queue.is_empty().await {
            let _ = queue.dequeue().await;
        }
        let remove_time = remove_start.elapsed();
        println!("Time to remove 1000 tracks: {:?}", remove_time);

        assert_eq!(queue.len().await, 0);
    }
}
