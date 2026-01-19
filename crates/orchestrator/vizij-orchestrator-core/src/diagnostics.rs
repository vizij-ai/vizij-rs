use serde::{Deserialize, Serialize};

/// Minimal diagnostics configuration placeholder.
///
/// Expanded later with logging, sampling, and export hooks. The struct remains
/// here so serialized configs have a stable place to land.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsCfg {
    /// Toggle diagnostics collection for orchestrator runs.
    ///
    /// The current runtime does not consume this flag directly.
    pub enabled: bool,
}

impl Default for DiagnosticsCfg {
    fn default() -> Self {
        DiagnosticsCfg { enabled: true }
    }
}
