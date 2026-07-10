//! Generates the Arora module glue from `module.yaml` + the type records in
//! `types/`: the `arora_function_<uuid>` exports, the buffer ABI, and the
//! `Struct <-> Value::Structure` conversions the codegen emits (ARORA-55).

use anyhow::Result;
use arora_module_core::analyze_module_from_path;
use arora_module_rust::{generate_records, generate_sources, rustfmt::apply_rustfmt};
use arora_registry::{local::LocalRegistry, local_yaml::load_records_from_yaml_dir};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let mut registry = LocalRegistry::new();

    // Load the declared animation-schema types (AnimationClip/AnimTrack/Keypoint
    // /TrackOutput) so `module.yaml`'s references resolve.
    let types_dir = PathBuf::from("types");
    load_records_from_yaml_dir(types_dir.clone(), &mut registry).await?;
    println!("cargo:rerun-if-changed={}", types_dir.display());

    let module_yaml = "module.yaml";
    let assets = analyze_module_from_path(module_yaml, &mut registry).await?;
    println!("cargo:rerun-if-changed={}", module_yaml);

    let records =
        generate_records(&assets, &registry).map_err(|e| anyhow::anyhow!("records: {e}"))?;
    records
        .sync(PathBuf::from("records"))
        .await
        .map_err(|e| anyhow::anyhow!("sync records: {e}"))?;

    let sources = generate_sources(assets, &mut registry).await?;
    let source_path = PathBuf::from("src/arora_generated/");
    sources
        .sync(source_path.clone())
        .await
        .map_err(|e| anyhow::anyhow!("sync sources to {}: {e}", source_path.display()))?;
    apply_rustfmt(source_path.clone()).await?;
    println!("cargo:rerun-if-changed={}", source_path.display());

    Ok(())
}
