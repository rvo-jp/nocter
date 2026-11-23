//! One sparse retirement frontier for an owning evaluator slot, including moved prefixes.
use super::{Coordinate, Path, Result, Rule, Storage};
use crate::state::budget::Resident;
use std::collections::BTreeMap;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(in crate::state) enum Hole {
    Value(Path),
    Array { base: Path, start: u64, end: u64 },
}
#[derive(Default)]
pub(in crate::state) struct Ownership {
    holes: BTreeMap<Hole, Resident<()>>,
}
impl Hole {
    fn contains(&self, path: &[Coordinate]) -> bool {
        match self {
            Self::Value(hole) => path.starts_with(hole),
            Self::Array { base, start, end } => {
                path.starts_with(base)
                    && path
                        .get(base.len())
                        .and_then(|index| index.array_index())
                        .is_some_and(|i| *start <= i && i < *end)
            }
        }
    }
    fn under(&self, path: &[Coordinate]) -> bool {
        match self {
            Self::Value(hole) => hole.starts_with(path),
            Self::Array { base, .. } => base.starts_with(path),
        }
    }
    fn overlaps(&self, path: &[Coordinate]) -> bool {
        self.contains(path) || self.under(path)
    }
    fn overlaps_array(&self, base: &Path, start: u64, end: u64) -> bool {
        if self.contains(base) {
            return true;
        }
        let selected = match self {
            Self::Value(path) => path,
            Self::Array {
                base: other,
                start: a,
                end: b,
            } if other == base => {
                return *a < end && start < *b;
            }
            Self::Array { base, .. } => base,
        };
        selected.starts_with(base)
            && selected
                .get(base.len())
                .and_then(|index| index.array_index())
                .is_some_and(|i| start <= i && i < end)
    }
}
impl Ownership {
    pub(in crate::state) fn is_empty(&self) -> bool {
        self.holes.is_empty()
    }
    pub(in crate::state) fn is_retired(&self, path: &Path) -> bool {
        self.holes.contains_key(&Hole::Value(path.clone()))
    }
    pub(in crate::state) fn matches(&self, expected: &[Hole]) -> bool {
        self.holes.keys().eq(expected)
    }
    pub(super) fn consolidate_prefix(
        &mut self,
        storage: &Storage,
        base: &Path,
        selected: &Path,
        end: u64,
    ) -> Result<()> {
        let previous = end.checked_sub(1).ok_or(Rule::InvalidPlan)?;
        let element = Hole::Value(selected.clone());
        let prefix = Hole::Array {
            base: base.clone(),
            start: 0,
            end: previous,
        };
        if !self.holes.contains_key(&element)
            || (previous != 0 && !self.holes.contains_key(&prefix))
        {
            return Err(Rule::InvalidPlan);
        }
        // Reserve before mutation. The frontier stays sparse and failure retains both old entries.
        let allocation = storage.entry(())?;
        self.holes.remove(&element);
        if previous != 0 {
            self.holes.remove(&prefix);
        }
        self.holes.insert(
            Hole::Array {
                base: base.clone(),
                start: 0,
                end,
            },
            allocation,
        );
        Ok(())
    }
    pub(super) fn restore(&mut self, path: &Path) -> Result<()> {
        if self.holes.keys().any(|hole| match hole {
            Hole::Value(h) => h != path && hole.contains(path),
            // Region consolidation grants no per-element restoration operation.
            Hole::Array { .. } => hole.contains(path),
        }) {
            return Err(Rule::InvalidPlan);
        }
        self.holes.retain(|hole, _| !hole.under(path));
        Ok(())
    }
    pub(super) fn retire(&mut self, storage: &Storage, path: &Path, partial: bool) -> Result<()> {
        if !self.can_release(path, partial) {
            return Err(Rule::InvalidPlan);
        }
        let allocation = storage.entry(())?;
        self.holes.retain(|hole, _| !hole.under(path));
        self.holes.insert(Hole::Value(path.clone()), allocation);
        Ok(())
    }
    pub(super) fn retire_array(
        &mut self,
        storage: &Storage,
        base: &Path,
        start: u64,
        end: u64,
    ) -> Result<()> {
        if start >= end
            || self
                .holes
                .keys()
                .any(|hole| hole.overlaps_array(base, start, end))
        {
            return Err(Rule::InvalidPlan);
        }
        self.holes.insert(
            Hole::Array {
                base: base.clone(),
                start,
                end,
            },
            storage.entry(())?,
        );
        Ok(())
    }
    pub(in crate::state) fn can_release(&self, path: &Path, partial: bool) -> bool {
        !self
            .holes
            .keys()
            .any(|hole| hole.contains(path) || (!partial && hole.overlaps(path)))
    }
}
