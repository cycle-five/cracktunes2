#[cfg(test)]
mod queue_tests {
    use std::collections::VecDeque;

    use tokio;

    use crate::{CrackTrackQueue, ResolvedTrack, EMPTY_QUEUE};
    use crack_types::{QueryType, UserId};

    // Helper function to create a test track
    fn create_test_track(id: &str) -> ResolvedTrack {
        ResolvedTrack::new(QueryType::VideoLink(format!(
            "https://www.youtube.com/watch?v={}",
            id
        )))
        .with_user_id(UserId::new(1))
    }

    #[tokio::test]
    async fn test_queue_empty() {
        let queue = CrackTrackQueue::new();
        assert!(queue.is_empty().await);
        assert_eq!(queue.len().await, 0);
        assert!(queue.dequeue().await.is_none());
    }

    #[tokio::test]
    async fn test_queue_enqueue_dequeue() {
        let queue = CrackTrackQueue::new();

        // Add tracks
        let track1 = create_test_track("1");
        let track2 = create_test_track("2");

        queue.enqueue(track1.clone(), None).await;
        assert_eq!(queue.len().await, 1);
        assert!(!queue.is_empty().await);

        queue.enqueue(track2.clone(), None).await;
        assert_eq!(queue.len().await, 2);

        // Dequeue tracks (FIFO order)
        let dequeued1 = queue.dequeue().await.unwrap();
        assert_eq!(dequeued1.get_url(), track1.get_url());
        assert_eq!(queue.len().await, 1);

        let dequeued2 = queue.dequeue().await.unwrap();
        assert_eq!(dequeued2.get_url(), track2.get_url());
        assert_eq!(queue.len().await, 0);
        assert!(queue.is_empty().await);
    }

    #[tokio::test]
    async fn test_queue_clear() {
        let queue = CrackTrackQueue::new();

        // Add tracks
        queue.enqueue(create_test_track("1"), None).await;
        queue.enqueue(create_test_track("2"), None).await;
        queue.enqueue(create_test_track("3"), None).await;

        assert_eq!(queue.len().await, 3);

        // Clear queue
        queue.clear().await;
        assert_eq!(queue.len().await, 0);
        assert!(queue.is_empty().await);
    }

    #[tokio::test]
    async fn test_queue_get_remove() {
        let queue = CrackTrackQueue::new();

        // Add tracks
        let track1 = create_test_track("1");
        let track2 = create_test_track("2");
        let track3 = create_test_track("3");

        queue.enqueue(track1.clone(), None).await;
        queue.enqueue(track2.clone(), None).await;
        queue.enqueue(track3.clone(), None).await;

        // Get track at index
        let get_track2 = queue.get(1).await.unwrap();
        assert_eq!(get_track2.get_url(), track2.get_url());
        assert_eq!(queue.len().await, 3); // Length unchanged

        // Remove track at index
        let removed_track2 = queue.remove(1).await.unwrap();
        assert_eq!(removed_track2.get_url(), track2.get_url());
        assert_eq!(queue.len().await, 2); // Length decreased

        // The next track is now track3
        let next_track = queue.get(1).await.unwrap();
        assert_eq!(next_track.get_url(), track3.get_url());
    }

    #[tokio::test]
    async fn test_queue_push_pop_front_back() {
        let queue = CrackTrackQueue::new();

        let track1 = create_test_track("1");
        let track2 = create_test_track("2");

        // Push to back (same as enqueue)
        queue.push_back(track1.clone()).await;

        // Push to front
        queue.push_front(track2.clone()).await;

        // Order should be: track2, track1
        assert_eq!(queue.get(0).await.unwrap().get_url(), track2.get_url());
        assert_eq!(queue.get(1).await.unwrap().get_url(), track1.get_url());

        // Pop from back
        let back = queue.pop_back().await.unwrap();
        assert_eq!(back.get_url(), track1.get_url());
        assert_eq!(queue.len().await, 1);

        // Pop from front
        let front = queue.pop_front().await.unwrap();
        assert_eq!(front.get_url(), track2.get_url());
        assert_eq!(queue.len().await, 0);
    }

    #[tokio::test]
    async fn test_queue_insert() {
        let queue = CrackTrackQueue::new();

        // Add tracks
        queue.enqueue(create_test_track("1"), None).await;
        queue.enqueue(create_test_track("3"), None).await;

        // Insert in the middle
        let track2 = create_test_track("2");
        queue.insert(1, track2.clone()).await;

        // Check order
        assert_eq!(
            queue.get(0).await.unwrap().get_url(),
            "https://www.youtube.com/watch?v=1"
        );
        assert_eq!(
            queue.get(1).await.unwrap().get_url(),
            "https://www.youtube.com/watch?v=2"
        );
        assert_eq!(
            queue.get(2).await.unwrap().get_url(),
            "https://www.youtube.com/watch?v=3"
        );
    }

