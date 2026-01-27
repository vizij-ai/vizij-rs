#![allow(dead_code)]
//! Identifiers and simple allocators for core entities.

use serde::{Deserialize, Serialize};

/// Stable identifier for a loaded animation.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct AnimId(pub u32);

/// Stable identifier for a player.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct PlayerId(pub u32);

/// Stable identifier for an instance.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct InstId(pub u32);

/// Monotonic allocator for `AnimId`, `PlayerId`, and `InstId`.
///
/// Dense indices improve cache locality; IDs are opaque externally. Counters
/// wrap on `u32` overflow, so extremely long-lived sessions can see reuse.
#[derive(Default, Debug)]
pub struct IdAllocator {
    next_anim: u32,
    next_player: u32,
    next_inst: u32,
}

impl IdAllocator {
    /// Create a new allocator with counters reset to zero.
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    /// Allocate the next animation id.
    ///
    /// IDs wrap after `u32::MAX` allocations.
    pub fn alloc_anim(&mut self) -> AnimId {
        let id = AnimId(self.next_anim);
        self.next_anim = self.next_anim.wrapping_add(1);
        id
    }

    #[inline]
    /// Allocate the next player id.
    ///
    /// IDs wrap after `u32::MAX` allocations.
    pub fn alloc_player(&mut self) -> PlayerId {
        let id = PlayerId(self.next_player);
        self.next_player = self.next_player.wrapping_add(1);
        id
    }

    #[inline]
    /// Allocate the next instance id.
    ///
    /// IDs wrap after `u32::MAX` allocations.
    pub fn alloc_inst(&mut self) -> InstId {
        let id = InstId(self.next_inst);
        self.next_inst = self.next_inst.wrapping_add(1);
        id
    }

    #[inline]
    /// Reset all counters back to zero.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Allocates monotonic.
    fn alloc_monotonic() {
        let mut alloc = IdAllocator::new();
        assert_eq!(alloc.alloc_anim(), AnimId(0));
        assert_eq!(alloc.alloc_anim(), AnimId(1));
        assert_eq!(alloc.alloc_player(), PlayerId(0));
        assert_eq!(alloc.alloc_player(), PlayerId(1));
        assert_eq!(alloc.alloc_inst(), InstId(0));
        assert_eq!(alloc.alloc_inst(), InstId(1));
    }
}
