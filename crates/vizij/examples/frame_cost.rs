//! What a Vizij frame costs, in instructions retired: the device's step,
//! the page's change drain and the view's frame, for faces composed as the
//! browser module composes them (`web::load_vizij`'s defaults) and stepped
//! at 60 Hz. Instruction counts hold where wall time does not, on a machine
//! that is busy with other work.
//!
//! ```bash
//! cargo run --profile profiling -p vizij --example frame_cost -- face.glb [options]
//! ```
//!
//! The `profiling` profile is the release code without its fat LTO, which a
//! build of the whole view takes long and much memory to link.
//!
//! Options:
//! - `--frames N`: frames measured (default 600), after `--warmup N` (120);
//! - `--faces N`: faces side by side (1);
//! - `--graphs rig,pose-driver,…`: the bundle graphs composed (`loadVizij`'s);
//! - `--program auto|none|<id>`: the program run (`auto`);
//! - `--no-animations`: leave the bundle's animations out;
//! - `--speaking`: a `say` run over a provider streaming a viseme every 6
//!   steps, never ending, with the conversation's `speaking` input at 1;
//! - `--sections`: also count the step's parts — the interpreter's tick, its
//!   module calls, its store reads and writes — at the cost of a counter
//!   read around each;
//! - `--drain`: also drain each device's change feed to JSON, as the page's
//!   `drainChanges` does;
//! - `--view`: also run the view, on one thread with no pipelined rendering
//!   as in the browser, drawing offscreen, and count each part of its frame;
//! - `--png <path>`: with `--view`, save the frame the view draws last;
//! - `--dump-state <path>`: write each face's store and rig pose after the
//!   run, path-sorted, its values as JSON with record fields in name order —
//!   what two builds compare to show they drive a face alike.

use std::time::Duration;

use vizij::face::{self, FaceConfig, LoadedFace, ProgramSelect};
use vizij_arora_hal::RigHal;
use vizij_arora_store::BlackboardStore;

mod counter {
    //! Instructions retired by the calling thread: what a section run on
    //! this thread costs, and nothing another thread (a GPU driver's, say)
    //! does meanwhile.

    /// The calling thread's instruction counter, `None` where this example
    /// reads none.
    pub fn instructions() -> Option<u64> {
        imp::instructions()
    }

    #[cfg(target_os = "macos")]
    mod imp {
        extern "C" {
            /// The kernel's per-thread counters; kind 1 is
            /// `[instructions, cycles]`.
            fn thread_selfcounts(kind: i32, buffer: *mut u64, size: usize) -> i32;
        }

        pub fn instructions() -> Option<u64> {
            let mut counts = [0u64; 2];
            // SAFETY: `counts` is the 16 bytes the call is told it may write.
            let status = unsafe { thread_selfcounts(1, counts.as_mut_ptr(), 16) };
            (status == 0).then_some(counts[0])
        }
    }

    #[cfg(target_os = "linux")]
    mod imp {
        use std::cell::Cell;

        /// `perf_event_attr` up to its first published size.
        #[repr(C)]
        struct PerfEventAttr {
            kind: u32,
            size: u32,
            config: u64,
            sample_period: u64,
            sample_type: u64,
            read_format: u64,
            flags: u64,
            wakeup_events: u32,
            bp_type: u32,
            config1: u64,
        }
        const PERF_TYPE_HARDWARE: u32 = 0;
        const PERF_COUNT_HW_INSTRUCTIONS: u64 = 1;
        const EXCLUDE_KERNEL: u64 = 1 << 5;
        const EXCLUDE_HV: u64 = 1 << 6;

        thread_local! {
            /// This thread's counter, opened on first use.
            static COUNTER: Cell<Option<i32>> = const { Cell::new(None) };
        }

        fn open() -> Option<i32> {
            let attr = PerfEventAttr {
                kind: PERF_TYPE_HARDWARE,
                size: std::mem::size_of::<PerfEventAttr>() as u32,
                config: PERF_COUNT_HW_INSTRUCTIONS,
                sample_period: 0,
                sample_type: 0,
                read_format: 0,
                flags: EXCLUDE_KERNEL | EXCLUDE_HV,
                wakeup_events: 0,
                bp_type: 0,
                config1: 0,
            };
            // SAFETY: `attr` outlives the call; pid 0 and cpu -1 count this
            // thread on any CPU.
            let fd = unsafe { libc::syscall(libc::SYS_perf_event_open, &attr, 0, -1, -1, 0) };
            (fd >= 0).then_some(fd as i32)
        }

