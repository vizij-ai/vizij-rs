//! Loader for test_animation.json format
//!
//! This module provides functionality to load animations from the test_animation.json format
//! and convert them to the internal AnimationData representation.

use crate::{AnimationData, AnimationKeypoint, AnimationTime, AnimationTrack, Value};
use serde::Deserialize;

#[derive(Deserialize)]
struct StudioAnimationPoint {
    #[allow(dead_code)]
    id: String,
    stamp: f64,
    value: f64,
    #[serde(rename = "trackId")]
    #[allow(dead_code)]
    track_id: Option<String>,
}

#[derive(Deserialize)]
struct StudioAnimationTrack {
    #[allow(dead_code)]
    id: String,
    name: String,
    points: Vec<StudioAnimationPoint>,
    #[serde(rename = "animatableId")]
    #[allow(dead_code)]
    animatable_id: String,
}

#[derive(Deserialize)]
struct StudioAnimationData {
    id: String,
    name: String,
    tracks: Vec<StudioAnimationTrack>,
    #[serde(default)]
    #[allow(dead_code)]
    groups: serde_json::Value,
    #[serde(default)]
    #[allow(dead_code)]
    transitions: serde_json::Value,
    duration: u64,
}

/// Convert test animation format to internal AnimationData
///
/// # Arguments
/// * `test_data` - The parsed test animation data
///
/// # Returns
/// * `AnimationData` - The converted animation data
fn convert_test_animation(test_data: StudioAnimationData) -> AnimationData {
    let mut animation = AnimationData::new(&test_data.id, &test_data.name);
    let duration_seconds = test_data.duration as f64 / 1000.0;

    for track_data in test_data.tracks {
        let mut track =
            AnimationTrack::new_with_id(&track_data.id, &track_data.name, &track_data.name)
                .unwrap_or_else(|_| AnimationTrack::new(&track_data.name, &track_data.name));

        for point in track_data.points {
            // Convert stamp (0.0-1.0) to time in seconds
            let time_seconds = point.stamp * duration_seconds;
            let time = AnimationTime::from_seconds(time_seconds).unwrap();
            let keypoint = AnimationKeypoint::new(time, Value::Float(point.value));
            track.add_keypoint(keypoint).unwrap();
        }

        animation.add_track(track);
    }

    // Set the duration
    animation.metadata.duration = AnimationTime::from_seconds(duration_seconds).unwrap();
    animation
}

/// Load test animation from JSON string
///
/// # Arguments
/// * `json_str` - The JSON string containing the test animation data
///
/// # Returns
/// * `Result<AnimationData, Box<dyn std::error::Error>>` - The loaded animation or error
///
/// # Example
/// ```rust
/// use animation_player::loaders::load_test_animation_from_json;
///
/// let json = r#"
/// {
///   "id": "test-id",
///   "name": "Test Animation",
///   "tracks": [
///     {
///       "id": "track-id",
///       "name": "test_track",
///       "points": [
///         {
///           "id": "point-id",
///           "stamp": 0.0,
///           "value": 0.0
///         },
///         {
///           "id": "point-id-2",
///           "stamp": 1.0,
///           "value": 10.0
///         }
///       ],
///       "animatableId": "animatable-id"
///     }
///   ],
///   "duration": 5000
/// }
/// "#;
///
/// let animation = load_test_animation_from_json(json).unwrap();
/// assert_eq!(animation.id, "test-id");
/// assert_eq!(animation.name, "Test Animation");
/// ```
pub fn load_test_animation_from_json(
    json_str: &str,
) -> Result<AnimationData, Box<dyn std::error::Error>> {
    let test_data: StudioAnimationData = serde_json::from_str(json_str)?;
    Ok(convert_test_animation(test_data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_conversion() {
        let json = r#"
        {
          "id": "test-id",
          "name": "Test Animation",
          "tracks": [
            {
              "id": "track-id",
              "name": "test_track",
              "points": [
                {
                  "id": "point-id",
                  "stamp": 0.0,
                  "value": 0.0
                },
                {
                  "id": "point-id-2",
                  "stamp": 1.0,
                  "value": 10.0
                }
              ],
              "animatableId": "animatable-id"
            }
          ],
          "duration": 5000
        }
        "#;

        let animation = load_test_animation_from_json(json).unwrap();
        assert_eq!(animation.id, "test-id");
        assert_eq!(animation.name, "Test Animation");
        assert_eq!(
            animation.metadata.duration,
            AnimationTime::from_seconds(5.0).unwrap()
        );
        assert_eq!(animation.tracks.len(), 1);

        let track = animation.tracks.values().next().unwrap();
        assert_eq!(track.name, "test_track");
        assert_eq!(track.target, "test_track");
        assert_eq!(track.keypoints.len(), 2);

        // First keypoint at stamp 0.0 -> time 0.0
        assert_eq!(
            track.keypoints[0].time,
            AnimationTime::from_seconds(0.0).unwrap()
        );
        if let Value::Float(val) = track.keypoints[0].value {
            assert_eq!(val, 0.0);
        }

        // Second keypoint at stamp 1.0 -> time 5.0 (1.0 * 5.0 duration)
        assert_eq!(
            track.keypoints[1].time,
            AnimationTime::from_seconds(5.0).unwrap()
        );
        if let Value::Float(val) = track.keypoints[1].value {
            assert_eq!(val, 10.0);
        }
    }

    #[test]
    fn test_stamp_conversion() {
        let json = r#"
        {
          "id": "test-id",
          "name": "Test Animation",
          "tracks": [
            {
              "id": "track-id",
              "name": "test_track",
              "points": [
                {
                  "id": "point-id",
                  "stamp": 0.5,
                  "value": 5.0
                }
              ],
              "animatableId": "animatable-id"
            }
          ],
          "duration": 2000
        }
        "#;

        let animation = load_test_animation_from_json(json).unwrap();
        let track = animation.tracks.values().next().unwrap();

        // stamp 0.5 with duration 2000ms (2.0s) should be time 1.0s
        assert_eq!(
            track.keypoints[0].time,
            AnimationTime::from_seconds(1.0).unwrap()
        );
    }
}
