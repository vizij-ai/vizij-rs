//! What loading a Vizij costs: in instructions retired, a structural reload
//! — the face loaded and ready, then loaded again from its bytes under the
//! same id, as `@vizij/runtime-react` reloads it — and in memory, faces
//! loaded and unloaded in turn. Instruction counts hold where wall time does
//! not, on a machine that is busy with other work.
//!
//! ```bash
//! cargo run --profile profiling -p vizij --example load_cost -- face.glb
//! cargo run --profile profiling -p vizij --example load_cost -- --churn 20 a.glb b.glb
//! ```
//!
//! The `profiling` profile is the release code without its fat LTO, which a
//! build of the whole view takes long and much memory to link.
//!
//! A reload counts `describe` (`FaceMeta::from_glb_bytes`), the device build
//! as `web::load_vizij` builds it with its defaults, part by part (the face's
//! composition, its keys staged and declared, the builder and its graph
//! parse, the Arora build, the animations, the program), and then each view
//! frame until the reloaded face is ready. Each count is the calling
//! thread's and, on macOS, the whole process's: the view runs on one thread
//! as in the browser, but wgpu's Metal driver works on threads of its own.
//! The thread's count needs macOS (`thread_selfcounts`) or Linux with perf
//! events; the process's needs macOS (`proc_pid_rusage`), and reads `-`
//! elsewhere.
//!
//! Options:
//! - `--churn N`: instead, load and unload the GLBs given in turn for `N`
//!   cycles, as the authoring preset churn does: each cycle takes the shown
//!   face down, describes and loads the next one, steps its device and
//!   draws until it is ready, draining the device's changes each frame, then
//!   draws a second more. It prints, per cycle, the live bytes, the peak of
//!   live bytes and the largest single allocation, from a counting global
//!   allocator, and the largest drain;
//! - `--overlap`: with `--churn`, a second face (the page's empty-state demo
//!   face, say) loads and draws beside each cycle's.
//!
//! Every allocation of 16 MiB or more prints where it comes from, on stderr.

use std::time::{Duration, Instant};

use vizij::face::{self, FaceConfig, LoadedFace, ProgramSelect};
use vizij::view::meta::FaceMeta;
use vizij::view::ViewEvent;
use vizij_arora_hal::RigHal;
use vizij_arora_store::BlackboardStore;

#[path = "cost/counter.rs"]
mod counter;
#[path = "cost/view.rs"]
mod offscreen;

/// The calling thread's instructions retired so far.
fn instructions() -> u64 {
    counter::instructions().expect("an instruction counter: macOS, or Linux with perf events")
}

/// Instructions retired by every thread of this process so far, `None`
/// where none is read (an OS other than macOS).
fn process_instructions() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut u64) -> i32;
        }
        /// `RUSAGE_INFO_V4`, whose `ri_instructions` follows a 16-byte UUID
        /// and 29 other `u64` fields.
        const RUSAGE_INFO_V4: i32 = 4;
        const RI_INSTRUCTIONS: usize = 2 + 29;
        let mut info = [0u64; 64];
        // SAFETY: the buffer is larger than `rusage_info_v4`, which the call
        // writes.
        let status = unsafe {
            proc_pid_rusage(std::process::id() as i32, RUSAGE_INFO_V4, info.as_mut_ptr())
        };
        (status == 0).then_some(info[RI_INSTRUCTIONS])
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// What a section cost: the thread's and the process's instructions, and
/// the wall time.
struct Counted<T> {
    value: T,
    thread: u64,
    process: Option<u64>,
    wall: Duration,
}

fn counted<T>(f: impl FnOnce() -> T) -> Counted<T> {
    let (thread, process, wall) = (instructions(), process_instructions(), Instant::now());
    let value = f();
    Counted {
        value,
        thread: instructions() - thread,
        process: process_instructions()
            .zip(process)
            .map(|(after, before)| after - before),
        wall: wall.elapsed(),
    }
}

/// A count, `-` where none is read.
fn count(n: Option<u64>) -> String {
    n.map_or_else(|| "-".to_string(), |n| n.to_string())
}

/// One device, built as `web::load_vizij` builds it with its defaults.
struct Device {
    arora: arora::Arora,
    rig: RigHal,
    meta: FaceMeta,
}