        pub fn instructions() -> Option<u64> {
            let fd = COUNTER.with(|counter| {
                if counter.get().is_none() {
                    counter.set(open());
                }
                counter.get()
            })?;
            let mut count = 0u64;
            // SAFETY: reads the 8 bytes of `count`.
            let read = unsafe { libc::read(fd, (&mut count as *mut u64).cast(), 8) };
            (read == 8).then_some(count)
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    mod imp {
        pub fn instructions() -> Option<u64> {
            None
        }
    }
}

/// The calling thread's instructions retired so far.
fn instructions() -> u64 {
    counter::instructions().expect("an instruction counter: macOS, or Linux with perf events")
}

struct Args {
    glb: String,
    frames: usize,
    warmup: usize,
    faces: usize,
    graphs: Option<Vec<String>>,
    program: ProgramSelect,
    animations: bool,
    view: bool,
    drain: bool,
    speaking: bool,
    sections: bool,
    png: Option<String>,
    dump_state: Option<String>,
}

fn args() -> Args {
    let mut args = Args {
        glb: String::new(),
        frames: 600,
        warmup: 120,
        faces: 1,
        graphs: None,
        program: ProgramSelect::Auto,
        animations: true,
        view: false,
        drain: false,
        speaking: false,
        sections: false,
        png: None,
        dump_state: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || it.next().expect("option value");
        match arg.as_str() {
            "--frames" => args.frames = value().parse().unwrap(),
            "--warmup" => args.warmup = value().parse().unwrap(),
            "--faces" => args.faces = value().parse().unwrap(),
            "--graphs" => args.graphs = Some(value().split(',').map(str::to_string).collect()),
            "--program" => {
                args.program = match value().as_str() {
                    "auto" => ProgramSelect::Auto,
                    "none" => ProgramSelect::None,
                    id => ProgramSelect::Id(id.to_string()),
                }
            }
            "--no-animations" => args.animations = false,
            "--view" => args.view = true,
            "--drain" => args.drain = true,
            "--speaking" => args.speaking = true,
            "--sections" => args.sections = true,
            "--png" => args.png = Some(value()),
            "--dump-state" => args.dump_state = Some(value()),
            glb => args.glb = glb.to_string(),
        }
    }
    args
}

/// One device, built as `web::load_vizij` builds it.
struct Device {
    arora: arora::Arora,
    rig: RigHal,
    changes: arora_types::data::Subscription,
    meta: vizij::view::meta::FaceMeta,
}

fn device(glb: &[u8], args: &Args) -> Device {
    let config = FaceConfig {
        wanted: args.graphs.clone().unwrap_or_else(|| {
            ["rig", "pose-driver", "pose", "standard-adaptation"]
                .map(String::from)
                .to_vec()
        }),
        program: args.program.clone(),
        stage_neutral: true,
        ros4hri: false,
        #[cfg(feature = "studio")]
        studio: false,
        speech: None,
    };
    let LoadedFace { meta, spec, .. } = face::load_face(glb, &config).expect("load face");
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
    let speech = args.speaking.then(|| {
        arora::HostModule::from_exports(
            SPEAKER,
            vizij_arora_tts::say::exports(Speaker { ticks: 0 }),
        )
    });
    let builder = if args.sections {
        counted::builder(&spec, rig.clone(), store.clone(), &meta.bundle, speech)
    } else {
        face::builder_for(
            &spec,
            rig.clone(),
            store.clone(),
            &meta.bundle.skills,
            speech,
        )
        .expect("builder")
    };
    let mut arora = builder.build().expect("build");
    if args.animations {
        face::load_animations(&mut arora, &meta.bundle).expect("animations");
    }
    let program = face::spawn_program(&mut arora, &meta.bundle, &config.program).expect("program");
    let running = program.and(meta.bundle.program(&config.program));
    eprintln!(
        "device: {} nodes composed, program {}",
        serde_json::from_str::<serde_json::Value>(&spec).unwrap()["nodes"]
            .as_array()
            .map_or(0, Vec::len),
        running.map_or("none", |(id, _)| id.as_str()),
    );
    if args.speaking {
        speak(&mut arora, &meta.bundle.rig_prefix());
    }
    let changes = arora.store().subscribe();
    Device {
        arora,
        rig,
        changes,
        meta,
    }
}

/// `value` as JSON with its record fields in name order: one text per value,
/// whatever order a map iterates in.
fn canonical<T: serde::Serialize>(value: &T) -> String {
    fn sorted(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => {
                let mut entries: Vec<_> = map.into_iter().collect();
                entries.sort_by(|l, r| l.0.cmp(&r.0));
                serde_json::Value::Object(
                    entries.into_iter().map(|(k, v)| (k, sorted(v))).collect(),
                )
            }
            serde_json::Value::Array(items) => {
                serde_json::Value::Array(items.into_iter().map(sorted).collect())
            }
            other => other,
        }
    }
    sorted(serde_json::to_value(value).expect("a value serializes")).to_string()
}

/// The speech provider of `--speaking`: an utterance that never ends,
/// its viseme moving every 6 ticks, as a provider streams the shape at
/// the playhead.
struct Speaker {
    ticks: usize,
}

const SPEAKER: uuid::Uuid = uuid::uuid!("5f0c1d9e-3c43-4a59-9d55-1f6c4f3f1e0b");

impl vizij_arora_tts::Say for Speaker {
    fn say(
        &mut self,
        text: String,
        _voice: Option<String>,
        viseme: &mut String,
        speech: &mut String,
    ) -> arora_behavior::Status {
        const SHAPES: [&str; 8] = ["aa", "PP", "E", "SS", "oh", "nn", "ih", "sil"];
        self.ticks += 1;
        *viseme = SHAPES[(self.ticks / 6) % SHAPES.len()].to_string();
        *speech = text;
        arora_behavior::Status::Running
    }
}

/// Start a `say` run and mark the conversation as speaking.
fn speak(arora: &mut arora::Arora, rig_prefix: &str) {
    use arora_behavior::{interpreter_module, RunPolicy};
    use arora_types::call::Call;
    use arora_types::value::{StructureField, Value};
    use vizij_arora_tts::say::ids::say;
    let call = Call {
        module_id: Some(SPEAKER),
        id: say::FUNCTION,
        args: vec![StructureField {
            id: say::TEXT,
            value: Box::new(Value::String("Hello from the frame harness".into())),
        }],
    };
    arora
        .call(interpreter_module::encode_spawn(
            &call,
            RunPolicy::Concurrent,
        ))
        .expect("spawn say");
    arora
        .store()
        .write(arora_types::data::StateChange::set(
            format!("{rig_prefix}standard/vizij/conversation/speaking"),
            vizij_api_core::value::float(1.0),
        ))
        .expect("speaking");
}

/// The page's `drainChanges`: the feed folded into the latest value per key,
/// as `arora-web` folds it on every step, then that net change as a JSON
/// object of path to value.
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
    out.len()
}

