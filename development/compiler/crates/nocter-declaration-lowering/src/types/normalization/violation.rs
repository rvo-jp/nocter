use nocter_syntax::SyntaxOrigin;

/// Stable source-level rule selected while canonicalizing declaration-header types.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TypeNormalizationRule {
    RecursiveAlias,
    UnknownAssociatedType,
    AmbiguousAssociatedType,
    AmbiguousCallableProvenance,
    InvalidCallableRequirement,
}

impl TypeNormalizationRule {
    pub const ALL: [Self; 5] = [
        Self::RecursiveAlias,
        Self::UnknownAssociatedType,
        Self::AmbiguousAssociatedType,
        Self::AmbiguousCallableProvenance,
        Self::InvalidCallableRequirement,
    ];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::RecursiveAlias => "E0310",
            Self::UnknownAssociatedType => "E0311",
            Self::AmbiguousAssociatedType => "E0312",
            Self::AmbiguousCallableProvenance => "E0313",
            Self::InvalidCallableRequirement => "E0314",
        }
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::RecursiveAlias => "type aliases form a recursive expansion cycle",
            Self::UnknownAssociatedType => "type selection names no associated type",
            Self::AmbiguousAssociatedType => "type selection does not identify one associated type",
            Self::AmbiguousCallableProvenance => {
                "structural callable result provenance cannot be inferred uniquely"
            }
            Self::InvalidCallableRequirement => {
                "callable requirement does not resolve to a callable type"
            }
        }
    }

    #[must_use]
    pub const fn help(self) -> &'static str {
        match self {
            Self::RecursiveAlias => "break the alias cycle with a nominal or indirect type",
            Self::UnknownAssociatedType => {
                "select an associated type provided by the base type's interface requirements"
            }
            Self::AmbiguousAssociatedType => {
                "make the base type's interface requirement identify one associated declaration"
            }
            Self::AmbiguousCallableProvenance => {
                "add an explicit from clause naming the result's parameter origins"
            }
            Self::InvalidCallableRequirement => {
                "use a callable type or an alias that resolves to one after the colon"
            }
        }
    }

    #[must_use]
    pub const fn related_message(self) -> Option<&'static str> {
        match self {
            Self::RecursiveAlias => Some("another alias in this cycle is declared here"),
            Self::UnknownAssociatedType
            | Self::AmbiguousAssociatedType
            | Self::AmbiguousCallableProvenance
            | Self::InvalidCallableRequirement => None,
        }
    }
}

/// Exact syntax subjects retained when a type-normalization rule is selected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeNormalizationViolation {
    rule: TypeNormalizationRule,
    primary: SyntaxOrigin,
    related: Box<[SyntaxOrigin]>,
}

impl TypeNormalizationViolation {
    #[must_use]
    pub fn new(rule: TypeNormalizationRule, primary: SyntaxOrigin) -> Self {
        Self {
            rule,
            primary,
            related: Box::new([]),
        }
    }

    pub(crate) fn alias_cycle(mut aliases: Vec<SyntaxOrigin>) -> Option<Self> {
        let primary = aliases.first().copied()?;
        aliases.remove(0);
        Some(Self {
            rule: TypeNormalizationRule::RecursiveAlias,
            primary,
            related: aliases.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn rule(&self) -> TypeNormalizationRule {
        self.rule
    }

    #[must_use]
    pub const fn primary(&self) -> SyntaxOrigin {
        self.primary
    }

    #[must_use]
    pub const fn related(&self) -> &[SyntaxOrigin] {
        &self.related
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::TypeNormalizationRule;

    #[test]
    fn type_normalization_rule_codes_are_closed_and_unique() {
        let codes: BTreeSet<_> = TypeNormalizationRule::ALL
            .into_iter()
            .map(TypeNormalizationRule::code)
            .collect();
        assert_eq!(codes.len(), TypeNormalizationRule::ALL.len());
    }
}
