use crate::ResolvedTrack;
use crate::EMPTY_QUEUE;

use rand::seq::SliceRandom;
use std::collections::VecDeque;
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;
use tokio::sync::Mutex;

/// A [`CrackTrackQueue`] queue of tracks to be played.
#[derive(Clone, Debug)]
pub struct CrackTrackQueue {
    //inner: Arc<DashMap<GuildId, VecDeque<ResolvedTrack>>>,
    inner: Arc<Mutex<VecDeque<ResolvedTrack>>>,
    pub(crate) playing: Option<ResolvedTrack>,
    pub(crate) display: String,
    // New field to reference the Songbird driver
    songbird_call: Option<Arc<Mutex<songbird::Call>>>,
    // New field to reference the Songbird TrackQueue
    // songbird_queue: Option<Arc<songbird::tracks::TrackQueue>>,
    // Reference to reqwest client for creating YoutubeDl inputs
    req_client: Option<reqwest::Client>,
}

/// Implement [`Default`] for [`CrackTrackQueue`].
impl Default for CrackTrackQueue {
    fn default() -> Self {
        CrackTrackQueue {
            inner: Arc::new(Mutex::new(VecDeque::new())),
            display: EMPTY_QUEUE.to_string(),
            playing: None,
            songbird_call: None,
            req_client: None,
        }
    }
}

/// Implement [`CrackTrackQueue`].
impl CrackTrackQueue {
    /// Create a new [`CrackTrackQueue`].
    #[must_use]
    pub fn new() -> Self {
        CrackTrackQueue::default()
    }

    /// Create a new [`CrackTrackQueue`] with a given [`VecDeque`] of [`ResolvedTrack`].
    #[must_use]
    pub fn with_queue(queue: VecDeque<ResolvedTrack>) -> Self {
        CrackTrackQueue {
            inner: Arc::new(Mutex::new(queue)),
            ..Default::default()
        }
    }

    // New method to set the Songbird queue reference
    pub fn with_songbird(
        mut self,
        call: Arc<Mutex<songbird::Call>>,
        req_client: reqwest::Client,
    ) -> Self {
        self.songbird_call = Some(call);
        self.req_client = Some(req_client);
        self
    }

    // Update enqueue to add to both queues
    pub async fn enqueue(
        &self,
        track: ResolvedTrack,
        pre_acquired_call: Option<&mut songbird::Call>,
    ) -> ResolvedTrack {
        // Add to metadata queue
        self.push_back(track.clone()).await;

        // If we have a pre-acquired call, use it; otherwise acquire our own lock
        if let Some(call) = pre_acquired_call {
            let input =
                songbird::input::YoutubeDl::new(self.req_client.clone().unwrap(), track.get_url());
            let _ = call.enqueue_input(input.into()).await;
        } else if let (Some(songbird_call), Some(req_client)) =
            (&self.songbird_call, &self.req_client)
        {
            let input = songbird::input::YoutubeDl::new(req_client.clone(), track.get_url());
            let mut call = songbird_call.lock().await;
            let _ = call.enqueue_input(input.into()).await;
        }

        track
    }

    // Update dequeue to remove from metadata queue only
    // (Songbird will handle its own queue)
    pub async fn dequeue(&self) -> Option<ResolvedTrack> {
        self.pop_front().await
    }

    // Update clear to clear both queues
    pub async fn clear(&self) {
        self.inner.lock().await.clear();
        if let Some(songbird_call) = &self.songbird_call {
            songbird_call.lock().await.stop();
        }
    }

    /// Get the queue.
    pub async fn get_queue(&self) -> VecDeque<ResolvedTrack> {
        self.inner.lock().await.clone()
    }

    /// Return the display string for the queue.
    #[must_use]
    pub fn get_display(&self) -> String {
        self.display.clone()
    }

    /// Build the display string for the queue.
    /// This *must* be called before displaying the queue.
    pub async fn build_display(&mut self) {
        let now_playing = if let Some(track) = &self.playing {
            format!("Now Playing: {}", track)
        } else {
            "Nothing is currently playing.".to_string()
        };
        let queued = {
            let queue = self.inner.lock().await.clone();
            queue
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>()
                .join("\n")
        };
        self.display = format!("{}\n\n{}", now_playing, queued);
    }

    /// Get the length of the queue.
    pub async fn len(&self) -> usize {
        self.inner.lock().await.len()
    }

    /// Check if the queue is empty.
    pub async fn is_empty(&self) -> bool {
        self.inner.lock().await.is_empty()
    }

    /// Get the element at the given index in the queue.
    pub async fn get(&self, index: usize) -> Option<ResolvedTrack> {
        self.inner.lock().await.get(index).cloned()
    }

    /// Remove the element at the given index in the queue.
    pub async fn remove(&self, index: usize) -> Option<ResolvedTrack> {
        self.inner.lock().await.remove(index)
    }

    /// Add a track to the back of the queue.
    pub async fn push_back(&self, track: ResolvedTrack) {
        self.inner.lock().await.push_back(track);
    }

    /// Add a track to the front of the queue.
    pub async fn push_front(&self, track: ResolvedTrack) {
        self.inner.lock().await.push_front(track);
    }

    /// Remove the last track from the queue.
    pub async fn pop_back(&self) -> Option<ResolvedTrack> {
        self.inner.lock().await.pop_back()
    }

    /// Remove the first track from the queue.
    pub async fn pop_front(&self) -> Option<ResolvedTrack> {
        self.inner.lock().await.pop_front()
    }

    /// Insert a track at the given index in the queue.
    pub async fn insert(&self, index: usize, track: ResolvedTrack) {
        self.inner.lock().await.insert(index, track);
    }

    /// Append a vector of tracks to the end of the queue.
    pub async fn append_vec(&self, vec: Vec<ResolvedTrack>) {
        self.append(&mut VecDeque::from(vec)).await;
    }

    /// Append another queue to the end of this queue.
    pub async fn append(&self, other: &mut VecDeque<ResolvedTrack>) {
        self.inner.lock().await.append(other);
    }

    /// Shuffle the queue.
    pub async fn shuffle(&self) {
        self.inner
            .lock()
            .await
            .make_contiguous()
            .shuffle(&mut rand::rng());
    }

    /// Append a copy of this queue to another queue.
    pub async fn append_self_to_other(&self, other: &mut VecDeque<ResolvedTrack>) {
        other.append(&mut self.inner.lock().await.clone());
    }
}

/// Implement [`Display`] for [`CrackTrackQueue`].
impl Display for CrackTrackQueue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display)
    }
}