mod counted {
    //! `face::builder_for`'s device, its interpreter, its store and the
    //! interpreter's module calls counted section by section: the same
    //! composition, kept in step with `builder_for`, around wrappers that
    //! count — but for the `studio` feature's keeping of Studio's target
    //! forms, which a face composed without Studio's keys does not use.
    use std::cell::Cell;
    use std::collections::HashMap;
    use std::rc::Rc;

    use arora_behavior::{BehaviorContext, BehaviorError, BehaviorInterpreter, BehaviorStatus};
    use arora_types::call::{Call, CallBridge, CallError, CallResult, Callable, CallableId};
    use arora_types::data::{
        DataError, DataStore, Key, KeyMeta, Slot, State, StateChange, Subscription,
    };
    use arora_types::value::Value;
    use vizij_arora_behavior::{gaze, parse_spec, speech, viseme, ProcessingGraph};
    use vizij_arora_hal::RigHal;
    use vizij_arora_host::Bundle;
    use vizij_arora_store::BlackboardStore;

    use super::instructions;

    thread_local! {
        pub static TICK: Cell<u64> = const { Cell::new(0) };
        pub static CALLS: Cell<u64> = const { Cell::new(0) };
        pub static READS: Cell<u64> = const { Cell::new(0) };
        pub static WRITES: Cell<u64> = const { Cell::new(0) };
        /// Store accesses outside the tick: the clock, the bridges.
        pub static OUTSIDE: Cell<u64> = const { Cell::new(0) };
        static IN_TICK: Cell<bool> = const { Cell::new(false) };
    }

