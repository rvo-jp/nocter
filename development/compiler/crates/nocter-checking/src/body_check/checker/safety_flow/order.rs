use std::collections::BTreeSet;

use super::SafetyValue;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum SafetySymbol {
    Value(SafetyValue),
    Length(SafetyValue),
}

/// One normalized ordering edge. `strict` distinguishes `<` from `<=`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct OrderFact {
    pub(super) lower: SafetySymbol,
    pub(super) upper: SafetySymbol,
    pub(super) strict: bool,
}

/// Transitively closed path-sensitive ordering facts.
///
/// Closure is part of insertion so joins intersect semantic facts rather than whichever source
/// comparisons happened to spell those facts on each incoming path.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct OrderRelations {
    facts: BTreeSet<OrderFact>,
}

impl OrderRelations {
    pub(super) fn insert(&mut self, fact: OrderFact) -> bool {
        self.facts.insert(OrderFact {
            strict: false,
            ..fact
        });
        if fact.strict {
            self.facts.insert(fact);
        }
        self.close();
        !self.proves(fact.lower, fact.lower, true) && !self.proves(fact.upper, fact.upper, true)
    }

    pub(super) fn proves(&self, lower: SafetySymbol, upper: SafetySymbol, strict: bool) -> bool {
        self.facts.contains(&OrderFact {
            lower,
            upper,
            strict,
        })
    }

    pub(super) fn has_strict_upper_bound(&self, lower: SafetySymbol) -> bool {
        self.facts
            .iter()
            .any(|fact| fact.lower == lower && fact.strict)
    }

    pub(super) fn retain_shared(&mut self, incoming: &Self) {
        self.facts.retain(|fact| incoming.facts.contains(fact));
    }

    fn close(&mut self) {
        loop {
            let current = self.facts.iter().copied().collect::<Vec<_>>();
            let mut changed = false;
            for left in &current {
                for right in &current {
                    if left.upper != right.lower {
                        continue;
                    }
                    let fact = OrderFact {
                        lower: left.lower,
                        upper: right.upper,
                        strict: left.strict || right.strict,
                    };
                    changed |= self.facts.insert(fact);
                    if fact.strict {
                        changed |= self.facts.insert(OrderFact {
                            strict: false,
                            ..fact
                        });
                    }
                }
            }
            if !changed {
                break;
            }
        }
    }
}
