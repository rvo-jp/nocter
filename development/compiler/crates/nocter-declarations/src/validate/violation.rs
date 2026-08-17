use std::fmt;

use nocter_model::DeclarationSiteId;

use super::ProgramIntegrityError;

/// One source-language declaration rule enforced after graph integrity is established.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DeclarationRule {
    EmptyEnum,
    InvalidInherentAttachment,
    DuplicateConstruction,
    InvalidConstructionResult,
    InvalidDropTarget,
    DuplicateDrop,
    CopyDrop,
    PayloadlessEnumDrop,
    PrimitiveAuthority,
    BuiltinConformanceAuthority,
    InvalidConformanceTarget,
    IncompleteAssociatedTypes,
    InvalidOpaqueResult,
    InvalidLiteralSignature,
}

impl DeclarationRule {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::EmptyEnum => "E0200",
            Self::InvalidInherentAttachment => "E0201",
            Self::DuplicateConstruction => "E0202",
            Self::InvalidConstructionResult => "E0203",
            Self::InvalidDropTarget => "E0204",
            Self::DuplicateDrop => "E0205",
            Self::CopyDrop => "E0206",
            Self::PayloadlessEnumDrop => "E0207",
            Self::PrimitiveAuthority => "E0208",
            Self::BuiltinConformanceAuthority => "E0209",
            Self::InvalidConformanceTarget => "E0210",
            Self::IncompleteAssociatedTypes => "E0211",
            Self::InvalidOpaqueResult => "E0212",
            Self::InvalidLiteralSignature => "E0213",
        }
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::EmptyEnum => "enum must declare at least one variant",
            Self::InvalidInherentAttachment => {
                "construction or instance is outside its type's ownership boundary"
            }
            Self::DuplicateConstruction => "type family has more than one construction declaration",
            Self::InvalidConstructionResult => {
                "construction member does not produce its owning type"
            }
            Self::InvalidDropTarget => "drop declaration target cannot own a drop body",
            Self::DuplicateDrop => "type family has more than one drop declaration",
            Self::CopyDrop => "copy struct cannot declare a drop body",
            Self::PayloadlessEnumDrop => "payloadless enum cannot declare a drop body",
            Self::PrimitiveAuthority => {
                "primitive declaration is outside the selected standard package"
            }
            Self::BuiltinConformanceAuthority => {
                "built-in conformance is outside the selected standard package"
            }
            Self::InvalidConformanceTarget => {
                "conformance target is not a nominal or compiler-owned built-in type"
            }
            Self::IncompleteAssociatedTypes => "conformance does not bind every associated type",
            Self::InvalidOpaqueResult => {
                "opaque result requires a supported callable with a source body"
            }
            Self::InvalidLiteralSignature => {
                "literal member does not match its language-defined shape"
            }
        }
    }

    #[must_use]
    pub const fn related_message(self) -> Option<&'static str> {
        match self {
            Self::DuplicateConstruction | Self::DuplicateDrop => {
                Some("previous declaration is here")
            }
            Self::InvalidConstructionResult | Self::InvalidLiteralSignature => {
                Some("owning construction is declared here")
            }
            Self::InvalidInherentAttachment | Self::InvalidDropTarget => {
                Some("target type is declared here")
            }
            Self::CopyDrop | Self::PayloadlessEnumDrop => Some("target type is declared here"),
            Self::IncompleteAssociatedTypes => Some("interface is declared here"),
            Self::EmptyEnum
            | Self::PrimitiveAuthority
            | Self::BuiltinConformanceAuthority
            | Self::InvalidConformanceTarget
            | Self::InvalidOpaqueResult => None,
        }
    }

    #[must_use]
    pub const fn help(self) -> &'static str {
        match self {
            Self::EmptyEnum => "add at least one variant or use a struct",
            Self::InvalidInherentAttachment => "declare the surface in the target type's module",
            Self::DuplicateConstruction => {
                "combine the type's construction entries into one construct declaration"
            }
            Self::InvalidConstructionResult => {
                "return the constructed type directly or through supported outcome layers"
            }
            Self::InvalidDropTarget => {
                "declare drop beside a move-only nominal type owned by this module"
            }
            Self::DuplicateDrop => "keep one drop declaration for the type family",
            Self::CopyDrop => "remove the drop declaration from the copy struct",
            Self::PayloadlessEnumDrop => "remove the drop declaration from the payloadless enum",
            Self::PrimitiveAuthority => {
                "declare primitives only in the selected toolchain standard package"
            }
            Self::BuiltinConformanceAuthority => {
                "declare built-in conformances in the selected standard package"
            }
            Self::InvalidConformanceTarget => {
                "conform a nominal type or an authorized built-in type"
            }
            Self::IncompleteAssociatedTypes => {
                "bind every associated type declared by the interface"
            }
            Self::InvalidOpaqueResult => {
                "use the opaque result on a supported callable with a source body"
            }
            Self::InvalidLiteralSignature => {
                "declare a public sequence element pack or one public readonly str parameter"
            }
        }
    }
}

