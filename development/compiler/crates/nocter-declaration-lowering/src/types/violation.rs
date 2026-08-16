use nocter_source_index::SyntaxOrigin;

/// Stable source-level rule selected while binding declaration-header type syntax.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TypeBindingRule {
    UnknownTypeContextName,
    InvalidTypeEntity,
    InvalidTypeArguments,
    InvalidSelfType,
    InvalidArrayLength,
    DuplicateCallableParameter,
    UnknownProvenanceOrigin,
    DuplicateProvenanceOrigin,
    UnknownOpaqueBinding,
    DuplicateOpaqueBinding,
    OpaqueArgumentOrder,
    InvalidRequirement,
    RecursiveBinderRefinement,
}

impl TypeBindingRule {
    pub const ALL: [Self; 13] = [
        Self::UnknownTypeContextName,
        Self::InvalidTypeEntity,
        Self::InvalidTypeArguments,
        Self::InvalidSelfType,
        Self::InvalidArrayLength,
        Self::DuplicateCallableParameter,
        Self::UnknownProvenanceOrigin,
        Self::DuplicateProvenanceOrigin,
        Self::UnknownOpaqueBinding,
        Self::DuplicateOpaqueBinding,
        Self::OpaqueArgumentOrder,
        Self::InvalidRequirement,
        Self::RecursiveBinderRefinement,
    ];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnknownTypeContextName => "E0290",
            Self::InvalidTypeEntity => "E0291",
            Self::InvalidTypeArguments => "E0292",
            Self::InvalidSelfType => "E0293",
            Self::InvalidArrayLength => "E0294",
            Self::DuplicateCallableParameter => "E0295",
            Self::UnknownProvenanceOrigin => "E0296",
            Self::DuplicateProvenanceOrigin => "E0297",
            Self::UnknownOpaqueBinding => "E0298",
            Self::DuplicateOpaqueBinding => "E0299",
            Self::OpaqueArgumentOrder => "E0300",
            Self::InvalidRequirement => "E0301",
            Self::RecursiveBinderRefinement => "E0302",
        }
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::UnknownTypeContextName => "name is unknown in this type context",
            Self::InvalidTypeEntity => "name does not denote the required kind of type entity",
            Self::InvalidTypeArguments => "type application has invalid generic arguments",
            Self::InvalidSelfType => "Self is used outside a type-owning context",
            Self::InvalidArrayLength => "fixed-array length is outside the supported range",
            Self::DuplicateCallableParameter => "callable type repeats a parameter name",
            Self::UnknownProvenanceOrigin => "result provenance names no callable parameter",
            Self::DuplicateProvenanceOrigin => "result provenance repeats an origin",
            Self::UnknownOpaqueBinding => "opaque result names no associated type",
            Self::DuplicateOpaqueBinding => "opaque result repeats an associated binding",
            Self::OpaqueArgumentOrder => {
                "opaque positional type argument follows an associated binding"
            }
            Self::InvalidRequirement => "generic requirement has an invalid semantic shape",
            Self::RecursiveBinderRefinement => "binder refinement contains the binder it replaces",
        }
    }

    #[must_use]
    pub const fn help(self) -> &'static str {
        match self {
            Self::UnknownTypeContextName => "declare or import the name before using it here",
            Self::InvalidTypeEntity => "use a declaration valid in this type context",
            Self::InvalidTypeArguments => {
                "supply exactly the declared generic arguments, or remove arguments where forbidden"
            }
            Self::InvalidSelfType => "use Self only inside a type-owned declaration",
            Self::InvalidArrayLength => "use a fixed-array length representable as u64",
            Self::DuplicateCallableParameter => "give each named callable parameter a unique name",
            Self::UnknownProvenanceOrigin => "name one of this callable type's named parameters",
            Self::DuplicateProvenanceOrigin => "remove the repeated provenance origin",
            Self::UnknownOpaqueBinding => {
                "bind an associated type declared by the selected interface"
            }
            Self::DuplicateOpaqueBinding => "remove the repeated associated binding",
            Self::OpaqueArgumentOrder => {
                "place every positional type argument before associated bindings"
            }
            Self::InvalidRequirement => "rewrite the requirement using a valid requirement form",
            Self::RecursiveBinderRefinement => {
                "refine the binder to a type that does not contain that binder"
            }
        }
    }

    #[must_use]
    pub const fn related_message(self) -> Option<&'static str> {
        match self {
            Self::DuplicateCallableParameter => Some("the first parameter is named here"),
            Self::DuplicateProvenanceOrigin => Some("the first origin is named here"),
            Self::DuplicateOpaqueBinding => Some("the first associated binding is named here"),
            Self::UnknownTypeContextName
            | Self::InvalidTypeEntity
            | Self::InvalidTypeArguments
            | Self::InvalidSelfType
            | Self::InvalidArrayLength
            | Self::UnknownProvenanceOrigin
            | Self::UnknownOpaqueBinding
            | Self::OpaqueArgumentOrder
            | Self::InvalidRequirement
            | Self::RecursiveBinderRefinement => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::TypeBindingRule;

    #[test]
    fn type_binding_rule_codes_are_closed_and_unique() {
        let codes: BTreeSet<_> = TypeBindingRule::ALL
            .into_iter()
            .map(TypeBindingRule::code)
            .collect();
        assert_eq!(codes.len(), TypeBindingRule::ALL.len());
        assert!(codes.iter().all(|code| {
            code.len() == 5
                && code.starts_with('E')
                && code[1..].bytes().all(|byte| byte.is_ascii_digit())
        }));
    }
}

/// Exact syntax subjects retained when a type-binding rule is selected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeBindingViolation {
    rule: TypeBindingRule,
    primary: SyntaxOrigin,
    related: Option<SyntaxOrigin>,
}

impl TypeBindingViolation {
    #[must_use]
    pub const fn new(rule: TypeBindingRule, primary: SyntaxOrigin) -> Self {
        Self {
            rule,
            primary,
            related: None,
        }
    }

    #[must_use]
    pub const fn duplicate(
        rule: TypeBindingRule,
        first: SyntaxOrigin,
        second: SyntaxOrigin,
    ) -> Self {
        Self {
            rule,
            primary: second,
            related: Some(first),
        }
    }

    #[must_use]
    pub const fn rule(self) -> TypeBindingRule {
        self.rule
    }

    #[must_use]
    pub const fn primary(self) -> SyntaxOrigin {
        self.primary
    }

    #[must_use]
    pub const fn related(self) -> Option<SyntaxOrigin> {
        self.related
    }
}
