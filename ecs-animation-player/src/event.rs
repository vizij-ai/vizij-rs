//! Event system for animation player notifications

use crate::{AnimationTime, KeypointId, TrackId, Value};
use bevy::prelude::Event;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Types of animation events
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum EventType {
    /// Animation playback started
    PlaybackStarted,
    /// Animation playback paused
    PlaybackPaused,
    /// Animation playback stopped
    PlaybackStopped,
    /// Animation playback resumed
    PlaybackResumed,
    /// Animation reached the end
    PlaybackEnded,
    /// Animation time changed (seeking)
    TimeChanged,
    /// A keypoint was reached during playback
    KeypointReached,
    /// Animation track was added
    TrackAdded,
    /// Animation track was removed
    TrackRemoved,
    /// Animation track was modified
    TrackModified,
    /// Animation keypoint was added
    KeypointAdded,
    /// Animation keypoint was removed
    KeypointRemoved,
    /// Animation keypoint was modified
    KeypointModified,
    /// Animation data was loaded
    AnimationLoaded,
    /// Animation data was unloaded
    AnimationUnloaded,
    /// Animation data was modified
    AnimationModified,
    /// Performance warning (e.g., low FPS)
    PerformanceWarning,
    /// Error occurred during animation
    Error,
    /// Custom user-defined event
    Custom(String),
}

impl EventType {
    /// Get the name of this event type
    #[inline]
    pub fn name(&self) -> &str {
        match self {
            Self::PlaybackStarted => "playback_started",
            Self::PlaybackPaused => "playback_paused",
            Self::PlaybackStopped => "playback_stopped",
            Self::PlaybackResumed => "playback_resumed",
            Self::PlaybackEnded => "playback_ended",
            Self::TimeChanged => "time_changed",
            Self::KeypointReached => "keypoint_reached",
            Self::TrackAdded => "track_added",
            Self::TrackRemoved => "track_removed",
            Self::TrackModified => "track_modified",
            Self::KeypointAdded => "keypoint_added",
            Self::KeypointRemoved => "keypoint_removed",
            Self::KeypointModified => "keypoint_modified",
            Self::AnimationLoaded => "animation_loaded",
            Self::AnimationUnloaded => "animation_unloaded",
            Self::AnimationModified => "animation_modified",
            Self::PerformanceWarning => "performance_warning",
            Self::Error => "error",
            Self::Custom(name) => name,
        }
    }

    /// Check if this is a playback-related event
    #[inline]
    pub fn is_playback_event(&self) -> bool {
        matches!(
            self,
            Self::PlaybackStarted
                | Self::PlaybackPaused
                | Self::PlaybackStopped
                | Self::PlaybackResumed
                | Self::PlaybackEnded
                | Self::TimeChanged
        )
    }

    /// Check if this is a data modification event
    #[inline]
    pub fn is_modification_event(&self) -> bool {
        matches!(
            self,
            Self::TrackAdded
                | Self::TrackRemoved
                | Self::TrackModified
                | Self::KeypointAdded
                | Self::KeypointRemoved
                | Self::KeypointModified
                | Self::AnimationModified
        )
    }

    /// Check if this is an error or warning event
    #[inline]
    pub fn is_diagnostic_event(&self) -> bool {
        matches!(self, Self::PerformanceWarning | Self::Error)
    }
}

impl From<&str> for EventType {
    fn from(s: &str) -> Self {
        match s {
            "playback_started" => Self::PlaybackStarted,
            "playback_paused" => Self::PlaybackPaused,
            "playback_stopped" => Self::PlaybackStopped,
            "playback_resumed" => Self::PlaybackResumed,
            "playback_ended" => Self::PlaybackEnded,
            "time_changed" => Self::TimeChanged,
            "keypoint_reached" => Self::KeypointReached,
            "track_added" => Self::TrackAdded,
            "track_removed" => Self::TrackRemoved,
            "track_modified" => Self::TrackModified,
            "keypoint_added" => Self::KeypointAdded,
            "keypoint_removed" => Self::KeypointRemoved,
            "keypoint_modified" => Self::KeypointModified,
            "animation_loaded" => Self::AnimationLoaded,
            "animation_unloaded" => Self::AnimationUnloaded,
            "animation_modified" => Self::AnimationModified,
            "performance_warning" => Self::PerformanceWarning,
            "error" => Self::Error,
            custom => Self::Custom(custom.to_string()),
        }
    }
}

