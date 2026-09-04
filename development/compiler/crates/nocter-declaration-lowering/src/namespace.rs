use nocter_diagnostics::DiagnosticCode;
use nocter_syntax::SyntaxOrigin;

/// Stable source-level rule for names and authored visibility boundaries.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NamespaceRule {
    ReservedName,
    NameCollision,
    VisibilityAbovePackageRoot,
    InvalidCompileTimeValueName,
}

impl NamespaceRule {
    #[must_use]
    pub const fn code(self) -> DiagnosticCode {
        match self {
            Self::ReservedName => DiagnosticCode::E0240,
            Self::NameCollision => DiagnosticCode::E0241,
            Self::VisibilityAbovePackageRoot => DiagnosticCode::E0242,
            Self::InvalidCompileTimeValueName => DiagnosticCode::E0243,
        }
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::ReservedName => "name is reserved and cannot be introduced into this namespace",
            Self::NameCollision => "name is introduced more than once in the same namespace",
            Self::VisibilityAbovePackageRoot => "visibility boundary moves above the package root",
            Self::InvalidCompileTimeValueName => "compile-time value name is not UPPER_SNAKE_CASE",
        }
    }

    #[must_use]
    pub const fn help(self) -> &'static str {
        match self {
            Self::ReservedName => "choose a different declaration name or import alias",
            Self::NameCollision => "rename or remove one of the declarations or imports",
            Self::VisibilityAbovePackageRoot => {
                "use fewer ../ components or use pub(/) for package visibility"
            }
            Self::InvalidCompileTimeValueName => {
                "rename the const or static with uppercase ASCII letters, digits, and single underscores"
            }
        }
    }

    #[must_use]
    pub const fn related_message(self) -> Option<&'static str> {
        match self {
            Self::NameCollision => Some("the first introduction of this name is here"),
            Self::ReservedName
            | Self::VisibilityAbovePackageRoot
            | Self::InvalidCompileTimeValueName => None,
        }
    }
}

/// Exact syntax subjects for one authored namespace-rule violation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NamespaceViolation {
    rule: NamespaceRule,
    primary: SyntaxOrigin,
    related: Option<SyntaxOrigin>,
}

impl NamespaceViolation {
    #[must_use]
    pub const fn reserved_name(name: SyntaxOrigin) -> Self {
        Self {
            rule: NamespaceRule::ReservedName,
            primary: name,
            related: None,
        }
    }

    #[must_use]
    pub const fn name_collision(first: SyntaxOrigin, second: SyntaxOrigin) -> Self {
        Self {
            rule: NamespaceRule::NameCollision,
            primary: second,
            related: Some(first),
        }
    }

    #[must_use]
    pub const fn visibility_above_package_root(visibility: SyntaxOrigin) -> Self {
        Self {
            rule: NamespaceRule::VisibilityAbovePackageRoot,
            primary: visibility,
            related: None,
        }
    }

    #[must_use]
    pub const fn invalid_compile_time_value_name(name: SyntaxOrigin) -> Self {
        Self {
            rule: NamespaceRule::InvalidCompileTimeValueName,
            primary: name,
            related: None,
        }
    }

    #[must_use]
    pub const fn rule(self) -> NamespaceRule {
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