    #[tokio::test]
    async fn test_queue_append() {
        let queue = CrackTrackQueue::new();

        // Add initial tracks
        queue.enqueue(create_test_track("1"), None).await;
        queue.enqueue(create_test_track("2"), None).await;

        // Create a vector of tracks to append
        let tracks = vec![create_test_track("3"), create_test_track("4")];

        // Append vector
        queue.append_vec(tracks).await;

        // Check length and order
        assert_eq!(queue.len().await, 4);
        assert_eq!(
            queue.get(2).await.unwrap().get_url(),
            "https://www.youtube.com/watch?v=3"
        );
        assert_eq!(
            queue.get(3).await.unwrap().get_url(),
            "https://www.youtube.com/watch?v=4"
        );

        // Append using VecDeque
        let mut other_queue = VecDeque::new();
        other_queue.push_back(create_test_track("5"));
        other_queue.push_back(create_test_track("6"));

        queue.append(&mut other_queue).await;

        // Check length and order again
        assert_eq!(queue.len().await, 6);
        assert_eq!(
            queue.get(4).await.unwrap().get_url(),
            "https://www.youtube.com/watch?v=5"
        );
        assert_eq!(
            queue.get(5).await.unwrap().get_url(),
            "https://www.youtube.com/watch?v=6"
        );
    }

    #[tokio::test]
    async fn test_queue_shuffle() {
        let queue = CrackTrackQueue::new();

        // Add a bunch of tracks
        for i in 1..11 {
            queue.enqueue(create_test_track(&i.to_string()), None).await;
        }

        // Get the original order
        let original_queue = queue.get_queue().await;

        // Shuffle the queue
        queue.shuffle().await;

        // Get the shuffled order
        let shuffled_queue = queue.get_queue().await;

        // Same length
        assert_eq!(original_queue.len(), shuffled_queue.len());

        // Note: There's a small chance the shuffle doesn't change the order,
        // but with 10 items it's extremely unlikely
        let mut different = false;
        for i in 0..original_queue.len() {
            if original_queue[i].get_url() != shuffled_queue[i].get_url() {
                different = true;
                break;
            }
        }

        assert!(different, "Shuffle should have changed the order");

        // Ensure all original tracks are still in the queue (just in different order)
        let mut all_present = true;
        for track in &original_queue {
            let url = track.get_url();
            let found = shuffled_queue.iter().any(|t| t.get_url() == url);
            if !found {
                all_present = false;
                break;
            }
        }

        assert!(
            all_present,
            "All tracks should still be present after shuffle"
        );
    }

    #[tokio::test]
    async fn test_queue_display() {
        let mut queue = CrackTrackQueue::new();

        assert_eq!(queue.get_display(), EMPTY_QUEUE);

        // Add tracks
        queue.enqueue(create_test_track("1"), None).await;
        queue.enqueue(create_test_track("2"), None).await;

        // Display still empty until built
        assert_eq!(queue.display, EMPTY_QUEUE);

        // Build display
        queue.build_display().await;

        // Now display should have content
        assert_ne!(queue.display, EMPTY_QUEUE);
        let display = queue.get_display();
        assert!(!display.is_empty());
        assert!(display.contains("youtube.com/watch?v=1"));
        assert!(display.contains("youtube.com/watch?v=2"));
    }

    #[tokio::test]
    async fn test_queue_clone() {
        let queue = CrackTrackQueue::new();

        // Add tracks
        queue.enqueue(create_test_track("1"), None).await;
        queue.enqueue(create_test_track("2"), None).await;

        // Clone the queue
        let queue_clone = queue.clone();

        // Both should have same length
        assert_eq!(queue.len().await, queue_clone.len().await);

        // Modifying one *should* affect the other
        queue.enqueue(create_test_track("3"), None).await;
        assert_eq!(queue.len().await, 3);
        assert_eq!(queue_clone.len().await, 3);

        // Tracks should be the same in the clone
        assert_eq!(
            queue_clone.get(0).await.unwrap().get_url(),
            "https://www.youtube.com/watch?v=1"
        );
        assert_eq!(
            queue_clone.get(1).await.unwrap().get_url(),
            "https://www.youtube.com/watch?v=2"
        );
    }
}
