//! Minimal GLB (binary glTF) container codec.
//!
//! A GLB file is a 12-byte header (`glTF` magic, container version, total
//! length) followed by chunks, each an 8-byte header (payload length, type)
//! plus a payload aligned to 4 bytes. The first chunk holds the glTF JSON
//! document (space-padded); the following chunk, when present, holds the
//! binary buffer (`BIN\0`, zero-padded). This codec re-serializes only the
//! JSON chunk; every later chunk is preserved byte-for-byte, padding
//! included.

use thiserror::Error;

pub const GLB_MAGIC: [u8; 4] = *b"glTF";
pub const CHUNK_JSON: [u8; 4] = *b"JSON";
pub const CHUNK_BIN: [u8; 4] = *b"BIN\0";

#[derive(Debug, Error)]
pub enum GlbError {
    #[error("not a GLB file (missing glTF magic)")]
    BadMagic,
    #[error("unsupported GLB container version {0} (expected 2)")]
    UnsupportedVersion(u32),
    #[error("file truncated ({0})")]
    Truncated(&'static str),
    #[error("header declares {declared} bytes but the file has {actual}")]
    LengthMismatch { declared: u32, actual: usize },
    #[error("first chunk is not the JSON chunk")]
    MissingJsonChunk,
}

/// A chunk following the JSON chunk, preserved byte-for-byte (its stored
/// payload keeps whatever padding the source file used).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub kind: [u8; 4],
    pub data: Vec<u8>,
}

/// A parsed GLB container: the JSON chunk payload plus every later chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glb {
    pub version: u32,
    /// JSON chunk payload as stored (trailing space padding included).
    pub json: Vec<u8>,
    /// Chunks after the JSON chunk (typically the single `BIN\0` chunk),
    /// written back verbatim by [`Glb::to_bytes`].
    pub tail: Vec<Chunk>,
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    bytes
        .get(offset..offset + 4)
        .map(|b| u32::from_le_bytes(b.try_into().expect("4-byte slice")))
}

impl Glb {
    pub fn parse(bytes: &[u8]) -> Result<Self, GlbError> {
        if bytes.len() < 12 {
            return Err(GlbError::Truncated("header"));
        }
        if bytes[0..4] != GLB_MAGIC {
            return Err(GlbError::BadMagic);
        }
        let version = read_u32(bytes, 4).expect("length checked");
        if version != 2 {
            return Err(GlbError::UnsupportedVersion(version));
        }
        let declared = read_u32(bytes, 8).expect("length checked");
        if declared as usize != bytes.len() {
            return Err(GlbError::LengthMismatch {
                declared,
                actual: bytes.len(),
            });
        }

        let mut offset = 12;
        let mut json: Option<Vec<u8>> = None;
        let mut tail = Vec::new();
        while offset < bytes.len() {
            let length =
                read_u32(bytes, offset).ok_or(GlbError::Truncated("chunk header"))? as usize;
            let kind: [u8; 4] = bytes
                .get(offset + 4..offset + 8)
                .ok_or(GlbError::Truncated("chunk header"))?
                .try_into()
                .expect("4-byte slice");
            let data = bytes
                .get(offset + 8..offset + 8 + length)
                .ok_or(GlbError::Truncated("chunk payload"))?
                .to_vec();
            offset += 8 + length;
            match &json {
                None => {
                    if kind != CHUNK_JSON {
                        return Err(GlbError::MissingJsonChunk);
                    }
                    json = Some(data);
                }
                Some(_) => tail.push(Chunk { kind, data }),
            }
        }
        let json = json.ok_or(GlbError::MissingJsonChunk)?;
        Ok(Glb {
            version,
            json,
            tail,
        })
    }