/// Animation event with associated data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Event)]
pub struct AnimationEvent {
    /// Type of event
    pub event_type: EventType,
    /// Animation ID this event relates to
    pub animation_id: String,
    /// Player ID that generated this event
    pub player_id: Option<String>,
    /// Time when the event occurred
    pub timestamp: AnimationTime,
    /// Current animation time when event occurred
    pub animation_time: Option<AnimationTime>,
    /// Track ID if event relates to a specific track
    pub track_id: Option<TrackId>,
    /// Keypoint ID if event relates to a specific keypoint
    pub keypoint_id: Option<KeypointId>,
    /// Associated value (e.g., for keypoint reached events)
    pub value: Option<Value>,
    /// Additional event-specific data
    pub data: HashMap<String, String>,
    /// Human-readable message
    pub message: Option<String>,
}

impl AnimationEvent {
    /// Create a new animation event
    pub fn new(
        event_type: EventType,
        animation_id: impl Into<String>,
        timestamp: AnimationTime,
    ) -> Self {
        Self {
            event_type,
            animation_id: animation_id.into(),
            player_id: None,
            timestamp,
            animation_time: None,
            track_id: None,
            keypoint_id: None,
            value: None,
            data: HashMap::new(),
            message: None,
        }
    }

    /// Set the player ID
    #[inline]
    pub fn with_player_id(mut self, player_id: impl Into<String>) -> Self {
        self.player_id = Some(player_id.into());
        self
    }

    /// Set the animation time
    #[inline]
    pub fn with_animation_time(mut self, time: AnimationTime) -> Self {
        self.animation_time = Some(time);
        self
    }

    /// Set the track ID
    #[inline]
    pub fn with_track_id(mut self, track_id: TrackId) -> Self {
        self.track_id = Some(track_id);
        self
    }

    /// Set the keypoint ID
    #[inline]
    pub fn with_keypoint_id(mut self, keypoint_id: KeypointId) -> Self {
        self.keypoint_id = Some(keypoint_id);
        self
    }

    /// Set the associated value
    #[inline]
    pub fn with_value(mut self, value: Value) -> Self {
        self.value = Some(value);
        self
    }

