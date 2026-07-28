//! Visual regression: the native render of Quori and Toasty stays close to a
//! committed reference, so a change to the renderer (camera, materials, morphs,
//! the ambient model) that breaks the web-comparison parity fails CI.
//!
//! `#[ignore]`d — it renders on a GPU (lavapipe in CI), so the plain
//! `cargo test` never runs it. CI's `snapshot-regression` job runs it with
//! `--ignored`, `VIZIJ_FIXTURES` pointing at the face GLBs (fetched from
//! vizij-web), and lavapipe as the Vulkan adapter. Without `VIZIJ_FIXTURES` it
//! skips, so a local `cargo test -- --ignored` is a no-op unless you set it.

use std::path::PathBuf;
use std::process::Command;

use image::RgbaImage;

/// (name, GLB filename under `VIZIJ_FIXTURES`, committed reference PNG).
const CASES: &[(&str, &str, &[u8])] = &[
    (
        "quori",
        "Quori_Current_Extended.glb",
        include_bytes!("references/quori.png"),
    ),
    (
        "toasty",
        "Toasty_Current.glb",
        include_bytes!("references/toasty.png"),
    ),
];

/// Mean absolute per-channel difference (0..255) allowed after downscaling both
/// images to 32×32. The downscale averages out subpixel differences between the
/// reference's renderer and CI's lavapipe, while a broken render — blank, wrong
/// pose, wrong colours — still differs grossly at 32×32.
const MAX_MEAN_DIFF: f64 = 10.0;

#[test]
#[ignore = "renders on a GPU/lavapipe; run in the snapshot-regression CI job with VIZIJ_FIXTURES set"]
fn native_render_matches_reference() {
    let Some(fixtures) = std::env::var_os("VIZIJ_FIXTURES").map(PathBuf::from) else {
        eprintln!("VIZIJ_FIXTURES unset — skipping the snapshot regression");
        return;
    };
    let bin = env!("CARGO_BIN_EXE_vizij");

    for (name, glb, reference_png) in CASES {
        let glb_path = fixtures.join(glb);
        assert!(glb_path.exists(), "fixture missing: {}", glb_path.display());

        let out = std::env::temp_dir().join(format!("vizij-regression-{name}.png"));
        // The neutral face is deterministic (no time-varying program); the
        // reference was rendered the same way (default size + background).
        let status = Command::new(bin)
            .arg("--glb")
            .arg(&glb_path)
            .arg("--snapshot")
            .arg(&out)
            .arg("--no-autoplay")
            .status()
            .expect("run vizij --snapshot");
        assert!(status.success(), "{name}: vizij --snapshot failed");

        let rendered = image::open(&out)
            .unwrap_or_else(|e| panic!("{name}: open render {}: {e}", out.display()))
            .to_rgba8();
        let reference = image::load_from_memory(reference_png)
            .expect("decode reference")
            .to_rgba8();

        let diff = mean_diff(&rendered, &reference);
        eprintln!("{name}: mean 32×32 diff {diff:.2} (max {MAX_MEAN_DIFF})");
        assert!(
            diff < MAX_MEAN_DIFF,
            "{name}: render drifted from the reference (mean diff {diff:.2} >= {MAX_MEAN_DIFF}); \
             re-check the renderer, or regenerate tests/references/{name}.png if the change is intended"
        );
    }
}

/// Mean absolute per-channel difference of the two images downscaled to 32×32.
fn mean_diff(a: &RgbaImage, b: &RgbaImage) -> f64 {
    let a = image::imageops::thumbnail(a, 32, 32);
    let b = image::imageops::thumbnail(b, 32, 32);
    let sum: u64 = a
        .pixels()
        .zip(b.pixels())
        .flat_map(|(pa, pb)| {
            pa.0.iter()
                .zip(pb.0.iter())
                .map(|(x, y)| (i32::from(*x) - i32::from(*y)).unsigned_abs() as u64)
        })
        .sum();
    sum as f64 / (32.0 * 32.0 * 4.0)
}