    /// A store access, counted as the tick's when the tick makes it.
    fn access<R>(cell: &'static std::thread::LocalKey<Cell<u64>>, f: impl FnOnce() -> R) -> R {
        if IN_TICK.with(Cell::get) {
            count(cell, f)
        } else {
            count(&OUTSIDE, f)
        }
    }

    /// Instructions retired by `f`, added to `cell`.
    fn count<R>(cell: &'static std::thread::LocalKey<Cell<u64>>, f: impl FnOnce() -> R) -> R {
        let before = instructions();
        let r = f();
        let spent = instructions() - before;
        cell.with(|c| c.set(c.get() + spent));
        r
    }

    pub fn take(cell: &'static std::thread::LocalKey<Cell<u64>>) -> u64 {
        cell.with(|c| c.replace(0))
    }

    struct Graph(ProcessingGraph);

    struct Bridge<'a>(&'a mut dyn CallBridge);

    impl CallBridge for Bridge<'_> {
        fn arora_call(&mut self, call: Call) -> Result<CallResult, CallError> {
            count(&CALLS, || self.0.arora_call(call))
        }
        fn arora_register_callable(&mut self, callable: Rc<dyn Callable>) -> CallableId {
            self.0.arora_register_callable(callable)
        }
        fn arora_unregister_callable(&mut self, id: &CallableId) {
            self.0.arora_unregister_callable(id)
        }
        fn arora_call_indirect(&mut self, id: &CallableId) -> Result<Value, CallError> {
            self.0.arora_call_indirect(id)
        }
    }

    impl BehaviorInterpreter for Graph {
        fn tick(&mut self, ctx: &mut BehaviorContext) -> Result<BehaviorStatus, BehaviorError> {
            let store = ctx.store;
            let functions = ctx.functions;
            let mut bridge = Bridge(&mut *ctx.call_bridge);
            let mut inner = BehaviorContext {
                store,
                call_bridge: &mut bridge,
                functions,
            };
            IN_TICK.with(|t| t.set(true));
            let status = count(&TICK, || self.0.tick(&mut inner));
            IN_TICK.with(|t| t.set(false));
            status
        }
        fn apply(&mut self, diff: arora_behavior::graph::GraphDiff) -> Result<(), BehaviorError> {
            self.0.apply(diff)
        }
        fn load(&mut self, graph: arora_behavior::graph::Graph) -> Result<(), BehaviorError> {
            self.0.load(graph)
        }
        fn spawn(
            &mut self,
            call: Call,
            policy: arora_behavior::RunPolicy,
        ) -> Result<arora_behavior::TaskHandle, BehaviorError> {
            self.0.spawn(call, policy)
        }
        fn halt(&mut self, task: arora_behavior::TaskId) -> Result<(), BehaviorError> {
            self.0.halt(task)
        }
        fn described_methods(
            &self,
        ) -> HashMap<uuid::Uuid, arora_types::record::module::frozen::Export> {
            self.0.described_methods()
        }
    }

    #[derive(Clone)]
    struct Store(BlackboardStore);

    impl DataStore for Store {
        fn read(&self, keys: &[Key]) -> Vec<Option<Value>> {
            access(&READS, || self.0.read(keys))
        }
        fn write(&self, changes: StateChange) -> Result<(), DataError> {
            access(&WRITES, || self.0.write(changes))
        }
        fn snapshot(&self) -> State {
            self.0.snapshot()
        }
        fn slot(&self, key: &Key) -> Box<dyn Slot> {
            self.0.slot(key)
        }
        fn subscribe(&self) -> Subscription {
            self.0.subscribe()
        }
        fn subscribe_prefix(&self, prefix: &str) -> Subscription {
            self.0.subscribe_prefix(prefix)
        }
        fn meta(&self, keys: &[Key]) -> Vec<Option<KeyMeta>> {
            self.0.meta(keys)
        }
        fn all_meta(&self) -> HashMap<Key, KeyMeta> {
            self.0.all_meta()
        }
        fn set_meta(&self, meta: HashMap<Key, KeyMeta>) -> Result<(), DataError> {
            self.0.set_meta(meta)
        }
        fn set_prefix_meta(&self, meta: HashMap<String, KeyMeta>) -> Result<(), DataError> {
            self.0.set_prefix_meta(meta)
        }
        fn clone_box(&self) -> Box<dyn DataStore> {
            Box::new(self.clone())
        }
    }

    pub fn builder(
        spec: &str,
        rig: RigHal,
        store: BlackboardStore,
        bundle: &Bundle,
        speech: Option<arora::HostModule>,
    ) -> arora::AroraBuilder {
        let rig_prefix = bundle.rig_prefix();
        let store: Box<dyn DataStore> = Box::new(Store(store));
        let mut graph = ProcessingGraph::from_spec(parse_spec(spec).unwrap()).unwrap();
        let mut function_modules = vizij::modules::animation::function_modules();
        if let Some(provider) = &speech {
            function_modules.insert(speech::say::ids::say::FUNCTION, provider.id());
        }
        graph.set_function_modules(function_modules);
        let skills = &bundle.skills;
        graph.set_task_fragment(
            gaze::look_at::ids::look_at::FUNCTION,
            gaze::look_at_fragment_from(skills),
        );
        graph.set_task_fragment(
            viseme::play_viseme::ids::play_viseme::FUNCTION,
            viseme::play_viseme_fragment_from(skills, &rig_prefix),
        );
        graph.set_task_fragment(
            speech::say::ids::say::FUNCTION,
            speech::say_fragment_from(skills, &rig_prefix),
        );
        let builder = arora::Arora::builder()
            .with_hal(Box::new(rig))
            .with_host_module(vizij::modules::rest::host_module(store.clone_box()))
            .with_data_store(store)
            .with_behavior_interpreter(Box::new(Graph(graph)))
            .with_host_module(vizij::modules::animation::host_module());
        match speech {
            Some(provider) => builder.with_host_module(provider),
            None => builder,
        }
    }
}

