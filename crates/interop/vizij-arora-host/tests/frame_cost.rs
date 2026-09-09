//! A frame costs one copy of its pixels, not one `Value` per byte.
//!
//! Serde walks a `Vec<u8>` as a sequence, so seeding an image message the
//! obvious way materialises a 72-byte `Value` per pixel byte before packing
//! them back — ~110 MB and ~430 ms for one full-size frame, on the thread that
//! just rendered it. `frames` bypasses serde for the payload. Nothing else
//! would catch a regression here: the memory probes in `vizij` reduce to the
//! heap *floor*, and transient allocation sits above the floor by definition.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::UNIX_EPOCH;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
        PEAK.fetch_max(live, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// Peak live bytes above the starting point while `build` runs.
fn peak_while<T>(build: impl FnOnce() -> T) -> usize {
    let base = LIVE.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let value = build();
    let peak = PEAK.load(Ordering::Relaxed).saturating_sub(base);
    std::hint::black_box(value);
    peak
}

/// The default view is 763x486; a raw frame is its pixels four bytes apiece.
const WIDTH: u32 = 763;
const HEIGHT: u32 = 486;

/// One test, not two: the counter above is process-wide, so a second test
/// allocating in parallel would land inside this one's measurement.
#[test]
fn a_frame_costs_about_one_copy_of_its_payload() {
    // The type and registry are built once behind a `OnceLock`; warm both so
    // that allocation is not charged to a frame.
    let _ = vizij_arora_host::frames::raw_frame(1, 1, vec![0; 4], UNIX_EPOCH);
    let _ = vizij_arora_host::frames::compressed_frame("png", vec![0; 4], UNIX_EPOCH);

    // The caller's buffer is one copy; the message around it is a handful of
    // scalars. Four leaves room for an allocator's rounding without leaving
    // room for a `Value` per byte, which would be ~72x.
    let pixels = vec![7u8; (WIDTH * HEIGHT * 4) as usize];
    let raw = peak_while(|| {
        vizij_arora_host::frames::raw_frame(WIDTH, HEIGHT, pixels.clone(), UNIX_EPOCH)
    });
    assert!(
        raw < pixels.len() * 4,
        "building a {WIDTH}x{HEIGHT} raw frame peaked at {raw} bytes for a {} byte payload — the \
         pixels are going through serde as a sequence again",
        pixels.len()
    );

    let encoded = vec![7u8; 417_746];
    let compressed = peak_while(|| {
        vizij_arora_host::frames::compressed_frame("png", encoded.clone(), UNIX_EPOCH)
    });
    assert!(
        compressed < encoded.len() * 4,
        "building a compressed frame peaked at {compressed} bytes for a {} byte payload — the \
         codec bytes are going through serde as a sequence again",
        encoded.len()
    );
}
