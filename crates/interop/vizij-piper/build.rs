//! Self-provisioning build: everything Piper needs, fetched and prepared here so
//! `cargo build` is the whole setup — no API keys, no env vars, no manual steps.
//!
//! 1. Builds `libpiper` (cmake; it fetches espeak-ng and a prebuilt onnxruntime)
//!    from a pinned piper1-gpl commit.
//! 2. Downloads the default voice from Hugging Face.
//! 3. Patches the voice for phoneme alignments — the upstream Python
//!    `patch_voice_with_alignment` is a one-line graph edit (mark the model's
//!    single `Ceil` tensor as a graph output), reimplemented below as a minimal
//!    ONNX-protobuf surgery so the build needs no Python.
//! 4. Bakes the resulting paths in as compile-time defaults (overridable at run
//!    time via `PIPER_VOICE` / `PIPER_VOICE_CONFIG` / `PIPER_ESPEAK_DATA`).
//!
//! Artifacts land in `~/.cache/vizij-piper` (override: `VIZIJ_PIPER_CACHE`), so
//! they survive `cargo clean` and are shared across checkouts. Network is needed
//! only on the first build.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Pinned piper1-gpl commit (the revision validated in the Piper bootstrap
/// exercise: exact alignment reconciliation on the Python and C APIs).
const PIPER_COMMIT: &str = "4bfd11c5b5998660c52aaa743c4fa05717104e98";

/// The default voice. Medium-quality US English — the bootstrap baseline voice.
const VOICE: &str = "en_US-lessac-medium";
const VOICE_URL_DIR: &str =
    "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/lessac/medium";

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        panic!("vizij-piper: Windows is not supported yet (libpiper build scripting)");
    }

    let cache = cache_dir();
    std::fs::create_dir_all(&cache).expect("create the vizij-piper cache dir");

    let install = build_libpiper(&cache);
    let (voice, voice_config) = provision_voice(&cache);

    // Link + rpath (the dylibs stay in the cache install; rpath finds them).
    println!("cargo:rustc-link-search=native={}", install.display());
    println!(
        "cargo:rustc-link-search=native={}",
        install.join("lib").display()
    );
    println!("cargo:rustc-link-lib=dylib=piper");
    println!("cargo:rustc-link-lib=dylib=onnxruntime");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", install.display());
    println!(
        "cargo:rustc-link-arg=-Wl,-rpath,{}",
        install.join("lib").display()
    );
    // Exported over the DEP_PIPER_* channel (`links = "piper"`): the link-args
    // above cover only THIS crate's artifacts, so a consumer binary must add
    // the rpath itself — from DEP_PIPER_INSTALL_DIR in its own build script.
    println!("cargo:install_dir={}", install.display());

    // Baked-in defaults the library falls back to when the env vars are unset.
    println!(
        "cargo:rustc-env=VIZIJ_PIPER_DEFAULT_ESPEAK_DATA={}",
        install.join("espeak-ng-data").display()
    );
    println!(
        "cargo:rustc-env=VIZIJ_PIPER_DEFAULT_VOICE={}",
        voice.display()
    );
    println!(
        "cargo:rustc-env=VIZIJ_PIPER_DEFAULT_VOICE_CONFIG={}",
        voice_config.display()
    );

    generate_bindings(&install);
}

fn cache_dir() -> PathBuf {
    if let Ok(dir) = env::var("VIZIJ_PIPER_CACHE") {
        return PathBuf::from(dir);
    }
    let home = env::var("HOME").expect("HOME (or set VIZIJ_PIPER_CACHE)");
    PathBuf::from(home).join(".cache/vizij-piper")
}