const DT: Duration = Duration::from_micros(16_667);

fn main() {
    let args = args();
    let glb = std::fs::read(&args.glb).expect("read the GLB");
    let mut devices: Vec<Device> = (0..args.faces).map(|_| device(&glb, &args)).collect();

    let mut app = args.view.then(|| view_app(&glb, &devices));

    let mut view_counts = ViewCounts::default();
    let mut tick = 0u64;
    let mut calls = 0u64;
    let mut reads = 0u64;
    let mut writes = 0u64;
    let mut outside = 0u64;
    let mut step = 0u64;
    let mut drained = 0u64;
    let mut viewed = 0u64;
    let mut keys = 0usize;
    for frame in 0..args.warmup + args.frames {
        let measured = frame >= args.warmup;
        let before = instructions();
        for device in &mut devices {
            device.arora.step(DT).expect("step");
        }
        let after_step = instructions();
        if args.drain {
            for device in &devices {
                keys = drain(&device.changes);
            }
        } else {
            for device in &devices {
                while device.changes.try_recv().is_some() {}
            }
        }
        let after_drain = instructions();
        if let Some(app) = app.as_mut() {
            if measured {
                update_counted(app, &mut view_counts);
            } else {
                update_counted(app, &mut ViewCounts::default());
            }
        }
        let after_view = instructions();
        let (t, c, r, w) = (
            counted::take(&counted::TICK),
            counted::take(&counted::CALLS),
            counted::take(&counted::READS),
            counted::take(&counted::WRITES),
        );
        let o = counted::take(&counted::OUTSIDE);
        if measured {
            tick += t;
            calls += c;
            reads += r;
            writes += w;
            outside += o;
            step += after_step - before;
            drained += after_drain - after_step;
            viewed += after_view - after_drain;
        }
    }
    if let (Some(app), Some(path)) = (app.as_mut(), &args.png) {
        save_frame(app, path);
    }
    if let Some(path) = &args.dump_state {
        let mut out = String::new();
        for device in &devices {
            let state: std::collections::BTreeMap<String, String> = device
                .arora
                .store()
                .snapshot()
                .storage
                .into_iter()
                .map(|(key, value)| (key.path, canonical(&value)))
                .collect();
            for (path, value) in state {
                out += &format!("store {path} = {value}\n");
            }
            let mut pose: Vec<String> = device
                .rig
                .pose()
                .into_iter()
                .map(|(path, value)| format!("pose {path} = {}\n", canonical(&value)))
                .collect();
            pose.sort();
            out += &pose.concat();
        }
        std::fs::write(path, out).expect("dump the state");
    }
    let n = args.frames as u64;
    println!("frames {} faces {}", args.frames, args.faces);
    println!("device step   {:>12} instructions/frame", step / n);
    if args.sections {
        println!("  interpreter tick       {:>12}", tick / n);
        println!("    module calls         {:>12}", calls / n);
        println!("    store reads          {:>12}", reads / n);
        println!("    store writes         {:>12}", writes / n);
        println!(
            "    graph evaluation     {:>12}",
            (tick - calls - reads - writes) / n
        );
        println!("  step outside the tick  {:>12}", (step - tick) / n);
        println!("    of which store       {:>12}", outside / n);
    }
    if args.drain {
        println!(
            "drain to JSON {:>12} instructions/frame ({keys} keys)",
            drained / n
        );
    }
    if args.view {
        println!("view update   {:>12} instructions/frame", viewed / n);
        for (name, spent) in &view_counts.schedules {
            println!("  {name:<36} {:>12}", spent / n);
        }
    }
}