    /// Serialize back to GLB bytes: the JSON payload padded to a 4-byte
    /// boundary with spaces, tail chunks exactly as stored, chunk lengths
    /// and the header total length recomputed.
    pub fn to_bytes(&self) -> Vec<u8> {
        let json_padded_len = self.json.len().div_ceil(4) * 4;
        let mut total = 12 + 8 + json_padded_len;
        for chunk in &self.tail {
            total += 8 + chunk.data.len();
        }

        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(&GLB_MAGIC);
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(json_padded_len as u32).to_le_bytes());
        out.extend_from_slice(&CHUNK_JSON);
        out.extend_from_slice(&self.json);
        out.resize(out.len() + (json_padded_len - self.json.len()), b' ');
        for chunk in &self.tail {
            out.extend_from_slice(&(chunk.data.len() as u32).to_le_bytes());
            out.extend_from_slice(&chunk.kind);
            out.extend_from_slice(&chunk.data);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Glb {
        Glb {
            version: 2,
            json: br#"{"asset":{"version":"2.0"}}"#.to_vec(),
            tail: vec![Chunk {
                kind: CHUNK_BIN,
                data: vec![1, 2, 3, 4, 5, 0, 0, 0],
            }],
        }
    }

    #[test]
    fn round_trips_with_valid_alignment_and_lengths() {
        let glb = sample();
        let bytes = glb.to_bytes();

        // Header: magic, version, total length matching the buffer.
        assert_eq!(&bytes[0..4], b"glTF");
        assert_eq!(read_u32(&bytes, 4), Some(2));
        assert_eq!(read_u32(&bytes, 8), Some(bytes.len() as u32));
        assert_eq!(bytes.len() % 4, 0);

        // JSON chunk: length 4-byte aligned, space padding.
        let json_len = read_u32(&bytes, 12).unwrap() as usize;
        assert_eq!(json_len % 4, 0);
        assert_eq!(&bytes[16..20], b"JSON");
        assert_eq!(json_len - glb.json.len(), 1, "27-byte payload pads to 28");
        assert_eq!(bytes[20 + json_len - 1], b' ');

        let parsed = Glb::parse(&bytes).expect("round trip");
        assert_eq!(parsed.version, 2);
        assert_eq!(&parsed.json[..glb.json.len()], &glb.json[..]);
        assert!(parsed.json[glb.json.len()..].iter().all(|b| *b == b' '));
        assert_eq!(parsed.tail, glb.tail);

        // Re-serializing a parsed container reproduces the same bytes.
        assert_eq!(parsed.to_bytes(), bytes);
    }

    #[test]
    fn round_trips_without_tail_chunks() {
        let glb = Glb {
            tail: Vec::new(),
            ..sample()
        };
        let bytes = glb.to_bytes();
        let parsed = Glb::parse(&bytes).expect("round trip");
        assert!(parsed.tail.is_empty());
        assert_eq!(parsed.to_bytes(), bytes);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = sample().to_bytes();
        bytes[0] = b'x';
        assert!(matches!(Glb::parse(&bytes), Err(GlbError::BadMagic)));
    }

    #[test]
    fn rejects_unsupported_version() {
        let mut bytes = sample().to_bytes();
        bytes[4] = 3;
        assert!(matches!(
            Glb::parse(&bytes),
            Err(GlbError::UnsupportedVersion(3))
        ));
    }

    #[test]
    fn rejects_truncation_and_length_mismatch() {
        let bytes = sample().to_bytes();
        assert!(matches!(
            Glb::parse(&bytes[..8]),
            Err(GlbError::Truncated(_))
        ));
        // Cutting the buffer breaks the declared total length.
        assert!(matches!(
            Glb::parse(&bytes[..bytes.len() - 4]),
            Err(GlbError::LengthMismatch { .. })
        ));
    }

    #[test]
    fn rejects_leading_non_json_chunk() {
        let glb = Glb {
            version: 2,
            json: Vec::new(),
            tail: Vec::new(),
        };
        let mut bytes = glb.to_bytes();
        bytes[16..20].copy_from_slice(b"BIN\0");
        assert!(matches!(
            Glb::parse(&bytes),
            Err(GlbError::MissingJsonChunk)
        ));
    }
}