/// The device, each part of its build's thread instructions printed when
/// `report`.
fn device(glb: &[u8], report: bool) -> Device {
    let part = |name: &str, at: u64| {
        if report {
            println!("  {name:<22} {:>14} thread", instructions() - at);
        }
    };
    let config = FaceConfig {
        wanted: ["rig", "pose-driver", "pose", "standard-adaptation"]
            .map(String::from)
            .to_vec(),
        program: ProgramSelect::Auto,
        stage_neutral: true,
        ros4hri: false,
        #[cfg(feature = "studio")]
        studio: false,
        speech: None,
    };
    let at = instructions();
    let LoadedFace { meta, spec, .. } = face::load_face(glb, &config).expect("load face");
    part("load_face (compose)", at);
    let at = instructions();
    let rig = RigHal::new();
    let store = BlackboardStore::new();
    face::stage_neutral_pose(&store, &meta);
    face::declare_keys(
        &store,
        &spec,
        &meta.bundle.programs,
        &meta.bundle.neutral_stage_writes(),
        &face::rig_ranges(&meta.bundle),
    );
    part("stage and declare keys", at);
    let at = instructions();
    let builder =
        face::builder_for(&spec, rig.clone(), store, &meta.bundle.skills, None).expect("builder");
    part("builder (graph parse)", at);
    let at = instructions();
    let mut arora = builder.build().expect("build");
    part("arora build", at);
    let at = instructions();
    face::load_animations(&mut arora, &meta.bundle).expect("animations");
    part("load animations", at);
    let at = instructions();
    face::spawn_program(&mut arora, &meta.bundle, &config.program).expect("program");
    part("spawn program", at);
    Device { arora, rig, meta }
}

/// The view's cached render pipelines.
fn pipelines(app: &bevy::prelude::App) -> usize {
    app.sub_app(bevy::render::RenderApp)
        .world()
        .resource::<bevy::render::render_resource::PipelineCache>()
        .pipelines()
        .count()
}

mod tracking {
    //! The global allocator, counting: live bytes, the peak of live bytes and
    //! the largest single allocation since the last `reset_cycle`, and where
    //! each allocation of at least `REPORT_BYTES` comes from.
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub static LIVE: AtomicUsize = AtomicUsize::new(0);
    pub static PEAK: AtomicUsize = AtomicUsize::new(0);
    pub static LARGEST: AtomicUsize = AtomicUsize::new(0);
    /// Allocations this large print where they come from.
    const REPORT_BYTES: usize = 16 << 20;

    thread_local! {
        /// Whether this thread is printing a report, whose own allocations
        /// report nothing.
        static IN_REPORT: Cell<bool> = const { Cell::new(false) };
    }

    pub struct Counting;

    fn note(size: usize) {
        let live = LIVE.fetch_add(size, Ordering::Relaxed) + size;
        PEAK.fetch_max(live, Ordering::Relaxed);
        LARGEST.fetch_max(size, Ordering::Relaxed);
        if size >= REPORT_BYTES {
            let _ = IN_REPORT.try_with(|reporting| {
                if reporting.replace(true) {
                    return;
                }
                let trace = std::backtrace::Backtrace::force_capture().to_string();
                let frames: Vec<&str> = trace
                    .lines()
                    .filter(|line| line.contains("::") && !line.contains("at "))
                    .filter(|line| {
                        !["tracking::", "alloc::", "std::"]
                            .iter()
                            .any(|skipped| line.contains(skipped))
                    })
                    .take(12)
                    .collect();
                eprintln!("allocation of {} MiB\n{}", size >> 20, frames.join("\n"));
                reporting.set(false);
            });
        }
    }

    // SAFETY: every call forwards to `System` with the caller's layout, and
    // only counts what `System` returned.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            // SAFETY: the caller upholds `alloc`'s contract.
            let p = unsafe { System.alloc(layout) };
            if !p.is_null() {
                note(layout.size());
            }
            p
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            // SAFETY: the caller upholds `alloc_zeroed`'s contract.
            let p = unsafe { System.alloc_zeroed(layout) };
            if !p.is_null() {
                note(layout.size());
            }
            p
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: the caller upholds `dealloc`'s contract.
            unsafe { System.dealloc(ptr, layout) };
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            // SAFETY: the caller upholds `realloc`'s contract.
            let p = unsafe { System.realloc(ptr, layout, new_size) };
            if !p.is_null() {
                LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
                note(new_size);
            }
            p
        }
    }

    /// Start a cycle's peak and largest allocation from now.
    pub fn reset_cycle() {
        PEAK.store(LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
        LARGEST.store(0, Ordering::Relaxed);
    }
}

#[global_allocator]
static ALLOCATOR: tracking::Counting = tracking::Counting;

/// The page's `drainChanges`: the feed folded into the latest value per key,
/// then that net change as JSON. Its length in bytes.
fn drain(changes: &arora_types::data::Subscription) -> usize {
    let mut latest = std::collections::HashMap::new();
    while let Some(change) = changes.try_recv() {
        latest.extend(change.set);
        latest.extend(change.unset.into_iter().map(|key| (key, None)));
    }
    let mut out = serde_json::Map::new();
    for (key, value) in latest {
        out.insert(key.path, serde_json::to_value(value).unwrap());
    }
    serde_json::Value::Object(out).to_string().len()
}

const DT: Duration = Duration::from_micros(16_667);