/// The view as the page runs it: one App on one thread, no pipelined
/// rendering (the browser has none), drawing offscreen.
fn view_app(glb: &[u8], devices: &[Device]) -> bevy::prelude::App {
    use bevy::app::ScheduleRunnerPlugin;
    use bevy::asset::{AssetMetaCheck, AssetPlugin};
    use bevy::prelude::*;
    use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
    use bevy::render::render_resource::TextureUsages;
    use bevy::render::RenderPlugin;
    use bevy::window::ExitCondition;
    use bevy::winit::WinitPlugin;
    use vizij::view::snapshot::{ensure_ready, FORMAT};
    use vizij::view::{
        self, FaceAssets, OffscreenTarget, ViewEvent, ViewEvents, ViewOptions, ViewPlugin,
    };

    let (tx, rx) = std::sync::mpsc::channel();
    let mut app = App::new();
    FaceAssets::register(&mut app);
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(RenderPlugin {
                synchronous_pipeline_compilation: true,
                ..default()
            })
            .set(AssetPlugin {
                meta_check: AssetMetaCheck::Never,
                ..default()
            })
            .disable::<WinitPlugin>()
            .disable::<PipelinedRenderingPlugin>()
            .disable::<bevy::log::LogPlugin>(),
    )
    .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
        1.0 / 60.0,
    )));
    let mut target = Image::new_target_texture(WIDTH, HEIGHT, FORMAT, None);
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let handle = app.world_mut().resource_mut::<Assets<Image>>().add(target);
    app.insert_resource(OffscreenTarget(handle))
        .insert_resource(ViewEvents(std::sync::Mutex::new(rx)))
        .insert_resource(ViewOptions {
            background: Color::BLACK,
            fit: view::Fit::Contain,
            zoom: Vec2::ONE,
            ambient: std::f32::consts::FRAC_PI_2,
            unlit: false,
        })
        .add_plugins(ViewPlugin);
    for (i, device) in devices.iter().enumerate() {
        tx.send(ViewEvent::LoadFace {
            face_id: format!("face{i}"),
            meta: Box::new(device.meta.clone()),
            glb: glb.to_vec(),
            rig: device.rig.clone(),
        })
        .unwrap();
        if devices.len() > 1 {
            let width = WIDTH as i32 / devices.len() as i32;
            tx.send(ViewEvent::PlaceFace {
                face_id: format!("face{i}"),
                rect: Some(IRect::new(
                    i as i32 * width,
                    0,
                    (i as i32 + 1) * width,
                    HEIGHT as i32,
                )),
            })
            .unwrap();
        }
    }
    std::mem::forget(tx);
    ensure_ready(&mut app);
    for _ in 0..600 {
        app.update();
        if view::all_faces_ready(&mut app) {
            break;
        }
    }
    assert!(view::all_faces_ready(&mut app), "faces never got ready");
    // Every system on this thread, as in the browser, so this thread's count
    // is the whole frame's.
    let single = |world: &mut World| {
        for (_, schedule) in world.resource_mut::<Schedules>().iter_mut() {
            schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::new());
        }
    };
    single(app.world_mut());
    single(app.sub_app_mut(bevy::render::RenderApp).world_mut());
    app
}