    /// Set the message
    #[inline]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Add event data
    #[inline]
    pub fn with_data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.data.insert(key.into(), value.into());
        self
    }

    /// Get event data
    #[inline]
    pub fn get_data(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(|s| s.as_str())
    }

    /// Create a playback started event
    #[inline]
    pub fn playback_started(
        animation_id: impl Into<String>,
        player_id: impl Into<String>,
        timestamp: AnimationTime,
    ) -> Self {
        Self::new(EventType::PlaybackStarted, animation_id, timestamp)
            .with_player_id(player_id)
            .with_animation_time(AnimationTime::zero())
            .with_message("Animation playback started")
    }

    /// Create a playback paused event
    #[inline]
    pub fn playback_paused(
        animation_id: impl Into<String>,
        player_id: impl Into<String>,
        timestamp: AnimationTime,
        animation_time: AnimationTime,
    ) -> Self {
        Self::new(EventType::PlaybackPaused, animation_id, timestamp)
            .with_player_id(player_id)
            .with_animation_time(animation_time)
            .with_message("Animation playback paused")
    }

    /// Create a playback stopped event
    #[inline]
    pub fn playback_stopped(
        animation_id: impl Into<String>,
        player_id: impl Into<String>,
        timestamp: AnimationTime,
    ) -> Self {
        Self::new(EventType::PlaybackStopped, animation_id, timestamp)
            .with_player_id(player_id)
            .with_message("Animation playback stopped")
    }

    /// Create a playback ended event
    #[inline]
    pub fn playback_ended(
        animation_id: impl Into<String>,
        player_id: impl Into<String>,
        timestamp: AnimationTime,
        animation_time: AnimationTime,
    ) -> Self {
        Self::new(EventType::PlaybackEnded, animation_id, timestamp)
            .with_player_id(player_id)
            .with_animation_time(animation_time)
            .with_message("Animation playback ended")
    }

    /// Create a time changed event
    #[inline]
    pub fn time_changed(
        animation_id: impl Into<String>,
        player_id: impl Into<String>,
        timestamp: AnimationTime,
        old_time: AnimationTime,
        new_time: AnimationTime,
    ) -> Self {
        Self::new(EventType::TimeChanged, animation_id, timestamp)
            .with_player_id(player_id)
            .with_animation_time(new_time)
            .with_data("old_time", old_time.as_seconds().to_string())
            .with_data("new_time", new_time.as_seconds().to_string())
            .with_message(format!(
                "Animation time changed from {:.3}s to {:.3}s",
                old_time.as_seconds(),
                new_time.as_seconds()
            ))
    }

    /// Create a keypoint reached event
    #[inline]
    pub fn keypoint_reached(
        animation_id: impl Into<String>,
        player_id: impl Into<String>,
        timestamp: AnimationTime,
        track_id: TrackId,
        keypoint_id: KeypointId,
        value: Value,
        animation_time: AnimationTime,
    ) -> Self {
        Self::new(EventType::KeypointReached, animation_id, timestamp)
            .with_player_id(player_id)
            .with_animation_time(animation_time)
            .with_track_id(track_id)
            .with_keypoint_id(keypoint_id)
            .with_value(value)
            .with_message(format!(
                "Keypoint reached at {:.3}s",
                animation_time.as_seconds()
            ))
    }

    /// Create a track added event
    #[inline]
    pub fn track_added(
        animation_id: impl Into<String>,
        timestamp: AnimationTime,
        track_id: TrackId,
        track_name: impl Into<String>,
    ) -> Self {
        let track_name = track_name.into();
        Self::new(EventType::TrackAdded, animation_id, timestamp)
            .with_track_id(track_id)
            .with_data("track_name", track_name.clone())
            .with_message(format!("Track '{}' added", track_name))
    }

    /// Create a track removed event
    #[inline]
    pub fn track_removed(
        animation_id: impl Into<String>,
        timestamp: AnimationTime,
        track_id: TrackId,
        track_name: impl Into<String>,
    ) -> Self {
        let track_name = track_name.into();
        Self::new(EventType::TrackRemoved, animation_id, timestamp)
            .with_track_id(track_id)
            .with_data("track_name", track_name.clone())
            .with_message(format!("Track '{}' removed", track_name))
    }

    /// Create a performance warning event
    #[inline]
    pub fn performance_warning(
        animation_id: impl Into<String>,
        player_id: impl Into<String>,
        timestamp: AnimationTime,
        metric: impl Into<String>,
        value: f64,
        threshold: f64,
    ) -> Self {
        let metric = metric.into();
        Self::new(EventType::PerformanceWarning, animation_id, timestamp)
            .with_player_id(player_id)
            .with_data("metric", metric.clone())
            .with_data("value", value.to_string())
            .with_data("threshold", threshold.to_string())
            .with_message(format!(
                "Performance warning: {} = {:.3} (threshold: {:.3})",
                metric, value, threshold
            ))
    }

    /// Create an error event
    #[inline]
    pub fn error(
        animation_id: impl Into<String>,
        player_id: Option<String>,
        timestamp: AnimationTime,
        error_message: impl Into<String>,
    ) -> Self {
        let error_message = error_message.into();
        Self::new(EventType::Error, animation_id, timestamp)
            .with_message(error_message.clone())
            .with_data("error", error_message)
            .with_player_id(player_id.unwrap())
    }
}