/// Build libpiper once per (commit, target); reuse the cached install afterward.
fn build_libpiper(cache: &Path) -> PathBuf {
    let target = env::var("TARGET").unwrap();
    let install = cache.join(format!("libpiper-{}-{target}", &PIPER_COMMIT[..12]));
    let lib_name = if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        "libpiper.dylib"
    } else {
        "libpiper.so"
    };
    if install.join(lib_name).exists() && install.join("espeak-ng-data").exists() {
        return install;
    }

    // Short directory names on purpose: espeak-ng's data compiler holds paths
    // in a ~160-byte buffer, and deep build paths get silently truncated
    // (observed as `Bad vowel file` / truncated file names). Keep the whole
    // espeak build path comfortably under that.
    let src = cache.join(format!("src-{}", &PIPER_COMMIT[..12]));
    if !src.join("libpiper/CMakeLists.txt").exists() {
        let url = format!("https://github.com/OHF-Voice/piper1-gpl/archive/{PIPER_COMMIT}.tar.gz");
        run(Command::new("sh").arg("-c").arg(format!(
            "curl -fsSL '{url}' | tar xz -C '{}'",
            cache.display()
        )));
        let unpacked = cache.join(format!("piper1-gpl-{PIPER_COMMIT}"));
        std::fs::rename(&unpacked, &src).expect("rename the unpacked piper source");
        assert!(
            src.join("libpiper/CMakeLists.txt").exists(),
            "piper source tarball did not unpack where expected"
        );
    }
    let espeak_build_path_len = src
        .join("libpiper/build/espeak_ng/src/espeak_ng_external-build")
        .as_os_str()
        .len();
    assert!(
        espeak_build_path_len < 140,
        "the build path is too deep for espeak-ng's fixed path buffers \
         ({espeak_build_path_len} chars) — set VIZIJ_PIPER_CACHE to a shorter directory"
    );

    let build = src.join("libpiper/build");
    // current_dir(cache): libpiper's cmake drops a `download/` dir (the
    // onnxruntime archive) relative to the working directory — keep it in the
    // cache, never in the crate.
    run(Command::new("cmake")
        .current_dir(cache)
        .arg("-B")
        .arg(&build)
        .arg("-S")
        .arg(src.join("libpiper"))
        .arg("-DCMAKE_BUILD_TYPE=Release")
        .arg(format!("-DCMAKE_INSTALL_PREFIX={}", install.display())));
    // espeak-ng's data compiler resolves `../phsource` / `../dictsource`
    // relative to ESPEAK_DATA_PATH (the external project's build dir), i.e. one
    // level ABOVE it — where nothing puts them. Pre-plant symlinks to the
    // (future) espeak checkout; they dangle until the external project clones,
    // then resolve. Observed as `phsource/intonation: No such file or
    // directory` during `Compile intonations` otherwise.
    let espeak_src = build.join("espeak_ng/src");
    std::fs::create_dir_all(&espeak_src).expect("create the espeak src dir");
    for dir in ["phsource", "dictsource"] {
        let link = espeak_src.join(dir);
        let target = espeak_src.join("espeak_ng_external").join(dir);
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&target, &link).expect("plant the espeak data symlink");
    }
    // Sequential on purpose: espeak-ng's data generation is order-sensitive
    // under a parallel make, and cargo exports its jobserver through MAKEFLAGS
    // (which would re-parallelize the child make) — scrub it.
    run(Command::new("cmake")
        .current_dir(cache)
        .arg("--build")
        .arg(&build)
        .env_remove("MAKEFLAGS")
        .env_remove("MFLAGS")
        .env_remove("CARGO_MAKEFLAGS"));
    run(Command::new("cmake")
        .current_dir(cache)
        .arg("--install")
        .arg(&build));
    install
}

/// Download the default voice (model + config) and patch the model for
/// alignments. Idempotent: the patched model is cached.
fn provision_voice(cache: &Path) -> (PathBuf, PathBuf) {
    let voices = cache.join("voices");
    std::fs::create_dir_all(&voices).expect("create the voices dir");
    let model = voices.join(format!("{VOICE}.onnx"));
    let config = voices.join(format!("{VOICE}.onnx.json"));
    let patched = voices.join(format!("{VOICE}.alignment.onnx"));

    if !config.exists() {
        download(&format!("{VOICE_URL_DIR}/{VOICE}.onnx.json"), &config);
    }
    if !patched.exists() {
        if !model.exists() {
            download(&format!("{VOICE_URL_DIR}/{VOICE}.onnx"), &model);
        }
        let bytes = std::fs::read(&model).expect("read the downloaded voice model");
        let out = patch_alignment(&bytes).expect("patch the voice for alignments");
        std::fs::write(&patched, out).expect("write the patched voice model");
    }
    // libpiper resolves `<model>.json` when no config is given; our wrapper
    // passes the config path explicitly, so the `.alignment.onnx` name is fine.
    (patched, config)
}

