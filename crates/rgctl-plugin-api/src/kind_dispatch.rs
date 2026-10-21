//! Dense `u16` node-kind jump table for O(1) AST dispatch.

use crate::HandlerMask;
use std::sync::OnceLock;

/// Practical upper bound for named kind ids in Tier-1 grammars (fits L1).
pub const MAX_KIND_ID: usize = 512;

/// Direct-indexed handler mask table: `slots[kind_id]`.
#[derive(Debug, Clone)]
pub struct KindDispatchTable {
    slots: Box<[HandlerMask]>,
}

impl Default for KindDispatchTable {
    fn default() -> Self {
        Self::empty()
    }
}

impl KindDispatchTable {
    /// All-`SKIP` table of [`MAX_KIND_ID`] slots.
    pub fn empty() -> Self {
        Self {
            slots: vec![HandlerMask::SKIP; MAX_KIND_ID].into_boxed_slice(),
        }
    }

    /// Build from `(kind_id, mask)` pairs (ids ≥ [`MAX_KIND_ID`] are ignored).
    pub fn from_entries(entries: impl IntoIterator<Item = (u16, HandlerMask)>) -> Self {
        let mut table = Self::empty();
        for (id, mask) in entries {
            table.set(id, mask);
        }
        table
    }

    /// Build from coverage `(kind_name, label)` using a live grammar.
    pub fn from_coverage(
        language: &tree_sitter::Language,
        entries: impl IntoIterator<Item = (impl AsRef<str>, impl AsRef<str>)>,
    ) -> Self {
        let mut table = Self::empty();
        for (name, label) in entries {
            let Some(mask) = HandlerMask::from_coverage_label(label.as_ref()) else {
                continue;
            };
            let id = language.id_for_node_kind(name.as_ref(), true);
            if id == 0 {
                continue;
            }
            table.set(id, mask);
        }
        table
    }

    /// Set/union mask at `kind_id`.
    pub fn set(&mut self, kind_id: u16, mask: HandlerMask) {
        let idx = kind_id as usize;
        if idx >= self.slots.len() {
            return;
        }
        self.slots[idx] |= mask;
    }

    /// O(1) lookup; unknown / out-of-range → [`HandlerMask::SKIP`].
    #[inline]
    pub fn lookup(&self, kind_id: u16) -> HandlerMask {
        self.slots
            .get(kind_id as usize)
            .copied()
            .unwrap_or(HandlerMask::SKIP)
    }

    /// True if any semantic handler bit is set.
    #[inline]
    pub fn is_active(&self, kind_id: u16) -> bool {
        !self.lookup(kind_id).is_passive()
    }

    /// Slot count.
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Always false for default-sized tables.
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|m| *m == HandlerMask::SKIP)
    }
}

/// Helper to store a process-wide table behind [`OnceLock`].
pub type KindDispatchOnce = OnceLock<KindDispatchTable>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_round_trip() {
        let mut t = KindDispatchTable::empty();
        t.set(42, HandlerMask::SYMBOL | HandlerMask::RELATION);
        assert!(t.lookup(42).intersects(HandlerMask::SYMBOL));
        assert_eq!(t.lookup(0), HandlerMask::SKIP);
        assert_eq!(t.lookup(600), HandlerMask::SKIP);
    }

    #[test]
    fn from_coverage_rust_function() {
        let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let t = KindDispatchTable::from_coverage(&lang, [("function_item", "Symbol")]);
        let id = lang.id_for_node_kind("function_item", true);
        assert!(t.is_active(id));
    }
}
