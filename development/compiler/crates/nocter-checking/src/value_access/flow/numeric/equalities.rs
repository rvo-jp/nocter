//! Exact differences between current usize cells, not equality of source recipes or executions.
use std::collections::BTreeMap;
#[cfg(test)]
mod tests;

const LIMIT: i128 = u64::MAX as i128;
pub(super) const ZERO: usize = 0;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Entry {
    root: usize,
    offset: i128,
}
/// Canonical weighted partition: value(cell) = value(root) + offset. Cell zero is constant zero.
/// Join keeps only differences common to both inputs; each strict join splits a class, so its
/// height depends on the fixed cell space, not an array's length or number of loop iterations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Equalities {
    entries: Box<[Entry]>,
    possible: bool,
}
impl Equalities {
    pub(super) fn top(variables: usize) -> Option<Self> {
        let count = variables.checked_add(1)?;
        let mut entries = Vec::new();
        entries.try_reserve_exact(count).ok()?;
        entries.extend((0..count).map(|root| Entry { root, offset: 0 }));
        Some(Self {
            entries: entries.into(),
            possible: true,
        })
    }
    pub(super) fn possible(&self) -> bool {
        self.possible
    }
    pub(super) fn difference(&self, left: usize, right: usize) -> Option<i128> {
        if !self.possible {
            return None;
        }
        let left = self.entries.get(left)?;
        let right = self.entries.get(right)?;
        (left.root == right.root).then_some(left.offset - right.offset)
    }
    pub(super) fn constant(&self, cell: usize) -> Option<u64> {
        u64::try_from(self.difference(cell, ZERO)?).ok()
    }
    pub(super) fn bottom(&mut self) {
        self.possible = false;
        for (root, entry) in self.entries.iter_mut().enumerate() {
            *entry = Entry { root, offset: 0 };
        }
    }
    /// Detach only this current cell; snapshots and other members retain their old relations.
    pub(super) fn forget(&mut self, cell: usize) -> bool {
        if cell == ZERO || cell >= self.entries.len() {
            return false;
        }
        if !self.possible {
            return true;
        }
        let root = self.entries[cell].root;
        self.entries[cell] = Entry {
            root: cell,
            offset: 0,
        };
        // A detached representative must not be mistaken for the remaining old class.
        let remaining: Vec<_> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(id, entry)| (id != cell && entry.root == root).then_some(id))
            .collect();
        if let Some(&next) = remaining.first() {
            let base = self.entries[next].offset;
            for id in remaining {
                self.entries[id] = Entry {
                    root: next,
                    offset: self.entries[id].offset - base,
                };
            }
        }
        true
    }
    /// A reached normal checked assignment. Its mathematical result must remain a usize.
    pub(super) fn assign(&mut self, target: usize, source: usize, offset: i128) -> bool {
        if target == ZERO || target >= self.entries.len() || source >= self.entries.len() {
            return false;
        }
        if !self.possible {
            return true;
        }
        if !(-LIMIT..=LIMIT).contains(&offset) {
            self.bottom();
            return true;
        }
        if target == source {
            let root = self.entries[target].root;
            self.entries[target].offset += offset;
            self.normalize(root);
        } else {
            self.forget(target);
            let from = self.entries[source];
            self.entries[target] = Entry {
                root: from.root,
                offset: from.offset + offset,
            };
            self.normalize(from.root);
        }
        true
    }
    pub(super) fn assume(&mut self, left: usize, right: usize, difference: i128) -> bool {
        if left >= self.entries.len() || right >= self.entries.len() {
            return false;
        }
        if !self.possible {
            return true;
        }
        if !(-LIMIT..=LIMIT).contains(&difference) {
            self.bottom();
            return true;
        }
        let a = self.entries[left];
        let b = self.entries[right];
        if a.root == b.root {
            if a.offset - b.offset != difference {
                self.bottom();
            }
            return true;
        }
        let shift = b.offset + difference - a.offset;
        for entry in &mut self.entries {
            if entry.root == a.root {
                entry.root = b.root;
                entry.offset += shift;
            }
        }
        self.normalize(b.root);
        true
    }
    fn normalize(&mut self, root: usize) {
        let members: Vec<_> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(id, entry)| (entry.root == root).then_some(id))
            .collect();
        let Some(&first) = members.first() else {
            return;
        };
        let base = self.entries[first].offset;
        let (mut low, mut high) = (0, 0);
        for id in members {
            let offset = self.entries[id].offset - base;
            self.entries[id] = Entry {
                root: first,
                offset,
            };
            low = low.min(offset);
            high = high.max(offset);
        }
        // All cells are usize. These tests also keep every stored offset within +/- LIMIT;
        // intermediate rebasing/assignment arithmetic therefore fits comfortably in i128.
        if high - low > LIMIT || (first == ZERO && (low < 0 || high > LIMIT)) {
            self.bottom();
        }
    }
    pub(super) fn join(&self, other: &Self) -> Option<Self> {
        if self.entries.len() != other.entries.len() {
            return None;
        }
        if !self.possible {
            return Some(other.clone());
        }
        if !other.possible {
            return Some(self.clone());
        }
        let mut groups = BTreeMap::new();
        let entries = self
            .entries
            .iter()
            .zip(&other.entries)
            .enumerate()
            .map(|(id, (a, b))| {
                // Two members share an output class precisely when both inputs agree on their
                // difference. Representatives are chosen by cell order, never by numeric value.
                let (root, base) = *groups
                    .entry((a.root, b.root, a.offset - b.offset))
                    .or_insert((id, a.offset));
                Entry {
                    root,
                    offset: a.offset - base,
                }
            })
            .collect();
        Some(Self {
            entries,
            possible: true,
        })
    }
}
