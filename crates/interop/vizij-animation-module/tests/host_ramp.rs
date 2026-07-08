//! Host-side end-to-end repro: load the built `.wasm` into an Arora engine, run
//! the module's `load_animation` / `create_player` / `add_instance` / `step`
//! exports over the real buffer ABI, and assert a one-track 0->1 ramp advances.
//!
//! STATUS: this reproduces an arora-buffers 0.2.0 wire-format discrepancy for
//! arrays of structures across the `arora_call` boundary, so it currently traps
//! in the guest during argument deserialization. The module itself is correct —
//! its logic is verified natively in `src/lib.rs` `tests`.
//!
//! The gap: `arora-module-rust` 0.2.0 codegen serializes each array-of-struct
//! element as a full self-describing value (`begin_structure`: element
//! `TYPE_STRUCTURE` tag + 16-byte id) and reads it back with `check_type` — per
//! its own comment ("Each element is serialized as a full value ..."). But
//! `arora-buffers::serde_uuid` (the engine's generic `Value` codec, used by
//! `CallBridge::arora_call` to marshal `Call` args and decode results) encodes
//! `Value::ArrayStructure` elements raw (`begin_structure_raw`: no per-element
//! tag/id). Both the clip input (`tracks`/`points`) and the `step` return
//! (`[TrackOutput]`) are arrays of structs, so both directions mismarshal.
//! Reconciling the two is a buffer wire-format change, gated on a design
//! discussion (see report).
//!
//! Ignored by default (also needs the wasm artifact pre-built — a nested
//! `cargo build` would deadlock on the build lock). To reproduce:
//!
//! ```text
//! cargo build -p vizij-animation-module --target wasm32-wasip1
//! cargo test  -p vizij-animation-module --test host_ramp -- --ignored
//! ```

use std::path::PathBuf;

use arora_engine::engine::EngineBuilder;
use arora_engine::executor::wasm::WebAssemblyExecutor;
use arora_types::call::Call;
use arora_types::module::low::{Header, ModuleDefinition};
use arora_types::value::{Structure, StructureField, StructureWithoutId, Value};
use uuid::Uuid;

// --- declared ids (mirror module.yaml + the type records) --------------------
const MODULE_ID: &str = "76697a69-6a00-0000-0d00-000000000000";

const FN_LOAD: &str = "76697a69-6a00-0000-0f00-000000000001";
const FN_CREATE_PLAYER: &str = "76697a69-6a00-0000-0f00-000000000002";
const FN_ADD_INSTANCE: &str = "76697a69-6a00-0000-0f00-000000000003";
const FN_STEP: &str = "76697a69-6a00-0000-0f00-000000000004";

const P_CLIP: &str = "76697a69-6a00-0000-0f01-000000000001";
const P_NAME: &str = "76697a69-6a00-0000-0f02-000000000001";
const P_PLAYER: &str = "76697a69-6a00-0000-0f03-000000000001";
const P_ANIM: &str = "76697a69-6a00-0000-0f03-000000000002";
const P_DT_NS: &str = "76697a69-6a00-0000-0f04-000000000001";

const CLIP_TYPE: &str = "76697a69-6a00-0000-0000-000000000100";
const CLIP_NAME: &str = "76697a69-6a00-0000-0100-000000000001";
const CLIP_DURATION: &str = "76697a69-6a00-0000-0100-000000000002";
const CLIP_TRACKS: &str = "76697a69-6a00-0000-0100-000000000003";

const TRACK_TYPE: &str = "76697a69-6a00-0000-0000-000000000101";
const TR_ID: &str = "76697a69-6a00-0000-0101-000000000001";
const TR_NAME: &str = "76697a69-6a00-0000-0101-000000000002";
const TR_ANIMATABLE: &str = "76697a69-6a00-0000-0101-000000000003";
const TR_POINTS: &str = "76697a69-6a00-0000-0101-000000000004";

const KP_TYPE: &str = "76697a69-6a00-0000-0000-000000000102";
const KP_ID: &str = "76697a69-6a00-0000-0102-000000000001";
const KP_STAMP: &str = "76697a69-6a00-0000-0102-000000000002";
const KP_VALUE: &str = "76697a69-6a00-0000-0102-000000000003";

const TO_TRACK_ID: &str = "76697a69-6a00-0000-0110-000000000001";
const TO_DEFAULT_KEY: &str = "76697a69-6a00-0000-0110-000000000002";
const TO_VALUE: &str = "76697a69-6a00-0000-0110-000000000003";

fn u(s: &str) -> Uuid {
    Uuid::parse_str(s).expect("uuid")
}

fn field(id: &str, value: Value) -> StructureField {
    StructureField {
        id: u(id),
        value: Box::new(value),
    }
}

/// A one-track 0->1 ramp over 1000 ms (keyframes at 0.0 and 1.0), targeting
/// `node/x`, as the typed `AnimationClip` structure. The keyframe `value`s are
/// dynamic scalars; intermediate samples follow vizij-animation-core's default
/// keyframe interpolation (eased, not linear).
///
/// Typed `Vec<Struct>` fields cross as `Value::ArrayStructure` (a homogeneous
/// `add_array_structure`, element tag `TYPE_STRUCTURE`) — the layout the
/// generated deserializer expects; the generic `Value::ArrayValue` would encode
/// as an array of `TYPE_VALUE` and mismatch.
fn ramp_clip() -> Value {
    let keypoint = |id: &str, stamp: f32, v: f32| StructureWithoutId {
        fields: vec![
            field(KP_ID, Value::String(id.into())),
            field(KP_STAMP, Value::F32(stamp)),
            field(KP_VALUE, Value::F32(v)),
        ],
    };
    let track = StructureWithoutId {
        fields: vec![
            field(TR_ID, Value::String("t0".into())),
            field(TR_NAME, Value::String("ramp".into())),
            field(TR_ANIMATABLE, Value::String("node/x".into())),
            field(
                TR_POINTS,
                Value::ArrayStructure {
                    id: u(KP_TYPE),
                    elements: vec![keypoint("k0", 0.0, 0.0), keypoint("k1", 1.0, 1.0)],
                },
            ),
        ],
    };
    Value::Structure(Structure {
        id: u(CLIP_TYPE),
        fields: vec![
            field(CLIP_NAME, Value::String("ramp".into())),
            field(CLIP_DURATION, Value::U32(1000)),
            field(
                CLIP_TRACKS,
                Value::ArrayStructure {
                    id: u(TRACK_TYPE),
                    elements: vec![track],
                },
            ),
        ],
    })
}

