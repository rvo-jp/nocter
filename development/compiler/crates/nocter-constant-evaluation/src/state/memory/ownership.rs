//! One sparse retirement frontier for an owning evaluator slot, including cleanup intervals.
use super::{Path, Result, Rule, Storage};
use crate::state::budget::Resident;
use std::collections::BTreeMap;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
enum Hole {
    Value(Path),
    Array { base: Path, start: u64, end: u64 },
}
#[derive(Default)]
pub(in crate::state) struct Ownership {
    holes: BTreeMap<Hole, Resident<()>>,
}
impl Hole {
    fn contains(&self, path: &[usize]) -> bool {
        match self {
            Self::Value(hole) => path.starts_with(hole),
            Self::Array { base, start, end } => {
                path.starts_with(base)
                    && path
                        .get(base.len())
                        .and_then(|&i| u64::try_from(i).ok())
                        .is_some_and(|i| *start <= i && i < *end)
            }
        }
    }
    fn under(&self, path: &[usize]) -> bool {
        match self {
            Self::Value(hole) => hole.starts_with(path),
            Self::Array { base, .. } => base.starts_with(path),
        }
    }
    fn overlaps(&self, path: &[usize]) -> bool {
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
                .and_then(|&i| u64::try_from(i).ok())
                .is_some_and(|i| start <= i && i < end)
    }
}
impl Ownership {
    pub(in crate::state) fn is_empty(&self) -> bool {
        self.holes.is_empty()
    }
    pub(in crate::state) fn matches(&self, expected: &[Path]) -> bool {
        self.holes
            .keys()
            .map(|hole| match hole {
                Hole::Value(path) => Some(&path[..]),
                Hole::Array { .. } => None,
            })
            .eq(expected.iter().map(|path| Some(&path[..])))
    }
    pub(super) fn restore(&mut self, path: &Path) -> Result<()> {
        if self.holes.keys().any(|hole| match hole {
            Hole::Value(h) => h != path && hole.contains(path),
            // An interval is a cleanup frontier, not a promise to restore one of its elements.
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
    pub(super) fn can_release(&self, path: &Path, partial: bool) -> bool {
        !self
            .holes
            .keys()
            .any(|hole| hole.contains(path) || (!partial && hole.overlaps(path)))
    }
}