/// Read the offscreen target back and save it as a PNG: the frame the view
/// draws next, with the devices left where the run left them.
fn save_frame(app: &mut bevy::prelude::App, path: &str) {
    use bevy::prelude::*;
    use bevy::render::gpu_readback::{Readback, ReadbackComplete};
    use bevy::render::renderer::RenderDevice;
    use vizij::view::OffscreenTarget;

    #[derive(Resource, Default)]
    struct Frame(Option<Vec<u8>>);
    app.init_resource::<Frame>();
    let target = app.world().resource::<OffscreenTarget>().0.clone();
    app.world_mut().spawn(Readback::texture(target)).observe(
        |event: On<ReadbackComplete>, mut frame: ResMut<Frame>| {
            frame.0 = Some(event.data.clone());
        },
    );
    for _ in 0..120 {
        app.update();
        if let Some(data) = app.world_mut().resource_mut::<Frame>().0.take() {
            let row = WIDTH as usize * 4;
            let padded = RenderDevice::align_copy_bytes_per_row(row);
            let pixels: Vec<u8> = data
                .chunks_exact(padded)
                .flat_map(|line| &line[..row])
                .copied()
                .collect();
            image::RgbaImage::from_raw(WIDTH, HEIGHT, pixels)
                .expect("a frame")
                .save(path)
                .expect("save the frame");
            return;
        }
    }
    panic!("no frame read back");
}

const WIDTH: u32 = 763;
const HEIGHT: u32 = 760;

/// Instructions per part of one view frame, in the order `App::update`
/// runs them: each main schedule, then each sub-app's extract and update.
#[derive(Default)]
struct ViewCounts {
    schedules: Vec<(String, u64)>,
}

impl ViewCounts {
    fn add(&mut self, at: usize, name: impl FnOnce() -> String, spent: u64) {
        if self.schedules.len() <= at {
            self.schedules.push((name(), 0));
        }
        self.schedules[at].1 += spent;
    }
}

/// `App::update`, section by section.
fn update_counted(app: &mut bevy::prelude::App, counts: &mut ViewCounts) {
    use bevy::app::MainScheduleOrder;
    let labels = app.world().resource::<MainScheduleOrder>().labels.clone();
    let mut at = 0;
    for label in labels {
        let before = instructions();
        let _ = app.world_mut().try_run_schedule(label);
        counts.add(at, || format!("{label:?}"), instructions() - before);
        at += 1;
    }
    {
        let label = bevy::render::RenderApp;
        let mut sub = app.remove_sub_app(label).expect("render sub-app");
        let before = instructions();
        sub.extract(app.world_mut());
        let after_extract = instructions();
        sub.update();
        let after = instructions();
        counts.add(at, || "render extract".into(), after_extract - before);
        counts.add(
            at + 1,
            || "render update (prepare, queue, draw)".into(),
            after - after_extract,
        );
        at += 2;
        app.insert_sub_app(label, sub);
    }
    let before = instructions();
    app.world_mut().clear_trackers();
    counts.add(at, || "clear trackers".into(), instructions() - before);
}
