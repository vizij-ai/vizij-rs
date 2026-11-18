//! Optional passes a face's `.glb` goes through when it is imported, before
//! it becomes a face bundle.
//!
//! Each pass rewrites the file's mesh data in place and reports what it
//! did; the JSON chunk is left byte-for-byte, so a pass never changes the
//! document's structure, names or extensions.
//!
//! - [`optimize_morph_triangles`] chooses, for each part modelled as one
//!   large polygon, the triangles that stay unfolded across its morph
//!   targets ([`morph_triangles`] explains why exported triangles fold).
//!
//! ```no_run
//! let bytes = std::fs::read("face.glb")?;
//! let optimized = vizij_import::optimize_morph_triangles(&bytes)?;
//! if let Some(rewritten) = optimized.bytes {
//!     std::fs::write("face.glb", rewritten)?;
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod gltf;
pub mod morph_triangles;

use thiserror::Error;
use vizij_glb_migrate::glb::{Glb, GlbError, CHUNK_BIN};

pub use morph_triangles::Report;

/// Why a file could not go through a pass at all. Problems confined to one
/// mesh are reported in that pass's report instead, and the mesh is left
/// as it was.
#[derive(Debug, Error)]
pub enum ImportError {
    #[error(transparent)]
    Glb(#[from] GlbError),
    #[error("JSON chunk: {0}")]
    Json(#[from] serde_json::Error),
}

/// The result of a pass: the rewritten file when the pass changed
/// anything, and what it did.
#[derive(Debug, Clone)]
pub struct Optimized {
    pub bytes: Option<Vec<u8>>,
    pub report: Report,
}

/// Replaces the triangles of every part modelled as one large polygon with
/// the triangulation of the same outline that folds least across the
/// mesh's morph targets. Vertices, the JSON chunk and every other mesh are
/// unchanged; see [`morph_triangles`] for which parts qualify.
pub fn optimize_morph_triangles(glb: &[u8]) -> Result<Optimized, ImportError> {
    let mut container = Glb::parse(glb)?;
    let json: serde_json::Value = serde_json::from_slice(&container.json)?;
    let Some(bin) = container.tail.iter_mut().find(|c| c.kind == CHUNK_BIN) else {
        return Ok(Optimized {
            bytes: None,
            report: Report::default(),
        });
    };
    let report = morph_triangles::optimize(&json, &mut bin.data);
    let bytes = report.changed().then(|| container.to_bytes());
    Ok(Optimized { bytes, report })
}
