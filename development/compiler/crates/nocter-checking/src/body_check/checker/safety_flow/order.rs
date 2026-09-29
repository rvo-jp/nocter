use std::collections::BTreeSet;

use super::SafetyValue;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum SafetySymbol {
    Value(SafetyValue),
    Length(SafetyValue),
}

/// One normalized ordering edge. `strict` distinguishes `<` from `<=`.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
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
    pub(super) fn insert(&mut self, fact: &OrderFact) -> bool {
        self.facts.insert(OrderFact {
            strict: false,
            ..fact.clone()
        });
        if fact.strict {
            self.facts.insert(fact.clone());
        }
        self.close();
        !self.proves(&fact.lower, &fact.lower, true) && !self.proves(&fact.upper, &fact.upper, true)
    }

    pub(super) fn proves(&self, lower: &SafetySymbol, upper: &SafetySymbol, strict: bool) -> bool {
        self.facts.contains(&OrderFact {
            lower: lower.clone(),
            upper: upper.clone(),
            strict,
        })
    }

    pub(super) fn has_strict_upper_bound(&self, lower: &SafetySymbol) -> bool {
        self.facts
            .iter()
            .any(|fact| &fact.lower == lower && fact.strict)
    }

    pub(super) fn retain_shared(&mut self, incoming: &Self) {
        self.facts.retain(|fact| incoming.facts.contains(fact));
    }

    pub(super) fn equate(&mut self, left: SafetySymbol, right: SafetySymbol) {
        let _ = self.insert(&OrderFact {
            lower: left.clone(),
            upper: right.clone(),
            strict: false,
        });
        let _ = self.insert(&OrderFact {
            lower: right,
            upper: left,
            strict: false,
        });
    }

    pub(super) fn invalidate(&mut self, target: &SafetyValue) {
        self.facts
            .retain(|fact| !fact.lower.depends_on(target) && !fact.upper.depends_on(target));
    }

    fn close(&mut self) {
        loop {
            let current = self.facts.iter().cloned().collect::<Vec<_>>();
            let mut changed = false;
            for left in &current {
                for right in &current {
                    if left.upper != right.lower {
                        continue;
                    }
                    let fact = OrderFact {
                        lower: left.lower.clone(),
                        upper: right.upper.clone(),
                        strict: left.strict || right.strict,
                    };
                    changed |= self.facts.insert(fact.clone());
                    if fact.strict {
                        changed |= self.facts.insert(OrderFact {
                            strict: false,
                            ..fact.clone()
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

impl SafetySymbol {
    fn depends_on(&self, target: &SafetyValue) -> bool {
        match self {
            Self::Value(value) | Self::Length(value) => value.overlaps(target),
        }
    }
}
