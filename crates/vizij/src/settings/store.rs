//! What the pane keeps from one launch to the next, in `settings.json` in
//! the app's data directory: the background chosen for each face, by the
//! face's id, so that opening a face again brings its background back.
//!
//! ```json
//! { "backgrounds": { "quori_latest": "#737373" } }
//! ```
//!
//! The Studio identity is not here: arora and the native driver keep it
//! beside the device's credentials.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The settings kept, and the file they are kept in.
pub(super) struct Kept {
    path: PathBuf,
    settings: Settings,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct Settings {
    /// Face id → its background, `#rrggbb`.
    #[serde(default)]
    backgrounds: BTreeMap<String, String>,
}

impl Kept {
    /// The settings kept at `path`; none when the file is missing or does not
    /// read, which the log says.
    pub(super) fn load(path: PathBuf) -> Self {
        let settings = match std::fs::read_to_string(&path) {
            Ok(json) => serde_json::from_str(&json).unwrap_or_else(|e| {
                log::warn!(
                    "settings: {} does not read ({e}); starting afresh",
                    path.display()
                );
                Settings::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Settings::default(),
            Err(e) => {
                log::warn!("settings: cannot read {}: {e}", path.display());
                Settings::default()
            }
        };
        Self { path, settings }
    }

    /// The background kept for the face `face_id`.
    pub(super) fn background(&self, face_id: &str) -> Option<[u8; 3]> {
        self.settings
            .backgrounds
            .get(face_id)
            .and_then(|hex| crate::face::parse_rgb(hex).ok())
    }

    /// Keep `rgb` as the background of the face `face_id`, on disk at once.
    pub(super) fn set_background(&mut self, face_id: &str, [r, g, b]: [u8; 3]) {
        self.settings
            .backgrounds
            .insert(face_id.to_string(), format!("#{r:02x}{g:02x}{b:02x}"));
        if let Err(e) = save(&self.path, &self.settings) {
            log::warn!("settings: cannot keep them in {}: {e}", self.path.display());
        }
    }
}

/// Write `settings` beside `path` and rename it over it, so that a reader
/// never sees a partial file.
fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(settings)?;
    let partial = path.with_extension("json.partial");
    std::fs::write(&partial, json)?;
    std::fs::rename(&partial, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("vizij-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("settings.json")
    }

    #[test]
    fn a_background_kept_for_a_face_reads_back_at_the_next_load() {
        let path = scratch("round-trip");
        let mut kept = Kept::load(path.clone());
        assert_eq!(kept.background("quori"), None);
        kept.set_background("quori", [0x73, 0x73, 0x73]);
        kept.set_background("toasty", [0, 0x80, 0xff]);

        let again = Kept::load(path.clone());
        assert_eq!(again.background("quori"), Some([0x73, 0x73, 0x73]));
        assert_eq!(again.background("toasty"), Some([0, 0x80, 0xff]));
        assert_eq!(again.background("other"), None);
        let json = std::fs::read_to_string(&path).unwrap();
        assert!(json.contains("\"quori\": \"#737373\""), "{json}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_file_that_does_not_read_starts_afresh_and_is_replaced_on_change() {
        let path = scratch("damaged");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ not json").unwrap();
        let mut kept = Kept::load(path.clone());
        assert_eq!(kept.background("quori"), None);
        kept.set_background("quori", [1, 2, 3]);
        assert_eq!(
            Kept::load(path.clone()).background("quori"),
            Some([1, 2, 3])
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