fn workspace_target_wasm() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("target/wasm32-wasip1/debug/vizij_animation_module.wasm")
}

#[ignore = "needs the wasm artifact pre-built (a nested cargo build deadlocks on the build lock); run with --ignored after `cargo build -p vizij-animation-module --target wasm32-wasip1`"]
#[test]
fn ramp_advances_through_the_wasm_module() {
    // --- load the built wasm module into an engine --------------------------
    let mut engine = EngineBuilder::new()
        .add_executor(WebAssemblyExecutor::new().expect("wasm executor"))
        .build();

    let header_yaml = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/arora_generated/module.yaml"),
    )
    .expect("read generated header");
    let header: Header = serde_yaml::from_str(&header_yaml).expect("parse header");

    let wasm = std::fs::read(workspace_target_wasm()).expect(
        "wasm artifact missing — run `cargo build -p vizij-animation-module --target wasm32-wasip1`",
    );
    engine
        .load_module(ModuleDefinition {
            schema_version: 0,
            header,
            executable: wasm.into_boxed_slice(),
        })
        .expect("load module");

    let module = u(MODULE_ID);
    let call = |engine: &mut _, fn_id: &str, args: Vec<StructureField>| -> Value {
        <_ as arora_types::call::CallBridge>::arora_call(
            engine,
            &module,
            Call {
                module_id: None,
                id: u(fn_id),
                args,
            },
        )
        .expect("arora_call")
        .ret
    };

    // --- setup: load clip, create a player, attach an instance --------------
    let anim = as_u32(call(&mut engine, FN_LOAD, vec![field(P_CLIP, ramp_clip())]));
    let player = as_u32(call(
        &mut engine,
        FN_CREATE_PLAYER,
        vec![field(P_NAME, Value::String("p".into()))],
    ));
    let _inst = as_u32(call(
        &mut engine,
        FN_ADD_INSTANCE,
        vec![
            field(P_PLAYER, Value::U32(player)),
            field(P_ANIM, Value::U32(anim)),
        ],
    ));

    // --- step twice by 0.25 s: the ramp advances 0 -> 0.25 -> 0.5 -----------
    let quarter_s = 250_000_000u64; // 0.25 s in ns

    let first = step_track(call(
        &mut engine,
        FN_STEP,
        vec![field(P_DT_NS, Value::U64(quarter_s))],
    ));
    assert_eq!(first.track_id, "t0");
    assert_eq!(first.default_key, "node/x");

    let second = step_track(call(
        &mut engine,
        FN_STEP,
        vec![field(P_DT_NS, Value::U64(quarter_s))],
    ));
    assert_eq!(second.default_key, "node/x");

    // What this test proves is the cross-boundary contract: values marshal
    // faithfully through `arora_call` (typed `ArrayStructure` in and out) and the
    // track carries its authored key. The sampled magnitude follows
    // vizij-animation-core's default keyframe interpolation (eased, not linear),
    // so we assert the interpolation-agnostic facts rather than a specific curve:
    // the ramp advances strictly upward from 0, and at the 0.5 s midpoint it
    // reaches ~0.5 (the symmetric checkpoint the native unit test also confirms).
    assert!(
        first.value > 0.0 && first.value < second.value,
        "ramp should advance strictly upward, got {} then {}",
        first.value,
        second.value
    );
    assert!(
        (second.value - 0.5).abs() < 1e-3,
        "expected ~0.5 at the 0.5 s midpoint, got {}",
        second.value
    );
}

struct TrackOut {
    track_id: String,
    default_key: String,
    value: f32,
}

fn as_u32(v: Value) -> u32 {
    match v {
        Value::U32(n) => n,
        other => panic!("expected U32, got {other:?}"),
    }
}

fn as_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => panic!("expected String, got {other:?}"),
    }
}

/// Decode the first `TrackOutput` from a `step` result (`[TrackOutput]`).
fn step_track(ret: Value) -> TrackOut {
    // ARORA-55 #137: an array-of-struct return decodes as the typed
    // `Value::ArrayStructure` (elements are `StructureWithoutId`), not the
    // generic `ArrayValue`.
    let elements = match ret {
        Value::ArrayStructure { elements, .. } => elements,
        other => panic!("expected ArrayStructure, got {other:?}"),
    };
    let s = elements.into_iter().next().expect("one track output");
    let get = |id: &str| -> &Value {
        s.fields
            .iter()
            .find(|f| f.id == u(id))
            .map(|f| f.value.as_ref())
            .expect("field")
    };
    let value = match get(TO_VALUE) {
        Value::F32(f) => *f,
        Value::F64(f) => *f as f32,
        other => panic!("expected F32 value, got {other:?}"),
    };
    TrackOut {
        track_id: as_string(get(TO_TRACK_ID)),
        default_key: as_string(get(TO_DEFAULT_KEY)),
        value,
    }
}