/// The face loaded and ready, then reloaded under the same id: `describe`,
/// the device build and each view frame until it is ready, counted.
fn reload(glb: &[u8]) {
    let first = device(glb, false);
    let (mut app, events) = offscreen::view_app(glb, &[(&first.meta, &first.rig)]);
    eprintln!("first load ready, {} pipelines cached", pipelines(&app));

    let described = counted(|| FaceMeta::from_glb_bytes(glb).expect("describe"));
    drop(described.value);
    let built = counted(|| device(glb, true));
    events
        .send(ViewEvent::LoadFace {
            face_id: "face0".into(),
            meta: Box::new(built.value.meta.clone()),
            glb: glb.to_vec(),
            rig: built.value.rig.clone(),
        })
        .unwrap();
    for (name, thread, process, wall) in [
        (
            "describe",
            described.thread,
            described.process,
            described.wall,
        ),
        ("device build", built.thread, built.process, built.wall),
    ] {
        println!(
            "{name:<17} {thread:>14} thread {:>14} process {wall:>8.1?}",
            count(process)
        );
    }
    let mut frames = 0;
    let (mut thread, mut process, mut wall) = (0, Some(0), Duration::ZERO);
    loop {
        let before = pipelines(&app);
        let frame = counted(|| app.update());
        frames += 1;
        thread += frame.thread;
        process = process.zip(frame.process).map(|(sum, frame)| sum + frame);
        wall += frame.wall;
        let ready = vizij::view::all_faces_ready(&mut app);
        println!(
            "frame {frames:>3}         {:>14} thread {:>14} process {:>8.1?}  pipelines {before} -> {}{}",
            frame.thread,
            count(frame.process),
            frame.wall,
            pipelines(&app),
            if ready { "  ready" } else { "" }
        );
        if ready && frames > 1 {
            break;
        }
        assert!(frames < 600, "the reloaded face never got ready");
    }
    println!(
        "view to ready     {thread:>14} thread {:>14} process {wall:>8.1?} over {frames} frames",
        count(process)
    );
}

/// Faces loaded and unloaded in turn for `cycles` cycles, their memory
/// counted (the module's `--churn`).
fn churn(paths: &[String], cycles: usize, overlap: bool) {
    use std::sync::atomic::Ordering;
    let glbs: Vec<Vec<u8>> = paths
        .iter()
        .map(|path| std::fs::read(path).expect("read the GLB"))
        .collect();
    let mut shown = device(&glbs[0], false);
    let mut changes = shown.arora.store().subscribe();
    let (mut app, events) = offscreen::view_app(&glbs[0], &[(&shown.meta, &shown.rig)]);
    let mib = |bytes: usize| bytes as f64 / (1 << 20) as f64;
    println!("cycle  live MiB  peak MiB  largest MiB  drained KiB");
    for cycle in 1..=cycles {
        tracking::reset_cycle();
        events
            .send(ViewEvent::UnloadFace {
                face_id: "face0".into(),
            })
            .unwrap();
        app.update();
        drop(changes);
        drop(shown);
        let demo = overlap.then(|| {
            let demo = device(&glbs[0], false);
            events
                .send(ViewEvent::LoadFace {
                    face_id: "demo".into(),
                    meta: Box::new(demo.meta.clone()),
                    glb: glbs[0].clone(),
                    rig: demo.rig.clone(),
                })
                .unwrap();
            demo
        });
        let glb = &glbs[cycle % glbs.len()];
        drop(FaceMeta::from_glb_bytes(glb).expect("describe"));
        shown = device(glb, false);
        changes = shown.arora.store().subscribe();
        events
            .send(ViewEvent::LoadFace {
                face_id: "face0".into(),
                meta: Box::new(shown.meta.clone()),
                glb: glb.clone(),
                rig: shown.rig.clone(),
            })
            .unwrap();
        let mut drained = 0;
        let mut frames = 0;
        let mut after_ready = 0;
        while after_ready <= 60 {
            shown.arora.step(DT).expect("step");
            drained = drained.max(drain(&changes));
            app.update();
            frames += 1;
            if vizij::view::all_faces_ready(&mut app) {
                after_ready += 1;
            }
            assert!(frames < 2000, "the face never got ready");
        }
        if let Some(demo) = demo {
            events
                .send(ViewEvent::UnloadFace {
                    face_id: "demo".into(),
                })
                .unwrap();
            app.update();
            drop(demo);
        }
        println!(
            "{cycle:>5} {:>9.1} {:>9.1} {:>12.1} {:>12}",
            mib(tracking::LIVE.load(Ordering::Relaxed)),
            mib(tracking::PEAK.load(Ordering::Relaxed)),
            mib(tracking::LARGEST.load(Ordering::Relaxed)),
            drained / 1024
        );
    }
}

fn main() {
    let mut cycles = None;
    let mut overlap = false;
    let mut glbs = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--churn" => {
                cycles = Some(
                    args.next()
                        .and_then(|n| n.parse().ok())
                        .expect("--churn takes a cycle count"),
                )
            }
            "--overlap" => overlap = true,
            glb => glbs.push(glb.to_string()),
        }
    }
    assert!(!glbs.is_empty(), "a GLB path");
    match cycles {
        Some(cycles) => churn(&glbs, cycles, overlap),
        None => reload(&std::fs::read(&glbs[0]).expect("read the GLB")),
    }
}
