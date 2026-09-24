/// Source-level modifiers that describe a callable contract or execution form.
///
/// The declaration prefix order is language syntax. Semantic layers project these spellings into
/// guarantees and execution facts, but must not define another ordering.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableModifier {
    CompileTime,
    NoAllocation,
    NoTrap,
    Blocking,
    Async,
}

impl CallableModifier {
    /// Canonical declaration-prefix order.
    pub const ALL: &'static [Self] = &[
        Self::CompileTime,
        Self::NoAllocation,
        Self::NoTrap,
        Self::Blocking,
        Self::Async,
    ];

    #[must_use]
    pub fn from_spelling(text: &str) -> Option<Self> {
        match text {
            "const" => Some(Self::CompileTime),
            "noalloc" => Some(Self::NoAllocation),
            "notrap" => Some(Self::NoTrap),
            "blocking" => Some(Self::Blocking),
            "async" => Some(Self::Async),
            _ => None,
        }
    }

    #[must_use]
    pub const fn spelling(self) -> &'static str {
        match self {
            Self::CompileTime => "const",
            Self::NoAllocation => "noalloc",
            Self::NoTrap => "notrap",
            Self::Blocking => "blocking",
            Self::Async => "async",
        }
    }

    /// Whether both modifiers may describe the same callable declaration.
    ///
    /// Parsing intentionally retains incompatible canonical combinations so checking can issue a
    /// semantic diagnostic. Completion uses this relation to avoid proposing such a combination.
    #[must_use]
    pub const fn is_compatible_with(self, other: Self) -> bool {
        match (self, other) {
            (Self::Async, Self::Async) => true,
            (Self::Async, _) | (_, Self::Async) => false,
            _ => true,
        }
    }
}

/// Modifier vocabulary admitted by one callable-shaped syntax production.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CallablePrefixGrammar {
    /// `const noalloc notrap blocking async`, used by functions and methods.
    DeferredAllowed,
    /// `const noalloc notrap blocking`, used by structural callable types and primitive functions.
    Immediate,
    /// `noalloc notrap`, used by literals, operators, coercions, and destruction.
    ImmediateGuarantees,
}

impl CallablePrefixGrammar {
    #[must_use]
    pub const fn admits(self, modifier: CallableModifier) -> bool {
        match self {
            Self::DeferredAllowed => true,
            Self::Immediate => !matches!(modifier, CallableModifier::Async),
            Self::ImmediateGuarantees => {
                matches!(
                    modifier,
                    CallableModifier::NoAllocation | CallableModifier::NoTrap
                )
            }
        }
    }

    /// Whether `modifiers` is a prefix of the canonical order admitted by this production.
    #[must_use]
    pub fn is_canonical_prefix(self, modifiers: &[CallableModifier]) -> bool {
        let mut remaining = CallableModifier::ALL.iter().copied();
        modifiers.iter().copied().all(|modifier| {
            self.admits(modifier)
                && remaining.any(|candidate| candidate == modifier && self.admits(candidate))
        })
    }

    /// Whether completion may append `candidate` to a canonical, semantically compatible prefix.
    #[must_use]
    pub fn can_suggest_after(
        self,
        prefix: &[CallableModifier],
        candidate: CallableModifier,
    ) -> bool {
        if !self.admits(candidate) || !self.is_canonical_prefix(prefix) {
            return false;
        }
        let Some(candidate_index) = CallableModifier::ALL
            .iter()
            .position(|modifier| *modifier == candidate)
        else {
            return false;
        };
        prefix.iter().copied().all(|modifier| {
            CallableModifier::ALL
                .iter()
                .position(|known| *known == modifier)
                .is_some_and(|index| index < candidate_index)
                && modifier.is_compatible_with(candidate)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{CallableModifier, CallablePrefixGrammar};

    #[test]
    fn canonical_order_and_compatibility_are_independent() {
        let grammar = CallablePrefixGrammar::DeferredAllowed;
        assert!(
            grammar
                .is_canonical_prefix(&[CallableModifier::NoAllocation, CallableModifier::Async,])
        );
        assert!(
            !grammar.can_suggest_after(&[CallableModifier::NoAllocation], CallableModifier::Async,)
        );
        assert!(grammar.can_suggest_after(&[], CallableModifier::Async));
    }

    #[test]
    fn narrower_productions_share_the_same_order() {
        assert!(CallablePrefixGrammar::Immediate.can_suggest_after(
            &[CallableModifier::CompileTime],
            CallableModifier::NoAllocation,
        ));
        assert!(!CallablePrefixGrammar::Immediate.can_suggest_after(&[], CallableModifier::Async));
        assert!(
            CallablePrefixGrammar::ImmediateGuarantees
                .can_suggest_after(&[], CallableModifier::NoAllocation)
        );
        assert!(
            !CallablePrefixGrammar::ImmediateGuarantees
                .can_suggest_after(&[], CallableModifier::CompileTime)
        );
        assert!(
            CallablePrefixGrammar::ImmediateGuarantees
                .can_suggest_after(&[CallableModifier::NoAllocation], CallableModifier::NoTrap,)
        );
    }
}
