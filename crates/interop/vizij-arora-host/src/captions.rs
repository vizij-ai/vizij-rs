//! How a host shows a caption ([`standard::CAPTION_TEXT`]): in chunks of a
//! few words, each held long enough to read, in the colour of whoever says
//! it. The rules are the incumbent ROS4HRI face's, so a ROS4HRI deployment
//! reads the same captions on either face:
//!
//! - **Chunks** — the text splits on whitespace into chunks of
//!   [`CHUNK_WORDS`] words. A chunk is held for its words at
//!   [`WORDS_PER_MINUTE`], at least [`MIN_CHUNK_SECONDS`]; the last one of a
//!   caption is held [`LAST_CHUNK_EXTRA_SECONDS`] longer.
//! - **Queueing** — a caption's chunks play after those still queued, so a
//!   caption arriving while another shows waits its turn instead of cutting
//!   it off.
//! - **Colour** — the face's own speech ([`standard::CAPTION_SPEAKER_SELF`])
//!   in [`SELF_COLOR`], anyone else's in [`OTHER_COLOR`].
//!
//! Drawing is the host's (the native view's overlay); this module is the
//! schedule both hosts would otherwise write twice.

use std::collections::VecDeque;

use crate::standard;

/// Words per chunk.
pub const CHUNK_WORDS: usize = 8;

/// The reading pace a chunk is held for.
pub const WORDS_PER_MINUTE: f32 = 200.0;

/// The shortest a chunk is held, seconds.
pub const MIN_CHUNK_SECONDS: f32 = 2.0;

/// How much longer a caption's last chunk is held, seconds: the caption's
/// end stays readable after the voice stops.
pub const LAST_CHUNK_EXTRA_SECONDS: f32 = 1.5;

/// The colour of the face's own speech, sRGB.
pub const SELF_COLOR: [f32; 3] = [0.0, 0.4, 0.7];

/// The colour of anyone else's speech, sRGB.
pub const OTHER_COLOR: [f32; 3] = [1.0, 1.0, 1.0];

/// One chunk of a caption: the words shown together, how long they stay,
/// and who says them.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub text: String,
    pub seconds: f32,
    pub speaker: String,
}

impl Chunk {
    /// The colour the chunk shows in.
    pub fn color(&self) -> [f32; 3] {
        speaker_color(&self.speaker)
    }
}

/// The colour a caption by `speaker` shows in.
pub fn speaker_color(speaker: &str) -> [f32; 3] {
    if speaker == standard::CAPTION_SPEAKER_SELF {
        SELF_COLOR
    } else {
        OTHER_COLOR
    }
}

/// The chunks `text` shows as, in order. None for a text without words.
pub fn chunks(text: &str, speaker: &str) -> Vec<Chunk> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let count = words.chunks(CHUNK_WORDS).count();
    words
        .chunks(CHUNK_WORDS)
        .enumerate()
        .map(|(index, words)| {
            let reading = words.len() as f32 * 60.0 / WORDS_PER_MINUTE;
            let mut seconds = reading.max(MIN_CHUNK_SECONDS);
            if index + 1 == count {
                seconds += LAST_CHUNK_EXTRA_SECONDS;
            }
            Chunk {
                text: words.join(" "),
                seconds,
                speaker: speaker.to_string(),
            }
        })
        .collect()
}

/// A face's captions over time: what the caption keys last said, the chunks
/// still to show, and the one showing.
#[derive(Debug, Default)]
pub struct CaptionQueue {
    /// The caption keys as last read, `(text, speaker)`: a new caption is a
    /// change of either.
    last: Option<(String, String)>,
    queued: VecDeque<Chunk>,
    /// The chunk showing and the seconds it has left.
    showing: Option<(Chunk, f32)>,
}

