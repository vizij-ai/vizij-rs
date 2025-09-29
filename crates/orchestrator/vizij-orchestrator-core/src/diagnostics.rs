use serde::{Deserialize, Serialize};

/// Minimal diagnostics configuration placeholder.
/// Expanded later with logging, sampling, and export hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsCfg {
    pub enabled: bool,
}

impl Default for DiagnosticsCfg {
    fn default() -> Self {
        DiagnosticsCfg { enabled: true }
    }
}