/// Event listener trait for handling animation events
pub trait EventListener: Send + Sync {
    /// Handle an animation event
    fn on_event(&mut self, event: &AnimationEvent);

    /// Get the event types this listener is interested in
    fn interested_events(&self) -> Vec<EventType> {
        // Default: interested in all events
        vec![]
    }

    /// Check if this listener is interested in a specific event type
    fn is_interested_in(&self, event_type: &EventType) -> bool {
        let interested = self.interested_events();
        interested.is_empty() || interested.contains(event_type)
    }
}

/// Event dispatcher for managing event listeners and dispatching events
pub struct EventDispatcher {
    listeners: Vec<Box<dyn EventListener>>,
    event_queue: Vec<AnimationEvent>,
    max_queue_size: usize,
    enabled: bool,
}

impl EventDispatcher {
    /// Create a new event dispatcher
    pub fn new() -> Self {
        Self {
            listeners: Vec::new(),
            event_queue: Vec::new(),
            max_queue_size: 1000,
            enabled: true,
        }
    }

    /// Add an event listener
    pub fn add_listener(&mut self, listener: Box<dyn EventListener>) {
        self.listeners.push(listener);
    }

    /// Remove all listeners
    pub fn clear_listeners(&mut self) {
        self.listeners.clear();
    }

    /// Queues an event for later dispatching to interested listeners.
    /// Events are dispatched when `process_queue` is called.
    pub fn dispatch(&mut self, event: AnimationEvent) {
        if !self.enabled {
            return;
        }

        // Add to queue if we have space
        if self.event_queue.len() < self.max_queue_size {
            self.event_queue.push(event); // No need to clone here, as it's moved into the queue
        }
    }

    /// Processes all queued events, dispatching them to interested listeners.
    /// This method clears the internal event queue.
    pub fn process_queue(&mut self) {
        let events = std::mem::take(&mut self.event_queue);
        for event in events {
            // Dispatch to listeners
            for listener in &mut self.listeners {
                if listener.is_interested_in(&event.event_type) {
                    listener.on_event(&event);
                }
            }
        }
    }

    /// Get the number of queued events
    pub fn queue_len(&self) -> usize {
        self.event_queue.len()
    }

    /// Clear the event queue
    pub fn clear_queue(&mut self) {
        self.event_queue.clear();
    }

    /// Enable or disable event dispatching
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.clear_queue();
        }
    }

    /// Check if event dispatching is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set the maximum queue size
    pub fn set_max_queue_size(&mut self, size: usize) {
        self.max_queue_size = size;
        if self.event_queue.len() > size {
            self.event_queue.truncate(size);
        }
    }

    /// Get the maximum queue size
    pub fn max_queue_size(&self) -> usize {
        self.max_queue_size
    }

    /// Get the number of registered listeners
    pub fn listener_count(&self) -> usize {
        self.listeners.len()
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple event listener that logs events
pub struct LoggingEventListener {
    interested_events: Vec<EventType>,
}

impl LoggingEventListener {
    /// Create a new logging event listener
    pub fn new() -> Self {
        Self {
            interested_events: vec![],
        }
    }

    /// Create a logging listener for specific event types
    pub fn for_events(events: Vec<EventType>) -> Self {
        Self {
            interested_events: events,
        }
    }
}

impl Default for LoggingEventListener {
    fn default() -> Self {
        Self::new()
    }
}

impl EventListener for LoggingEventListener {
    fn on_event(&mut self, event: &AnimationEvent) {
        println!(
            "[{}] {}: {} - {}",
            event.timestamp.as_seconds(),
            event.event_type.name(),
            event.animation_id,
            event.message.as_deref().unwrap_or("No message")
        );
    }

    fn interested_events(&self) -> Vec<EventType> {
        self.interested_events.clone()
    }
}

/// Event listener that collects events for testing
pub struct CollectingEventListener {
    events: Vec<AnimationEvent>,
    interested_events: Vec<EventType>,
}

impl CollectingEventListener {
    /// Create a new collecting event listener
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            interested_events: vec![],
        }
    }

    /// Create a collecting listener for specific event types
    pub fn for_events(events: Vec<EventType>) -> Self {
        Self {
            events: Vec::new(),
            interested_events: events,
        }
    }

    /// Get all collected events
    pub fn events(&self) -> &[AnimationEvent] {
        &self.events
    }

    /// Get the number of collected events
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Clear collected events
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Get events of a specific type
    pub fn events_of_type(&self, event_type: &EventType) -> Vec<&AnimationEvent> {
        self.events
            .iter()
            .filter(|e| &e.event_type == event_type)
            .collect()
    }
}

