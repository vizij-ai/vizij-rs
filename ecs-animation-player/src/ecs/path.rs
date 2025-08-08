use bevy_reflect::{Access, ParsedPath};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BevyPath {
    pub path: String,
    pub parsed: ParsedPath,
}

impl BevyPath {
    pub fn parse(s: &str) -> Result<Self, String> {
        let parsed = ParsedPath::parse(s).map_err(|e| e.to_string())?;
        Ok(Self {
            path: s.to_string(),
            parsed,
        })
    }

    pub fn component(&self) -> Option<&str> {
        self.parsed
            .0
            .first()
            .and_then(|offset| match &offset.access {
                Access::Field(name) => Some(name.as_ref()),
                _ => None,
            })
    }

    pub fn property(&self) -> Option<ParsedPath> {
        if self.parsed.0.len() > 1 {
            Some(ParsedPath(self.parsed.0[1..].to_vec()))
        } else {
            None
        }
    }
}


impl fmt::Display for BevyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.path.fmt(f)
    }
}

impl Default for BevyPath {
    fn default() -> Self {
        Self {
            path: String::new(),
            parsed: ParsedPath(Vec::new()),
        }
    }
}
