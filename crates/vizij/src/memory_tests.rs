//! Does the device retain? A heap-floor probe, and the device run without a
//! bridge — the control the live-ROS leak test (`ros2_tests`) reads against.
//!
//! The measurement is the process' live heap: [`Counting`] wraps the system
//! allocator and keeps a running total of what it has handed out and not taken
//! back. That total is sampled repeatedly and reduced to its **floor** — the
//! lowest reading in a window. Transient allocations (a frame being encoded, a
//! step's working set, another test running alongside) sit above the floor;
//! only memory that is *kept* raises it. Comparing the floor before and after a
//! stretch of traffic asks the leak question directly, and the growth in bytes
//! is the failure message.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use arora_types::data::{DataStore, Key, StateChange};
use arora_types::value::Value;
use vizij_arora_hal::RigHal;
use vizij_arora_store::BlackboardStore;

use crate::device::builder_for;
use crate::frames::{encode_frame, FrameFormat, FRAME_KEY};

/// Bytes the process has been handed and has not given back.
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

/// The system allocator, counting its live bytes.
struct Counting;

// SAFETY: every method forwards to `System` with the arguments it was given and
// returns what it returns; the counter is incidental to the allocation.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc_zeroed(layout);
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        System.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = System.realloc(ptr, layout, new_size);
        if !new_ptr.is_null() {
            LIVE_BYTES.fetch_add(new_size, Ordering::Relaxed);
            LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        new_ptr
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// How long a floor window lasts, and how often it samples: long enough to span
/// many device steps, so the lowest reading catches a moment between the step's
/// own allocations.
const WINDOW: Duration = Duration::from_secs(2);
const SAMPLE: Duration = Duration::from_millis(50);

/// The lowest live-heap reading over [`WINDOW`]. Two floors taken around a
/// stretch of traffic differ by what that traffic left behind.
pub(crate) async fn heap_floor() -> usize {
    let mut floor = usize::MAX;
    let deadline = tokio::time::Instant::now() + WINDOW;
    while tokio::time::Instant::now() < deadline {
        floor = floor.min(LIVE_BYTES.load(Ordering::Relaxed));
        tokio::time::sleep(SAMPLE).await;
    }
    floor
}

/// The share of what crossed the seams that the heap floor may keep. A device
/// that holds nothing back scores zero here (the bridge-less control does);
/// a seam that queues what passes through it scores the whole of it.
pub(crate) const KEPT_BUDGET: usize = 4;

/// A deterministic RGBA gradient: a stand-in for a rendered face, so a PNG
/// frame compresses like an image rather than like noise.
fn gradient(side: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((side * side * 4) as usize);
    for y in 0..side {
        for x in 0..side {
            pixels.extend_from_slice(&[
                (x * 255 / side) as u8,
                (y * 255 / side) as u8,
                ((x + y) * 127 / side) as u8,
                255,
            ]);
        }
    }
    pixels
}

/// The view's frame feed, as [`crate::frames`] runs it: one `side`×`side` frame
/// pushed onto the rig's reading feed every `1/rate_hz`, forever, with its
/// encoded size added to `carried`. The pixels are encoded once and the value
/// cloned per push — each push still allocates its own copy of the payload,
/// which is what the probe watches for.
pub(crate) async fn feed_frames(
    rig: RigHal,
    side: u32,
    format: FrameFormat,
    rate_hz: f32,
    carried: &AtomicUsize,
) {
    let frame = encode_frame(&gradient(side), side, side, format);
    let payload = payload_bytes(&frame);
    // A frame that measures zero is a broken probe, not a free frame: every
    // assertion downstream is a comparison against this number, and a silent
    // zero turns "nothing was kept" into "nothing was sent" without saying so.
    assert!(
        payload > 0,
        "a {side}x{side} {format:?} frame measured no payload — payload_bytes no longer finds the \
         buffer in the frame's value shape"
    );
    let period = Duration::from_secs_f32(1.0 / rate_hz);
    loop {
        rig.push_reading(StateChange::set(Key::from(FRAME_KEY), frame.clone()));
        carried.fetch_add(payload, Ordering::Relaxed);
        tokio::time::sleep(period).await;
    }
}

/// The bytes one pushed frame carries: the image message's pixel or codec
/// buffer. Summed over the value rather than read from a named field, so the
/// probe follows the message shape instead of pinning it — the pixels are the
/// only bulk in either `sensor_msgs` image, whatever the fields around them
/// are called.
fn payload_bytes(value: &Value) -> usize {
    match value {
        Value::ArrayU8(data) => data.len(),
        Value::Structure(structure) => structure
            .fields
            .iter()
            .map(|field| payload_bytes(&field.value))
            .sum(),
        _ => 0,
    }
}

/// A face-shaped graph: one free input fanned out over [`FAN_OUT`] actuated
/// keys — the shape a rig has, and the traffic a bridge sees (every output key
/// written every step).
pub(crate) fn fan_out_spec() -> String {
    let mut nodes = vec![serde_json::json!(
        { "id": "in", "type": "input", "params": { "path": "face/mouth/open", "value": 0.0 } }
    )];
    let mut edges = Vec::new();
    for i in 0..FAN_OUT {
        nodes.push(serde_json::json!(
            { "id": format!("out{i}"), "type": "output", "params": { "path": format!("rig/joint{i}") } }
        ));
        edges.push(serde_json::json!(
            { "from": { "node_id": "in" }, "to": { "node_id": format!("out{i}"), "input": "in" } }
        ));
    }
    serde_json::json!({ "nodes": nodes, "edges": edges }).to_string()
}

/// How many keys the graph writes every step.
pub(crate) const FAN_OUT: usize = 16;

/// The step the device holds, and the sleep between steps — a running vizij's
/// cadence.
pub(crate) const STEP: Duration = Duration::from_millis(16);

/// The device alone — no bridge — under the heaviest thing that crosses its
/// seams: the view's raw frame feed landing in the store as a `view/frame`
/// reading, while the graph writes its keys every step.
///
/// The control for the live-ROS leak test: a rising floor here is the device
/// (store, HAL feed, runtime, graph) retaining; a flat floor here and a rising
/// one there puts the retention on the bridge.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_device_alone_keeps_a_flat_heap_under_a_frame_feed() {
    let _ = env_logger::builder()
        .parse_filters("warn")
        .is_test(true)
        .try_init();

    let rig = RigHal::new();
    let store = BlackboardStore::new();
    let mut arora = builder_for(&fan_out_spec(), rig.clone(), store.clone(), &[])
        .expect("build the device")
        .build()
        .expect("build arora");

    let device = async {
        loop {
            arora.step(STEP).expect("step");
            tokio::time::sleep(STEP).await;
        }
    };
    tokio::pin!(device);

    let carried = AtomicUsize::new(0);
    let frames = feed_frames(rig.clone(), 128, FrameFormat::Raw, 20.0, &carried);
    tokio::pin!(frames);

    let measure = async {
        // Warm-up: the graph's first steps, the store's keys, the frame value's
        // buffers — allocated once and then held, which is not a leak.
        tokio::time::sleep(Duration::from_secs(3)).await;
        let before = heap_floor().await;
        let carried_before = carried.load(Ordering::Relaxed);
        tokio::time::sleep(Duration::from_secs(20)).await;
        let after = heap_floor().await;
        let carried = carried.load(Ordering::Relaxed) - carried_before;
        (before, after, carried, store.snapshot().storage.len())
    };
    tokio::pin!(measure);

    let (before, after, carried, keys) = tokio::select! {
        _ = &mut device => unreachable!("the device loop never returns"),
        _ = &mut frames => unreachable!("the frame feed never returns"),
        measured = &mut measure => measured,
    };

    let growth = after.saturating_sub(before);
    eprintln!(
        "[device] heap floor {before} -> {after} bytes ({growth} kept) while {carried} bytes of \
         frames crossed the seams; {keys} store keys"
    );
    assert!(
        growth < carried / KEPT_BUDGET,
        "the device's heap floor rose {growth} bytes while {carried} bytes of frames passed \
         through it — something on the seams is keeping them"
    );
}