impl Default for CollectingEventListener {
    fn default() -> Self {
        Self::new()
    }
}

impl EventListener for CollectingEventListener {
    fn on_event(&mut self, event: &AnimationEvent) {
        self.events.push(event.clone());
    }

    fn interested_events(&self) -> Vec<EventType> {
        self.interested_events.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_classification() {
        assert!(EventType::PlaybackStarted.is_playback_event());
        assert!(!EventType::PlaybackStarted.is_modification_event());
        assert!(!EventType::PlaybackStarted.is_diagnostic_event());

        assert!(EventType::TrackAdded.is_modification_event());
        assert!(!EventType::TrackAdded.is_playback_event());

        assert!(EventType::Error.is_diagnostic_event());
        assert!(!EventType::Error.is_playback_event());
    }

    #[test]
    fn test_event_creation() {
        let timestamp = AnimationTime::zero();
        let event = AnimationEvent::playback_started("test_anim", "player_1", timestamp);

        assert_eq!(event.event_type, EventType::PlaybackStarted);
        assert_eq!(event.animation_id, "test_anim");
        assert_eq!(event.player_id, Some("player_1".to_string()));
        assert!(event.message.is_some());
    }

    #[test]
    fn test_event_dispatcher() {
        let mut dispatcher = EventDispatcher::new();
        let _listener = CollectingEventListener::new();

        let event = AnimationEvent::new(
            EventType::PlaybackStarted,
            "test_anim",
            AnimationTime::zero(),
        );

        dispatcher.add_listener(Box::new(CollectingEventListener::new()));
        dispatcher.dispatch(event.clone());

        assert_eq!(dispatcher.queue_len(), 1);
    }

    #[test]
    fn test_event_listener_filtering() {
        let listener = CollectingEventListener::for_events(vec![EventType::PlaybackStarted]);

        assert!(listener.is_interested_in(&EventType::PlaybackStarted));
        assert!(!listener.is_interested_in(&EventType::PlaybackStopped));
    }

    #[test]
    fn test_event_data() {
        let event = AnimationEvent::new(EventType::TimeChanged, "test_anim", AnimationTime::zero())
            .with_data("old_time", "0.0")
            .with_data("new_time", "1.0");

        assert_eq!(event.get_data("old_time"), Some("0.0"));
        assert_eq!(event.get_data("new_time"), Some("1.0"));
        assert_eq!(event.get_data("missing"), None);
    }

    #[test]
    fn test_performance_warning_event() {
        let event = AnimationEvent::performance_warning(
            "test_anim",
            "player_1",
            AnimationTime::zero(),
            "fps",
            45.0,
            60.0,
        );

        assert_eq!(event.event_type, EventType::PerformanceWarning);
        assert_eq!(event.get_data("metric"), Some("fps"));
        assert_eq!(event.get_data("value"), Some("45"));
        assert_eq!(event.get_data("threshold"), Some("60"));
    }
}
