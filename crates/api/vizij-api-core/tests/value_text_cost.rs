//! What reading a large value from JSON text costs: the browser runtime's
//! `invoke` and `setValue` hand their values over as text, and a wasm heap
//! grows to the peak of what is live at once. V8 grows a wasm memory a page at
//! a time, each grow costing in the memory's size, so a peak that is a large
//! multiple of the text makes the first read of a large value superlinear in
//! its size. This pins the peak to a small multiple of the text, the same
//! multiple at every size.
//!
//! The measurement is the live heap: [`Counting`] wraps the system allocator
//! and keeps the running total of what it has handed out and not taken back,
//! and the highest total since it was last reset. This file is its own test
//! binary with one test, so nothing else allocates while it measures.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use vizij_api_core::json::parse_value_str;

/// Bytes handed out and not given back.
static LIVE: AtomicUsize = AtomicUsize::new(0);
/// The highest [`LIVE`] since [`reset_peak`].
static PEAK: AtomicUsize = AtomicUsize::new(0);

/// The system allocator, counting its live bytes and their peak.
struct Counting;

fn grew(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

// SAFETY: every method forwards to `System` with the arguments it was given and
// returns what it returns; the counters are incidental to the allocation.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            grew(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc_zeroed(layout);
        if !ptr.is_null() {
            grew(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        System.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = System.realloc(ptr, layout, new_size);
        if !new_ptr.is_null() {
            // A moved block holds both sizes for the copy.
            grew(new_size);
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        new_ptr
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn reset_peak() {
    PEAK.store(LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
}

/// The peak live bytes `f` adds over what was live when it started.
fn peak_of(f: impl FnOnce()) -> usize {
    let base = LIVE.load(Ordering::Relaxed);
    reset_peak();
    f();
    PEAK.load(Ordering::Relaxed) - base
}

/// A clip as the animation module's `load_clip` takes it, in canonical Arora
/// `Value` JSON, with `keypoints` keypoints on one track: the shape and the
/// size per keypoint (~900 bytes) `Runtime.moduleAnimation` answers.
fn clip_json(keypoints: usize) -> String {
    let field =
        |id: &str, value: String| format!(r#"{{"id":"76697a69-6a00-0000-{id}","value":{value}}}"#);
    let transition = || {
        format!(
            r#"{{"structs":{{"elements":[{{"fields":[{},{},{}]}}],"id":"76697a69-6a00-0000-0000-000000000203"}}}}"#,
            field("0203-000000000001", r#"{"str":"linear"}"#.into()),
            field("0203-000000000002", r#"{"f64":0}"#.into()),
            field("0203-000000000003", r#"{"f64":0}"#.into()),
        )
    };
    let keypoint = |i: usize| {
        format!(
            r#"{{"fields":[{},{},{},{},{},{}]}}"#,
            field("0202-000000000001", format!(r#"{{"str":"a:0:{i}"}}"#)),
            field(
                "0202-000000000002",
                format!(r#"{{"i64":{}}}"#, i as u64 * 1_000_000_000)
            ),
            field(
                "0202-000000000003",
                format!(r#"{{"f64":{}}}"#, (i as f64).sin())
            ),
            field("0202-000000000004", transition()),
            field("0202-000000000005", transition()),
            field("0202-000000000006", r#"{"str":""}"#.into()),
        )
    };
    let points: Vec<String> = (0..keypoints).map(keypoint).collect();
    let track = format!(
        r#"{{"fields":[{},{},{},{}]}}"#,
        field("0201-000000000001", r#"{"str":"a:0"}"#.into()),
        field("0201-000000000002", r#"{"str":"face/x"}"#.into()),
        field("0201-000000000003", r#"{"str":"face/x"}"#.into()),
        field(
            "0201-000000000004",
            format!(
                r#"{{"structs":{{"elements":[{}],"id":"76697a69-6a00-0000-0000-000000000202"}}}}"#,
                points.join(",")
            )
        ),
    );
    format!(
        r#"{{"struct":{{"fields":[{},{},{}],"id":"76697a69-6a00-0000-0000-000000000200"}}}}"#,
        field("0200-000000000001", r#"{"str":"A"}"#.into()),
        field(
            "0200-000000000002",
            format!(
                r#"{{"option":{{"i64":{}}}}}"#,
                keypoints as u64 * 1_000_000_000
            )
        ),
        field(
            "0200-000000000003",
            format!(
                r#"{{"structs":{{"elements":[{track}],"id":"76697a69-6a00-0000-0000-000000000201"}}}}"#
            )
        ),
    )
}

/// What the runtime does with a value's text before Arora reads it: parse it
/// in any accepted form and write it back as canonical text.
fn canonical_text(json: &str) -> String {
    let value = parse_value_str(json).expect("the clip parses");
    serde_json::to_string(&value).expect("the clip serializes")
}

/// Most a read may hold at once beyond its text, as a multiple of the text: the
/// parsed `Value` and the canonical text written back (about 3x). Reading
/// through a `serde_json::Value` tree holds about 60x.
const PEAK_PER_TEXT_BYTE: usize = 6;

#[test]
fn reading_a_large_value_holds_a_small_multiple_of_its_text() {
    let mut peaks = Vec::new();
    for keypoints in [1_000, 8_000] {
        let json = clip_json(keypoints);
        let peak = peak_of(|| {
            let canonical = canonical_text(&json);
            assert!(canonical.len() >= json.len() / 2);
        });
        let ratio = peak as f64 / json.len() as f64;
        eprintln!(
            "{keypoints} keypoints: {} text bytes, {peak} peak bytes ({ratio:.1}x)",
            json.len()
        );
        assert!(
            peak <= PEAK_PER_TEXT_BYTE * json.len(),
            "reading {} bytes of text held {peak} bytes at once ({ratio:.1}x the text, \
             more than {PEAK_PER_TEXT_BYTE}x)",
            json.len()
        );
        peaks.push((json.len(), peak));
    }
    // Linear: eight times the text holds about eight times the bytes.
    let [(small_text, small_peak), (large_text, large_peak)] = peaks[..] else {
        unreachable!()
    };
    let growth = (large_peak as f64 / small_peak as f64) / (large_text as f64 / small_text as f64);
    assert!(
        growth < 1.5,
        "the peak grew {growth:.2}x faster than the text from {small_text} to {large_text} bytes"
    );
}
