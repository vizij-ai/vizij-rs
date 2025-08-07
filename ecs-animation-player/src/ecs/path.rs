use bevy::reflect::Reflect;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Reflect)]
#[reflect(Hash, PartialEq)]
pub struct BevyPath(String);

impl BevyPath {
    pub fn parse(s: &str) -> Result<Self, ()> {
        Ok(BevyPath(s.to_string()))
    }
}

impl fmt::Display for BevyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