impl CaptionQueue {
    /// Read the caption keys: a caption that differs from the last one read
    /// queues its chunks. The first read only sets the baseline, so a face
    /// loaded after a caption was written does not replay it; an empty text
    /// queues nothing.
    pub fn read(&mut self, text: &str, speaker: &str) {
        let read = (text.to_string(), speaker.to_string());
        match &self.last {
            Some(last) if *last == read => return,
            Some(_) => self.queued.extend(chunks(text, speaker)),
            None => {}
        }
        self.last = Some(read);
    }

    /// Let `seconds` pass: the chunk showing ages, and once its time is up
    /// the next queued one shows.
    pub fn advance(&mut self, seconds: f32) {
        if let Some((_, left)) = &mut self.showing {
            *left -= seconds;
            if *left <= 0.0 {
                self.showing = None;
            }
        }
        if self.showing.is_none() {
            self.showing = self.queued.pop_front().map(|chunk| {
                let left = chunk.seconds;
                (chunk, left)
            });
        }
    }

    /// The chunk to draw now, if any.
    pub fn showing(&self) -> Option<&Chunk> {
        self.showing.as_ref().map(|(chunk, _)| chunk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eight words a chunk; each held for its words at 200 wpm, at least 2 s,
    /// the last one 1.5 s longer.
    #[test]
    fn a_caption_splits_into_timed_chunks_of_eight_words() {
        let text = "one two three four five six seven eight nine ten \
                    eleven twelve thirteen fourteen fifteen sixteen seventeen";
        let chunks = chunks(text, "__system__");
        let shown: Vec<(&str, f32)> = chunks
            .iter()
            .map(|c| (c.text.as_str(), c.seconds))
            .collect();
        assert_eq!(
            shown,
            [
                ("one two three four five six seven eight", 2.4),
                (
                    "nine ten eleven twelve thirteen fourteen fifteen sixteen",
                    2.4
                ),
                ("seventeen", 2.0 + 1.5),
            ]
        );
        assert!(chunks.iter().all(|c| c.color() == SELF_COLOR));
        assert!(super::chunks(" \t\n", "").is_empty());
    }

    #[test]
    fn the_face_s_own_speech_has_its_own_colour() {
        assert_eq!(speaker_color(standard::CAPTION_SPEAKER_SELF), SELF_COLOR);
        assert_eq!(speaker_color("person_1"), OTHER_COLOR);
        assert_eq!(speaker_color(""), OTHER_COLOR);
    }

    /// A new caption queues behind the one showing; each chunk shows for its
    /// time, then the next.
    #[test]
    fn captions_queue_and_show_in_turn() {
        let mut queue = CaptionQueue::default();
        queue.read("", "");
        queue.advance(0.0);
        assert_eq!(queue.showing(), None);

        queue.read("Hello there.", "__system__");
        queue.read("Hi!", "person_1");
        queue.advance(0.0);
        assert_eq!(
            queue.showing().map(|c| c.text.as_str()),
            Some("Hello there.")
        );
        queue.advance(3.0);
        assert_eq!(
            queue.showing().map(|c| c.text.as_str()),
            Some("Hello there.")
        );
        queue.advance(0.6);
        let showing = queue.showing().expect("the second caption");
        assert_eq!(
            (showing.text.as_str(), showing.color()),
            ("Hi!", OTHER_COLOR)
        );
        queue.advance(3.5);
        assert_eq!(queue.showing(), None);
    }

    /// The caption already in the keys when a face starts reading them is
    /// not shown, nor is the same caption read again.
    #[test]
    fn only_a_changed_caption_is_shown() {
        let mut queue = CaptionQueue::default();
        queue.read("Said before this face loaded.", "__system__");
        queue.advance(0.0);
        assert_eq!(queue.showing(), None);
        queue.read("Said before this face loaded.", "__system__");
        queue.advance(0.0);
        assert_eq!(queue.showing(), None);
        queue.read("Said before this face loaded.", "person_1");
        queue.advance(0.0);
        assert!(queue.showing().is_some(), "a new speaker is a new caption");
    }
}
