//! End-to-end runs of the `vizij-import` binary over a synthetic face.

mod common;

use std::path::PathBuf;
use std::process::{Command, Output};

use common::{build, mouth};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vizij-import"))
        .args(args)
        .output()
        .expect("binary runs")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vizij-import-cli-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("face.glb")
}

#[test]
fn check_then_rewrite_then_check_again() {
    let path = scratch("rewrite");
    let original = build(&[mouth(10)]).bytes;
    std::fs::write(&path, &original).unwrap();
    let p = path.to_str().unwrap();

    let check = run(&["--morph-triangles", "--check", p]);
    assert_eq!(check.status.code(), Some(1));
    assert_eq!(
        std::fs::read(&path).unwrap(),
        original,
        "--check writes nothing"
    );

    let rewrite = run(&["--morph-triangles", p]);
    assert_eq!(rewrite.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&rewrite.stdout);
    assert!(
        stdout.contains("mouth (primitive 0, 20 vertices): retriangulated"),
        "{stdout}"
    );
    assert!(stdout.contains("smile_shape"), "{stdout}");
    let mut backup = path.clone().into_os_string();
    backup.push(".bak");
    assert_eq!(std::fs::read(backup).unwrap(), original);

    let again = run(&["--morph-triangles", "--check", p]);
    assert_eq!(again.status.code(), Some(0));
}

#[test]
fn output_leaves_the_input_untouched() {
    let path = scratch("output");
    let original = build(&[mouth(10)]).bytes;
    std::fs::write(&path, &original).unwrap();
    let out = path.with_file_name("out.glb");
    let result = run(&[
        "--morph-triangles",
        path.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
    ]);
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_ne!(std::fs::read(&out).unwrap(), original);
}

#[test]
fn a_pass_must_be_named() {
    let path = scratch("no-pass");
    std::fs::write(&path, build(&[mouth(10)]).bytes).unwrap();
    let result = run(&[path.to_str().unwrap()]);
    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stderr).contains("no pass selected"));
}

#[test]
fn output_is_written_even_when_nothing_changes() {
    let path = scratch("unchanged-output");
    let original = build(&[mouth(10)]).bytes;
    std::fs::write(&path, &original).unwrap();
    let p = path.to_str().unwrap();
    assert_eq!(run(&["--morph-triangles", p]).status.code(), Some(0));
    let optimized = std::fs::read(&path).unwrap();

    let out = path.with_file_name("out.glb");
    std::fs::write(&out, b"stale").unwrap();
    let result = run(&["--morph-triangles", p, "-o", out.to_str().unwrap()]);
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(std::fs::read(&out).unwrap(), optimized);
}

#[test]
fn the_mode_is_chosen_on_the_flag() {
    let path = scratch("mode");
    std::fs::write(&path, build(&[mouth(10)]).bytes).unwrap();
    let p = path.to_str().unwrap();
    for flag in ["--morph-triangles=safe", "--morph-triangles=overall"] {
        assert_eq!(
            run(&[flag, "--dry-run", p]).status.code(),
            Some(0),
            "{flag}"
        );
    }
    let unknown = run(&["--morph-triangles=bold", p]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown option"));
}

#[test]
fn a_part_safe_mode_declined_is_listed_even_when_no_pose_still_folds() {
    // Every pose of this square is clean; only blends of its two targets
    // fold, and the rewrite that unfolds them would worsen a pose.
    use common::{flat, square};
    let spec = square("quad")
        .target(
            "a",
            flat(&[[-0.2, -0.14], [0.15, 0.68], [0.13, 1.83], [-0.63, 0.38]]),
        )
        .target(
            "b",
            flat(&[[0.63, 0.58], [0.52, 0.1], [0.96, 1.39], [-0.57, 1.59]]),
        );
    let path = scratch("declined");
    std::fs::write(&path, build(&[spec]).bytes).unwrap();
    let result = run(&[
        "--morph-triangles=safe",
        "--dry-run",
        path.to_str().unwrap(),
    ]);
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        stdout.contains("quad (primitive 0, 4 vertices): kept (overall mode would rewrite it"),
        "{stdout}"
    );
}
