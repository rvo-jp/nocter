use nocter_syntax::SyntaxOrigin;

/// Stable source-level rule selected while binding declaration-header type syntax.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TypeBindingRule {
    UnknownTypeContextName,
    InvalidTypeEntity,
    InvalidTypeArguments,
    InvalidSelfType,
    DuplicateCallableParameter,
    UnknownProvenanceOrigin,
    DuplicateProvenanceOrigin,
    UnknownOpaqueBinding,
    DuplicateOpaqueBinding,
    OpaqueArgumentOrder,
    InvalidRequirement,
    RecursiveBinderRefinement,
    DuplicateAssociatedRequirementBinding,
    DuplicateCopyRequirement,
    DuplicateInterfaceRequirement,
    DuplicateBinderRefinement,
}

impl TypeBindingRule {
    pub const ALL: [Self; 16] = [
        Self::UnknownTypeContextName,
        Self::InvalidTypeEntity,
        Self::InvalidTypeArguments,
        Self::InvalidSelfType,
        Self::DuplicateCallableParameter,
        Self::UnknownProvenanceOrigin,
        Self::DuplicateProvenanceOrigin,
        Self::UnknownOpaqueBinding,
        Self::DuplicateOpaqueBinding,
        Self::OpaqueArgumentOrder,
        Self::InvalidRequirement,
        Self::RecursiveBinderRefinement,
        Self::DuplicateAssociatedRequirementBinding,
        Self::DuplicateCopyRequirement,
        Self::DuplicateInterfaceRequirement,
        Self::DuplicateBinderRefinement,
    ];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnknownTypeContextName => "E0290",
            Self::InvalidTypeEntity => "E0291",
            Self::InvalidTypeArguments => "E0292",
            Self::InvalidSelfType => "E0293",
            Self::DuplicateCallableParameter => "E0295",
            Self::UnknownProvenanceOrigin => "E0296",
            Self::DuplicateProvenanceOrigin => "E0297",
            Self::UnknownOpaqueBinding => "E0298",
            Self::DuplicateOpaqueBinding => "E0299",
            Self::OpaqueArgumentOrder => "E0300",
            Self::InvalidRequirement => "E0301",
            Self::RecursiveBinderRefinement => "E0302",
            Self::DuplicateAssociatedRequirementBinding => "E0303",
            Self::DuplicateCopyRequirement => "E0304",
            Self::DuplicateInterfaceRequirement => "E0305",
            Self::DuplicateBinderRefinement => "E0306",
        }
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::UnknownTypeContextName => "name is unknown in this type context",
            Self::InvalidTypeEntity => "name does not denote the required kind of type entity",
            Self::InvalidTypeArguments => "type application has invalid generic arguments",
            Self::InvalidSelfType => "Self is used outside a type-owning context",
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
            Self::DuplicateAssociatedRequirementBinding => {
                "interface requirement repeats an associated binding"
            }
            Self::DuplicateCopyRequirement => "generic requirement repeats a copy predicate",
            Self::DuplicateInterfaceRequirement => {
                "generic requirement repeats an interface predicate"
            }
            Self::DuplicateBinderRefinement => "declaration pattern repeats a binder refinement",
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
            Self::DuplicateCallableParameter => "give each named callable parameter a unique name",
            Self::UnknownProvenanceOrigin => "name one of this callable type's named parameters",
            Self::DuplicateProvenanceOrigin => "remove the repeated provenance origin",
            Self::UnknownOpaqueBinding => {
                "bind an associated type declared by the selected interface"
            }
            Self::DuplicateOpaqueBinding | Self::DuplicateAssociatedRequirementBinding => {
                "remove the repeated associated binding"
            }
            Self::OpaqueArgumentOrder => {
                "place every positional type argument before associated bindings"
            }
            Self::InvalidRequirement => "rewrite the requirement using a valid requirement form",
            Self::RecursiveBinderRefinement => {
                "refine the binder to a type that does not contain that binder"
            }
            Self::DuplicateCopyRequirement => "remove the repeated copy predicate",
            Self::DuplicateInterfaceRequirement => {
                "merge or remove the repeated interface predicate"
            }
            Self::DuplicateBinderRefinement => {
                "keep exactly one concrete replacement for this pattern binder"
            }
        }
    }

    #[must_use]
    pub const fn related_message(self) -> Option<&'static str> {
        match self {
            Self::DuplicateCallableParameter => Some("the first parameter is named here"),
            Self::DuplicateProvenanceOrigin => Some("the first origin is named here"),
            Self::DuplicateOpaqueBinding => Some("the first associated binding is named here"),
            Self::DuplicateAssociatedRequirementBinding => {
                Some("the first associated binding is named here")
            }
            Self::DuplicateCopyRequirement => Some("the first copy predicate is named here"),
            Self::DuplicateInterfaceRequirement => {
                Some("the first interface predicate is named here")
            }
            Self::DuplicateBinderRefinement => Some("the first refinement is written here"),
            Self::UnknownTypeContextName
            | Self::InvalidTypeEntity
            | Self::InvalidTypeArguments
            | Self::InvalidSelfType
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
