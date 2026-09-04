use nocter_constant_evaluation::ConstantExpressionRule;
use nocter_diagnostics::DiagnosticCode;
use nocter_syntax::SyntaxOrigin;

/// Stable source-level rule selected while completing declaration definitions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DefinitionRule {
    InvalidCompileTimeValueType,
    NonConstantExpression,
    CompileTimeTypeMismatch,
    CompileTimeCycle,
    CompileTimeArithmeticFailure,
    UnknownResultProvenanceOrigin,
    DuplicateResultProvenanceOrigin,
    AmbiguousBodylessResultProvenance,
    UnknownAssociatedTypeBinding,
    DuplicateAssociatedTypeBinding,
    InvalidArgumentPackParameter,
}

impl DefinitionRule {
    pub const ALL: [Self; 11] = [
        Self::InvalidCompileTimeValueType,
        Self::NonConstantExpression,
        Self::CompileTimeTypeMismatch,
        Self::CompileTimeCycle,
        Self::CompileTimeArithmeticFailure,
        Self::UnknownResultProvenanceOrigin,
        Self::DuplicateResultProvenanceOrigin,
        Self::AmbiguousBodylessResultProvenance,
        Self::UnknownAssociatedTypeBinding,
        Self::DuplicateAssociatedTypeBinding,
        Self::InvalidArgumentPackParameter,
    ];

    #[must_use]
    pub const fn code(self) -> DiagnosticCode {
        match self {
            Self::InvalidCompileTimeValueType => DiagnosticCode::E0321,
            Self::NonConstantExpression => ConstantExpressionRule::NonConstantExpression.code(),
            Self::CompileTimeTypeMismatch => ConstantExpressionRule::TypeMismatch.code(),
            Self::CompileTimeCycle => ConstantExpressionRule::DependencyCycle.code(),
            Self::CompileTimeArithmeticFailure => ConstantExpressionRule::ArithmeticFailure.code(),
            Self::UnknownResultProvenanceOrigin => DiagnosticCode::E0315,
            Self::DuplicateResultProvenanceOrigin => DiagnosticCode::E0316,
            Self::AmbiguousBodylessResultProvenance => DiagnosticCode::E0317,
            Self::UnknownAssociatedTypeBinding => DiagnosticCode::E0318,
            Self::DuplicateAssociatedTypeBinding => DiagnosticCode::E0319,
            Self::InvalidArgumentPackParameter => DiagnosticCode::E0326,
        }
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::InvalidCompileTimeValueType => {
                "type is not supported by compile-time value semantics"
            }
            Self::NonConstantExpression => ConstantExpressionRule::NonConstantExpression.message(),
            Self::CompileTimeTypeMismatch => ConstantExpressionRule::TypeMismatch.message(),
            Self::CompileTimeCycle => ConstantExpressionRule::DependencyCycle.message(),
            Self::CompileTimeArithmeticFailure => {
                ConstantExpressionRule::ArithmeticFailure.message()
            }
            Self::UnknownResultProvenanceOrigin => {
                "result provenance names no receiver or parameter of this callable"
            }
            Self::DuplicateResultProvenanceOrigin => "result provenance repeats an origin",
            Self::AmbiguousBodylessResultProvenance => {
                "bodyless callable result provenance cannot be inferred uniquely"
            }
            Self::UnknownAssociatedTypeBinding => {
                "interface implementation binds an associated type not declared by its interface"
            }
            Self::DuplicateAssociatedTypeBinding => {
                "interface implementation repeats an associated type binding"
            }
            Self::InvalidArgumentPackParameter => {
                "an argument pack must be the one final parameter of a supported callable"
            }
        }
    }

    #[must_use]
    pub const fn help(self) -> &'static str {
        match self {
            Self::InvalidCompileTimeValueType => {
                "use bool, an integer type, char, readonly &str, or a fixed array of those types"
            }
            Self::NonConstantExpression => ConstantExpressionRule::NonConstantExpression.help(),
            Self::CompileTimeTypeMismatch => ConstantExpressionRule::TypeMismatch.help(),
            Self::CompileTimeCycle => ConstantExpressionRule::DependencyCycle.help(),
            Self::CompileTimeArithmeticFailure => ConstantExpressionRule::ArithmeticFailure.help(),
            Self::UnknownResultProvenanceOrigin => {
                "name self, a parameter of this callable, or static"
            }
            Self::DuplicateResultProvenanceOrigin => "remove the repeated provenance origin",
            Self::AmbiguousBodylessResultProvenance => {
                "add a from clause naming the inputs whose storage the result may retain"
            }
            Self::UnknownAssociatedTypeBinding => {
                "bind an associated type declared by the implemented interface"
            }
            Self::DuplicateAssociatedTypeBinding => "remove the repeated associated type binding",
            Self::InvalidArgumentPackParameter => {
                "keep at most one ... parameter in final position; sequence literals require exactly one"
            }
        }
    }

    #[must_use]
    pub const fn related_message(self) -> Option<&'static str> {
        match self {
            Self::DuplicateResultProvenanceOrigin => Some("the first origin is named here"),
            Self::DuplicateAssociatedTypeBinding => {
                Some("the first associated type binding is declared here")
            }
            Self::UnknownResultProvenanceOrigin
            | Self::InvalidCompileTimeValueType
            | Self::NonConstantExpression
            | Self::CompileTimeTypeMismatch
            | Self::CompileTimeCycle
            | Self::CompileTimeArithmeticFailure
            | Self::AmbiguousBodylessResultProvenance
            | Self::UnknownAssociatedTypeBinding
            | Self::InvalidArgumentPackParameter => None,
        }
    }
}

/// Exact syntax subjects retained when a declaration-definition rule is selected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefinitionViolation {
    rule: DefinitionRule,
    primary: SyntaxOrigin,
    related: Option<SyntaxOrigin>,
}

impl DefinitionViolation {
    #[must_use]
    pub const fn new(rule: DefinitionRule, primary: SyntaxOrigin) -> Self {
        Self {
            rule,
            primary,
            related: None,
        }
    }

    #[must_use]
    pub const fn duplicate(
        rule: DefinitionRule,
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
    pub const fn rule(self) -> DefinitionRule {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::DefinitionRule;

    #[test]
    fn definition_rule_codes_are_closed_and_unique() {
        let codes: BTreeSet<_> = DefinitionRule::ALL
            .into_iter()
            .map(DefinitionRule::code)
            .collect();
        assert_eq!(codes.len(), DefinitionRule::ALL.len());
        assert!(codes.iter().all(|code| {
            let code = code.as_str();
            code.len() == 5
                && code.starts_with('E')
                && code[1..].bytes().all(|byte| byte.is_ascii_digit())
        }));
    }
}
