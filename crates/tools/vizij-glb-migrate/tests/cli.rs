//! End-to-end tests driving the `vizij-glb-migrate` binary over a synthetic
//! face-bundle GLB built in code.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value as Json};
use vizij_glb_migrate::glb::{Chunk, Glb, CHUNK_BIN};

/// Pre-padded (4-byte aligned) binary payload standing in for the mesh
/// buffer. Must survive migration byte-for-byte.
const BIN: &[u8] = &[0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x00, 0x00, 0x00];

fn sample_gltf() -> Json {
    json!({
        "asset": { "version": "2.0" },
        "scenes": [{
            "nodes": [0],
            "extensions": { "VIZIJ_bundle": {
                "graphs": [{
                    "id": "g1",
                    "spec": {
                        "nodes": [{
                            "id": "const",
                            "type": "constant",
                            "params": { "value": { "float": 1.0 } }
                        }],
                        "edges": []
                    }
                }],
                "poses": { "config": { "neutral": 0.25 } }
            }}
        }],
        "nodes": [{
            "name": "face",
            "extensions": { "RobotData": {
                "id": "robot",
                "features": {
                    "gaze": {
                        "value": {
                            "id": "g", "type": "vector3",
                            "default": { "x": 1.0, "y": 2.0, "z": 3.0 },
                            "constraints": {}
                        }
                    },
                    "tint": {
                        "value": {
                            "id": "t", "type": "rgb",
                            "default": { "r": 0.25, "g": 0.5, "b": 0.75 },
                            "constraints": {}
                        }
                    }
                }
            }}
        }]
    })
}

fn sample_glb_bytes() -> Vec<u8> {
    Glb {
        version: 2,
        json: serde_json::to_vec(&sample_gltf()).expect("serialize sample"),
        tail: vec![Chunk {
            kind: CHUNK_BIN,
            data: BIN.to_vec(),
        }],
    }
    .to_bytes()
}

fn test_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test dir");
    dir
}

fn run(args: &[&Path]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vizij-glb-migrate"))
        .args(args)
        .output()
        .expect("run vizij-glb-migrate")
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn assert_migrated(bytes: &[u8]) {
    // Container invariants: total length, alignment, JSON space padding.
    assert_eq!(read_u32(bytes, 8) as usize, bytes.len());
    assert_eq!(bytes.len() % 4, 0);
    let json_len = read_u32(bytes, 12) as usize;
    assert_eq!(json_len % 4, 0);

    let glb = Glb::parse(bytes).expect("output parses as GLB");
    // BIN chunk byte-identical.
    assert_eq!(glb.tail.len(), 1);
    assert_eq!(glb.tail[0].kind, CHUNK_BIN);
    assert_eq!(glb.tail[0].data, BIN);

    // Values rewritten to canonical arora serde; untouched parts intact.
    let root: Json = serde_json::from_slice(&glb.json).expect("JSON chunk parses");
    assert_eq!(
        root.pointer("/scenes/0/extensions/VIZIJ_bundle/graphs/0/spec/nodes/0/params/value"),
        Some(&json!({ "f32": 1.0 }))
    );
    let features = "/nodes/0/extensions/RobotData/features";
    let gaze: vizij_api_core::Value = serde_json::from_value(
        root.pointer(&format!("{features}/gaze/value/default"))
            .expect("gaze default")
            .clone(),
    )
    .expect("gaze default is a canonical Value");
    assert_eq!(vizij_api_core::value::as_vec3(&gaze), Some([1.0, 2.0, 3.0]));
    let tint: vizij_api_core::Value = serde_json::from_value(
        root.pointer(&format!("{features}/tint/value/default"))
            .expect("tint default")
            .clone(),
    )
    .expect("tint default is a canonical Value");
    assert_eq!(
        vizij_api_core::value::as_color_rgba(&tint),
        Some([0.25, 0.5, 0.75, 1.0])
    );
    assert_eq!(
        root.pointer("/scenes/0/extensions/VIZIJ_bundle/poses/config/neutral"),
        Some(&json!(0.25))
    );
}

#[test]
fn in_place_migration_backs_up_and_is_idempotent() {
    let dir = test_dir("in-place");
    let input = dir.join("face.glb");
    let original = sample_glb_bytes();
    std::fs::write(&input, &original).unwrap();

    let output = run(&[&input]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let backup = dir.join("face.glb.bak");
    assert_eq!(std::fs::read(&backup).unwrap(), original);
    let migrated = std::fs::read(&input).unwrap();
    assert_ne!(migrated, original);
    assert_migrated(&migrated);

    // Second run: nothing left to do, file untouched.
    let output = run(&[&input]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("unchanged"));
    assert_eq!(std::fs::read(&input).unwrap(), migrated);
}

#[test]
fn check_exits_one_until_migrated() {
    let dir = test_dir("check");
    let input = dir.join("face.glb");
    let original = sample_glb_bytes();
    std::fs::write(&input, &original).unwrap();

    let check = Path::new("--check");
    let output = run(&[check, &input]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("would update"));
    // --check writes nothing.
    assert_eq!(std::fs::read(&input).unwrap(), original);
    assert!(!dir.join("face.glb.bak").exists());

    let output = run(&[&input]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");

    let output = run(&[check, &input]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
}

#[test]
fn dry_run_reports_and_writes_nothing() {
    let dir = test_dir("dry-run");
    let input = dir.join("face.glb");
    let original = sample_glb_bytes();
    std::fs::write(&input, &original).unwrap();

    let output = run(&[Path::new("--dry-run"), &input]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("would update"));
    assert_eq!(std::fs::read(&input).unwrap(), original);
    assert!(!dir.join("face.glb.bak").exists());
}

#[test]
fn output_flag_leaves_input_untouched() {
    let dir = test_dir("output");
    let input = dir.join("face.glb");
    let out = dir.join("migrated.glb");
    let original = sample_glb_bytes();
    std::fs::write(&input, &original).unwrap();

    let output = run(&[Path::new("-o"), &out, &input]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(std::fs::read(&input).unwrap(), original);
    assert!(!dir.join("face.glb.bak").exists());
    assert_migrated(&std::fs::read(&out).unwrap());
}

#[test]
fn migrates_multiple_inputs() {
    let dir = test_dir("multi");
    let a = dir.join("a.glb");
    let b = dir.join("b.glb");
    std::fs::write(&a, sample_glb_bytes()).unwrap();
    std::fs::write(&b, sample_glb_bytes()).unwrap();

    let output = run(&[&a, &b]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_migrated(&std::fs::read(&a).unwrap());
    assert_migrated(&std::fs::read(&b).unwrap());
}

#[test]
fn invalid_input_exits_two() {
    let dir = test_dir("invalid");
    let input = dir.join("not-a.glb");
    std::fs::write(&input, b"definitely not a glb").unwrap();

    let output = run(&[&input]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("magic"));
}
