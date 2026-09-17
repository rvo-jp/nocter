use nocter_diagnostics::DiagnosticCode;
use nocter_syntax::{NodeId, SyntaxOrigin, SyntaxToken};

/// Stable source-level rule for generic binder declarations.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GenericRule {
    ReservedBinder,
    DuplicateBinder,
    ShadowingBinder,
    UnsupportedConstantParameterType,
}

impl GenericRule {
    #[must_use]
    pub const fn code(self) -> DiagnosticCode {
        match self {
            Self::ReservedBinder => DiagnosticCode::E0280,
            Self::DuplicateBinder => DiagnosticCode::E0281,
            Self::ShadowingBinder => DiagnosticCode::E0282,
            Self::UnsupportedConstantParameterType => DiagnosticCode::E0283,
        }
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::ReservedBinder => "generic binder uses a reserved type name",
            Self::DuplicateBinder => "generic binder is declared more than once",
            Self::ShadowingBinder => "generic binder shadows an inherited binder",
            Self::UnsupportedConstantParameterType => {
                "constant generic parameter type is not supported"
            }
        }
    }

    #[must_use]
    pub const fn help(self) -> &'static str {
        match self {
            Self::ReservedBinder => "choose a non-reserved binder name",
            Self::DuplicateBinder => "remove or rename one of the duplicate binders",
            Self::ShadowingBinder => "rename the nested binder or reuse the inherited binder",
            Self::UnsupportedConstantParameterType => "use `usize` as the parameter type",
        }
    }

    #[must_use]
    pub const fn related_message(self) -> Option<&'static str> {
        match self {
            Self::DuplicateBinder => Some("the first binder is declared here"),
            Self::ShadowingBinder => Some("the inherited binder is declared here"),
            Self::ReservedBinder | Self::UnsupportedConstantParameterType => None,
        }
    }
}

/// Exact syntax subjects for one authored generic-binder violation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenericViolation {
    rule: GenericRule,
    primary: SyntaxOrigin,
    related: Option<SyntaxOrigin>,
}

impl GenericViolation {
    #[must_use]
    pub const fn reserved_binder(token: SyntaxToken) -> Self {
        Self {
            rule: GenericRule::ReservedBinder,
            primary: SyntaxOrigin::Token(token),
            related: None,
        }
    }

    #[must_use]
    pub const fn duplicate_binder(first: SyntaxToken, second: SyntaxToken) -> Self {
        Self {
            rule: GenericRule::DuplicateBinder,
            primary: SyntaxOrigin::Token(second),
            related: Some(SyntaxOrigin::Token(first)),
        }
    }

    #[must_use]
    pub const fn shadowing_binder(inherited: SyntaxToken, nested: SyntaxToken) -> Self {
        Self {
            rule: GenericRule::ShadowingBinder,
            primary: SyntaxOrigin::Token(nested),
            related: Some(SyntaxOrigin::Token(inherited)),
        }
    }

    #[must_use]
    pub const fn unsupported_constant_parameter_type(parameter: NodeId) -> Self {
        Self {
            rule: GenericRule::UnsupportedConstantParameterType,
            primary: SyntaxOrigin::Node(parameter),
            related: None,
        }
    }

    #[must_use]
    pub const fn rule(self) -> GenericRule {
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