fn download(url: &str, to: &Path) {
    let tmp = to.with_extension("part");
    run(Command::new("curl")
        .arg("-fsSL")
        .arg("--retry")
        .arg("3")
        .arg("-o")
        .arg(&tmp)
        .arg(url));
    std::fs::rename(&tmp, to).expect("move the downloaded file into place");
}

fn run(cmd: &mut Command) {
    let desc = format!("{cmd:?}");
    let status = cmd.status().unwrap_or_else(|e| panic!("spawn {desc}: {e}"));
    assert!(status.success(), "command failed: {desc}");
}

fn generate_bindings(install: &Path) {
    let bindings = bindgen::Builder::default()
        .header(install.join("include/piper.h").to_str().unwrap())
        // macOS's C SDK ships no uchar.h (char32_t); libc++ does — parse the
        // header as C++. Linux is fine either way.
        .clang_args(["-x", "c++", "-std=c++17"])
        .clang_arg(format!("-I{}", install.join("include").display()))
        .allowlist_function("piper_.*")
        .allowlist_type("piper_.*")
        .allowlist_var("PIPER_.*")
        .generate()
        .expect("bindgen over piper.h");
    bindings
        .write_to_file(PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs"))
        .expect("write bindings.rs");
}

// ONNX alignment patch — a minimal protobuf surgery.
//==============================================================================
// Upstream's `piper.patch_voice_with_alignment` marks the model's single `Ceil`
// tensor as a graph output, exposing per-phoneme-id sample counts. In protobuf
// terms: find GraphProto.node (field 1) with NodeProto.op_type (field 4) ==
// "Ceil", take its NodeProto.output (field 2), and append a
// ValueInfoProto { name (field 1) } to GraphProto.output (field 12) inside
// ModelProto.graph (field 7). Only the two enclosing length prefixes change.

fn patch_alignment(model: &[u8]) -> Result<Vec<u8>, String> {
    // Locate the graph field (7, length-delimited) among ModelProto's fields.
    let mut pos = 0usize;
    let mut graph_span = None;
    while pos < model.len() {
        let (tag, p) = read_varint(model, pos)?;
        let field = (tag >> 3) as u32;
        let wire = (tag & 7) as u8;
        pos = p;
        match wire {
            0 => {
                let (_, p) = read_varint(model, pos)?;
                pos = p;
            }
            2 => {
                let (len, p) = read_varint(model, pos)?;
                let start = p;
                let end = start + len as usize;
                if field == 7 {
                    graph_span = Some((start, end));
                }
                pos = end;
            }
            5 => pos += 4,
            1 => pos += 8,
            w => return Err(format!("unexpected wire type {w} in ModelProto")),
        }
    }
    let (gstart, gend) = graph_span.ok_or("no graph field in ModelProto")?;
    let graph = &model[gstart..gend];

    // Scan the graph: collect the Ceil node's output name and the existing
    // graph outputs (for idempotency).
    let mut ceil_outputs: Vec<String> = Vec::new();
    let mut existing_outputs: Vec<String> = Vec::new();
    let mut pos = 0usize;
    while pos < graph.len() {
        let (tag, p) = read_varint(graph, pos)?;
        let field = (tag >> 3) as u32;
        let wire = (tag & 7) as u8;
        pos = p;
        match wire {
            0 => {
                let (_, p) = read_varint(graph, pos)?;
                pos = p;
            }
            2 => {
                let (len, p) = read_varint(graph, pos)?;
                let start = p;
                let end = start + len as usize;
                match field {
                    1 => {
                        // NodeProto: op_type (4, string), output (2, string).
                        let node = &graph[start..end];
                        let (op, outs) = scan_node(node)?;
                        if op == "Ceil" {
                            ceil_outputs.extend(outs);
                        }
                    }
                    12 => {
                        // ValueInfoProto: name (1, string).
                        if let Some(name) = scan_value_info_name(&graph[start..end])? {
                            existing_outputs.push(name);
                        }
                    }
                    _ => {}
                }
                pos = end;
            }
            5 => pos += 4,
            1 => pos += 8,
            w => return Err(format!("unexpected wire type {w} in GraphProto")),
        }
    }

    let ceil = match ceil_outputs.as_slice() {
        [] => return Err("no Ceil node found — not a Piper VITS voice?".into()),
        [one] => one.clone(),
        many => {
            return Err(format!(
                "multiple Ceil tensors, cannot autodetect: {many:?}"
            ))
        }
    };
    if existing_outputs.iter().any(|n| n == &ceil) {
        return Ok(model.to_vec()); // already patched
    }

    // Encode ValueInfoProto { name = ceil } and wrap it as graph field 12.
    let mut vi = Vec::new();
    vi.push(1 << 3 | 2);
    write_varint(&mut vi, ceil.len() as u64);
    vi.extend_from_slice(ceil.as_bytes());
    let mut appended = Vec::new();
    appended.push(12 << 3 | 2);
    write_varint(&mut appended, vi.len() as u64);
    appended.extend_from_slice(&vi);

    // Re-emit the ModelProto stream, copying every field and growing the graph
    // field by the appended output.
    let mut out = Vec::with_capacity(model.len() + appended.len() + 8);
    let mut pos = 0usize;
    while pos < model.len() {
        let field_start = pos;
        let (tag, p) = read_varint(model, pos)?;
        let field = (tag >> 3) as u32;
        let wire = (tag & 7) as u8;
        pos = p;
        match wire {
            2 => {
                let (len, p) = read_varint(model, pos)?;
                let start = p;
                let end = start + len as usize;
                if field == 7 {
                    write_varint(&mut out, tag);
                    write_varint(&mut out, len + appended.len() as u64);
                    out.extend_from_slice(&model[start..end]);
                    out.extend_from_slice(&appended);
                } else {
                    out.extend_from_slice(&model[field_start..end]);
                }
                pos = end;
            }
            0 => {
                let (_, p) = read_varint(model, pos)?;
                out.extend_from_slice(&model[field_start..p]);
                pos = p;
            }
            5 => {
                out.extend_from_slice(&model[field_start..pos + 4]);
                pos += 4;
            }
            1 => {
                out.extend_from_slice(&model[field_start..pos + 8]);
                pos += 8;
            }
            w => return Err(format!("unexpected wire type {w} in ModelProto")),
        }
    }
    Ok(out)
}

/// NodeProto: return (op_type, output names).
fn scan_node(node: &[u8]) -> Result<(String, Vec<String>), String> {
    let mut op = String::new();
    let mut outs = Vec::new();
    let mut pos = 0usize;
    while pos < node.len() {
        let (tag, p) = read_varint(node, pos)?;
        let field = (tag >> 3) as u32;
        let wire = (tag & 7) as u8;
        pos = p;
        match wire {
            0 => {
                let (_, p) = read_varint(node, pos)?;
                pos = p;
            }
            2 => {
                let (len, p) = read_varint(node, pos)?;
                let start = p;
                let end = start + len as usize;
                match field {
                    2 => outs.push(String::from_utf8_lossy(&node[start..end]).into_owned()),
                    4 => op = String::from_utf8_lossy(&node[start..end]).into_owned(),
                    _ => {}
                }
                pos = end;
            }
            5 => pos += 4,
            1 => pos += 8,
            w => return Err(format!("unexpected wire type {w} in NodeProto")),
        }
    }
    Ok((op, outs))
}

/// ValueInfoProto: return the name (field 1) if present.
fn scan_value_info_name(vi: &[u8]) -> Result<Option<String>, String> {
    let mut pos = 0usize;
    while pos < vi.len() {
        let (tag, p) = read_varint(vi, pos)?;
        let field = (tag >> 3) as u32;
        let wire = (tag & 7) as u8;
        pos = p;
        match wire {
            0 => {
                let (_, p) = read_varint(vi, pos)?;
                pos = p;
            }
            2 => {
                let (len, p) = read_varint(vi, pos)?;
                let start = p;
                let end = start + len as usize;
                if field == 1 {
                    return Ok(Some(String::from_utf8_lossy(&vi[start..end]).into_owned()));
                }
                pos = end;
            }
            5 => pos += 4,
            1 => pos += 8,
            w => return Err(format!("unexpected wire type {w} in ValueInfoProto")),
        }
    }
    Ok(None)
}

fn read_varint(buf: &[u8], mut pos: usize) -> Result<(u64, usize), String> {
    let mut value = 0u64;
    let mut shift = 0u32;
    loop {
        let byte = *buf.get(pos).ok_or("varint past end of buffer")?;
        pos += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok((value, pos));
        }
        shift += 7;
        if shift >= 64 {
            return Err("varint too long".into());
        }
    }
}

fn write_varint(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}