/// A declaration-rule violation with exact semantic diagnostic subjects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclarationViolation {
    rule: DeclarationRule,
    primary: DeclarationSiteId,
    related: Option<DeclarationSiteId>,
}

impl DeclarationViolation {
    #[must_use]
    pub const fn new(rule: DeclarationRule, primary: DeclarationSiteId) -> Self {
        Self {
            rule,
            primary,
            related: None,
        }
    }

    #[must_use]
    pub const fn with_related(
        rule: DeclarationRule,
        primary: DeclarationSiteId,
        related: DeclarationSiteId,
    ) -> Self {
        Self {
            rule,
            primary,
            related: Some(related),
        }
    }

    #[must_use]
    pub const fn rule(self) -> DeclarationRule {
        self.rule
    }

    #[must_use]
    pub const fn primary(self) -> DeclarationSiteId {
        self.primary
    }

    #[must_use]
    pub const fn related(self) -> Option<DeclarationSiteId> {
        self.related
    }
}

impl fmt::Display for DeclarationViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.rule.code(), self.rule.message())
    }
}

impl std::error::Error for DeclarationViolation {}

/// Distinguishes an invalid authored declaration from a malformed compiler-produced graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgramValidationError {
    Declaration(DeclarationViolation),
    Integrity(ProgramIntegrityError),
}

impl fmt::Display for ProgramValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declaration(error) => error.fmt(formatter),
            Self::Integrity(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ProgramValidationError {}

impl From<DeclarationViolation> for ProgramValidationError {
    fn from(error: DeclarationViolation) -> Self {
        Self::Declaration(error)
    }
}

impl From<ProgramIntegrityError> for ProgramValidationError {
    fn from(error: ProgramIntegrityError) -> Self {
        Self::Integrity(error)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::DeclarationRule;

    #[test]
    fn declaration_rule_codes_are_closed_unique_error_codes() {
        let rules = [
            DeclarationRule::EmptyEnum,
            DeclarationRule::InvalidInherentAttachment,
            DeclarationRule::DuplicateConstruction,
            DeclarationRule::InvalidConstructionResult,
            DeclarationRule::InvalidDropTarget,
            DeclarationRule::DuplicateDrop,
            DeclarationRule::CopyDrop,
            DeclarationRule::PayloadlessEnumDrop,
            DeclarationRule::PrimitiveAuthority,
            DeclarationRule::BuiltinConformanceAuthority,
            DeclarationRule::InvalidConformanceTarget,
            DeclarationRule::IncompleteAssociatedTypes,
            DeclarationRule::InvalidOpaqueResult,
        ];
        let codes: HashSet<_> = rules.iter().map(|rule| rule.code()).collect();

        assert_eq!(codes.len(), rules.len());
        assert!(codes.iter().all(|code| {
            code.len() == 5
                && code.starts_with('E')
                && code[1..].bytes().all(|byte| byte.is_ascii_digit())
        }));
    }
}
