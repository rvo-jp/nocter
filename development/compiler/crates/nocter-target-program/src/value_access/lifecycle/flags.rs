//! Body-local bit tests and phase writes derived from checked ownership snapshots.
use super::{Action, Alternative, Body, Draft, Failure, INVALID};
use nocter_checking::value_access::flow::{
    Location,
    resources::{Condition, Holder, Missing},
};
use std::collections::{BTreeMap, BTreeSet};

/// A one-bit equality test or assignment, depending on its recipe position.
/// Words belong to a fresh activation, never to the shared code instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BitValue {
    word: usize,
    mask: u64,
    value: bool,
}
impl BitValue {
    #[must_use]
    pub const fn word(self) -> usize {
        self.word
    }
    #[must_use]
    pub const fn mask(self) -> u64 {
        self.mask
    }
    #[must_use]
    pub const fn value(self) -> bool {
        self.value
    }
    fn new(index: usize, value: bool) -> Self {
        Self {
            word: index / 64,
            mask: 1u64 << (index % 64),
            value,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Recipe {
    tests: Box<[BitValue]>,
    before: Box<[BitValue]>,
    after: Box<[BitValue]>,
}
impl Recipe {
    /// Conjunction evaluated once, before either phase changes ownership or flag state.
    #[must_use]
    pub fn tests(&self) -> &[BitValue] {
        &self.tests
    }
    /// Apply after the complete before-step group and before the selected operation starts.
    #[must_use]
    pub fn before_writes(&self) -> &[BitValue] {
        &self.before
    }
    /// Apply after normal operation completion and its complete after-step group.
    #[must_use]
    pub fn after_writes(&self) -> &[BitValue] {
        &self.after
    }
}

/// Packed initialization words. No bank is required for a body without resource choices.
#[derive(Clone, Debug)]
pub struct Layout {
    initial: Box<[u64]>,
}
impl Layout {
    /// Copy this immutable template into each fresh activation's own flag storage.
    #[must_use]
    pub fn initial_words(&self) -> &[u64] {
        &self.initial
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Fact {
    Container(Holder),
    Missing(Holder, Missing),
}
type Snapshot = BTreeSet<Fact>;

fn snapshot(condition: &Condition) -> Snapshot {
    let mut facts = BTreeSet::new();
    for (holder, missing) in condition.owned() {
        facts.insert(Fact::Container(holder));
        facts.extend(
            missing
                .iter()
                .cloned()
                .map(|path| Fact::Missing(holder, path)),
        );
    }
    facts
}

/// Refine indistinguishable groups; skip facts that distinguish no remaining alternatives.
/// This is sparse in observed holes, not in the recursively expanded field schema.
fn discriminants(states: &[Snapshot]) -> Result<Vec<Fact>, Failure> {
    let available: BTreeSet<_> = states.iter().flat_map(BTreeSet::iter).cloned().collect();
    let mut groups = vec![(0..states.len()).collect::<Vec<_>>()];
    let mut selected = Vec::new();
    for fact in available {
        if groups.len() == states.len() {
            break;
        }
        if !groups.iter().any(|group| {
            let first = states[group[0]].contains(&fact);
            group
                .iter()
                .any(|&index| states[index].contains(&fact) != first)
        }) {
            continue;
        }
        let mut next = Vec::new();
        for group in groups {
            let (yes, no): (Vec<_>, Vec<_>) = group
                .into_iter()
                .partition(|&index| states[index].contains(&fact));
            if !yes.is_empty() {
                next.push(yes);
            }
            if !no.is_empty() {
                next.push(no);
            }
        }
        groups = next;
        selected.push(fact);
    }
    if groups.len() != states.len() {
        return Err(INVALID);
    }
    Ok(selected)
}

fn writes(indices: &BTreeMap<Fact, usize>, from: &Snapshot, to: &Snapshot) -> Box<[BitValue]> {
    from.symmetric_difference(to)
        .filter_map(|fact| {
            indices
                .get(fact)
                .map(|&index| BitValue::new(index, to.contains(fact)))
        })
        .collect()
}

pub(super) fn lower(
    drafts: BTreeMap<Location, Vec<Draft>>,
    entry: &Condition,
) -> Result<Body, Failure> {
    let mut guards = BTreeMap::new();
    let mut needed = BTreeSet::new();
    for (&at, alternatives) in &drafts {
        let chosen = if alternatives.len() == 1 {
            Vec::new()
        } else {
            let states = alternatives
                .iter()
                .map(|a| snapshot(&a.condition))
                .collect::<Vec<_>>();
            discriminants(&states)?
        };
        needed.extend(chosen.iter().cloned());
        guards.insert(at, chosen);
    }
    // The construction index is discarded after instruction emission; it is not another registry.
    let indices = needed
        .into_iter()
        .enumerate()
        .map(|(index, fact)| (fact, index))
        .collect::<BTreeMap<_, _>>();
    let mut initial = vec![0u64; indices.len().div_ceil(64)];
    let entry = snapshot(entry);
    for (fact, &index) in &indices {
        if entry.contains(fact) {
            initial[index / 64] |= 1u64 << (index % 64);
        }
    }
    let mut actions = BTreeMap::new();
    for (at, alternatives) in drafts {
        let guard = guards.remove(&at).ok_or(INVALID)?;
        let mut emitted = Vec::new();
        for draft in alternatives {
            let state = |condition: &Condition| {
                if indices.is_empty() {
                    Snapshot::new()
                } else {
                    snapshot(condition)
                }
            };
            let before = state(&draft.condition);
            let operation = state(&draft.operation_state);
            let after = state(&draft.final_state);
            let tests = guard
                .iter()
                .map(|fact| {
                    let index = *indices.get(fact).ok_or(INVALID)?;
                    Ok(BitValue::new(index, before.contains(fact)))
                })
                .collect::<Result<_, Failure>>()?;
            let flags = Recipe {
                tests,
                before: writes(&indices, &before, &operation),
                after: writes(&indices, &operation, &after),
            };
            emitted.push(Alternative {
                before: draft.before,
                after: draft.after,
                flags,
            });
        }
        actions.insert(
            at,
            Action {
                alternatives: simplify(emitted).into(),
            },
        );
    }
    Ok(Body {
        actions,
        flags: Layout {
            initial: initial.into(),
        },
    })
}

fn same_effects(left: &Alternative, right: &Alternative) -> bool {
    left.before == right.before
        && left.after == right.after
        && left.flags.before == right.flags.before
        && left.flags.after == right.flags.after
}

/// The union is exact when equal-key conjunctions differ in at most one bit value.
fn join(left: &[BitValue], right: &[BitValue]) -> Option<Box<[BitValue]>> {
    if left.len() != right.len() {
        return None;
    }
    let mut differing = None;
    for (index, (left, right)) in left.iter().zip(right).enumerate() {
        if left.word != right.word || left.mask != right.mask {
            return None;
        }
        if left.value != right.value {
            if differing.is_some() {
                return None;
            }
            differing = Some(index);
        }
    }
    Some(
        left.iter()
            .enumerate()
            .filter_map(|(index, value)| (Some(index) != differing).then_some(*value))
            .collect(),
    )
}

fn simplify(mut alternatives: Vec<Alternative>) -> Vec<Alternative> {
    if let Some(first) = alternatives.first()
        && alternatives.iter().all(|next| same_effects(first, next))
    {
        let mut shared = alternatives.swap_remove(0);
        shared.flags.tests = Box::new([]);
        return vec![shared];
    }
    // Optional simplification has bounded compile work; exhausting it preserves valid guards.
    let mut comparisons = 4096usize;
    loop {
        let mut merged = None;
        'search: for first in 0..alternatives.len() {
            for second in first + 1..alternatives.len() {
                if comparisons == 0 {
                    return alternatives;
                }
                comparisons -= 1;
                let (left, right) = (&alternatives[first], &alternatives[second]);
                if same_effects(left, right)
                    && let Some(tests) = join(&left.flags.tests, &right.flags.tests)
                {
                    merged = Some((first, second, tests));
                    break 'search;
                }
            }
        }
        let Some((first, second, tests)) = merged else {
            return alternatives;
        };
        alternatives[first].flags.tests = tests;
        alternatives.remove(second);
    }
}

#[cfg(test)]
mod tests {
    use super::BitValue;

    #[test]
    fn packed_bits_match_an_independent_boolean_bank_across_word_boundaries() {
        let bits = (0..130)
            .map(|index| BitValue::new(index, false))
            .collect::<Vec<_>>();
        let mut words = [0u64; 3];
        let mut expected = [false; 130];
        for value in [true, false] {
            for index in 0..expected.len() {
                let bit = bits[index];
                assert_eq!(bit.mask().count_ones(), 1);
                if value {
                    words[bit.word()] |= bit.mask();
                } else {
                    words[bit.word()] &= !bit.mask();
                }
                expected[index] = value;
                for (&selected, &expected) in bits.iter().zip(&expected) {
                    assert_eq!(words[selected.word()] & selected.mask() != 0, expected);
                }
            }
        }
    }
}
