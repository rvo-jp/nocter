use std::fmt;

use nocter_declarations::{CallableContractViolation, DeclarationGraph};
use nocter_diagnostics::{DiagnosticCode, SourceDiagnostic};
use nocter_source_index::{DiagnosticOrigins, SemanticEntity};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CallableContractRule {
    DeferredNoAllocation,
    DeferredBlocking,
    DeferredCompileTime,
    PrimitiveCompileTime,
}

impl CallableContractRule {
    pub const ALL: &'static [Self] = &[
        Self::DeferredNoAllocation,
        Self::DeferredBlocking,
        Self::DeferredCompileTime,
        Self::PrimitiveCompileTime,
    ];

    #[must_use]
    pub const fn code(self) -> DiagnosticCode {
        match self {
            Self::DeferredNoAllocation => DiagnosticCode::E0374,
            Self::DeferredBlocking => DiagnosticCode::E0418,
            Self::DeferredCompileTime => DiagnosticCode::E0422,
            Self::PrimitiveCompileTime => DiagnosticCode::E0423,
        }
    }

    fn diagnostic(self, primary: nocter_source_index::SourceOrigin) -> SourceDiagnostic {
        let (message, help) = match self {
            Self::DeferredNoAllocation => (
                "an asynchronous producer cannot promise `noalloc`",
                "remove `noalloc`; creating the owning computation requires storage",
            ),
            Self::DeferredBlocking => (
                "an asynchronous producer cannot admit synchronous blocking",
                "remove `blocking`; driving every `future T` must remain nonblocking",
            ),
            Self::DeferredCompileTime => (
                "an asynchronous producer cannot be evaluated at compile time",
                "remove `const`; compile-time evaluation cannot construct or drive a future",
            ),
            Self::PrimitiveCompileTime => (
                "a primitive function has no compile-time implementation",
                "remove `const`; compile-time callables require a checked source body",
            ),
        };
        SourceDiagnostic::new(self.code(), message, primary, [], Some(help))
    }
}

impl From<CallableContractViolation> for CallableContractRule {
    fn from(violation: CallableContractViolation) -> Self {
        match violation {
            CallableContractViolation::DeferredNoAllocation => Self::DeferredNoAllocation,
            CallableContractViolation::DeferredBlocking => Self::DeferredBlocking,
            CallableContractViolation::DeferredCompileTime => Self::DeferredCompileTime,
            CallableContractViolation::PrimitiveCompileTime => Self::PrimitiveCompileTime,
        }
    }
}

#[derive(Clone, Debug)]
pub enum CallableContractValidityError {
    Rule(SourceDiagnostic),
    MissingSource(SemanticEntity),
}

impl CallableContractValidityError {
    #[must_use]
    pub const fn source_diagnostic(&self) -> Option<&SourceDiagnostic> {
        match self {
            Self::Rule(diagnostic) => Some(diagnostic),
            Self::MissingSource(_) => None,
        }
    }
}

impl fmt::Display for CallableContractValidityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule(diagnostic) => {
                write!(formatter, "{}: {}", diagnostic.code(), diagnostic.message())
            }
            Self::MissingSource(entity) => {
                write!(formatter, "missing source for callable contract {entity:?}")
            }
        }
    }
}

impl std::error::Error for CallableContractValidityError {}

/// Validates the declaration-owned callable contract policy and projects failures to source.
///
/// # Errors
///
/// Returns a source-backed rule failure or an integrity failure when a declaration has no source
/// projection.
pub fn validate_callable_contracts(
    graph: &DeclarationGraph,
    origins: DiagnosticOrigins<'_>,
) -> Result<(), CallableContractValidityError> {
    for (id, callable) in graph.declarations().callables().iter() {
        let Some(violation) = callable.contract_violation() else {
            continue;
        };
        let entity = SemanticEntity::Callable(id);
        let origin = origins
            .declaration(entity)
            .ok_or(CallableContractValidityError::MissingSource(entity))?;
        return Err(CallableContractValidityError::Rule(
            CallableContractRule::from(violation).diagnostic(origin),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use nocter_declaration_lowering::lower_compile_unit_declarations;

    use super::{CallableContractRule, validate_callable_contracts};
    use crate::test_support::Fixture;

    #[test]
    fn diagnostic_codes_are_closed_and_unique() {
        let codes = CallableContractRule::ALL
            .iter()
            .copied()
            .map(CallableContractRule::code)
            .collect::<HashSet<_>>();
        assert_eq!(codes.len(), CallableContractRule::ALL.len());
    }

    #[test]
    fn incompatible_callable_contracts_have_declaration_diagnostics() {
        for (source, expected) in [
            (
                "noalloc async func impossible(): i32 { return 1 }\n",
                "E0374",
            ),
            (
                "blocking async func impossible(): i32 { return 1 }\n",
                "E0418",
            ),
            ("const async func impossible(): i32 { return 1 }\n", "E0422"),
        ] {
            let fixture = Fixture::new(source);
            let input = fixture.input(false);
            let lowered = lower_compile_unit_declarations(&input).unwrap();
            let (program, _frontend_bindings, source_index) = lowered.into_checking_parts();
            let (graph, _types, _values, _admission) = program.into_parts();
            let error =
                validate_callable_contracts(&graph, source_index.diagnostic_origins()).unwrap_err();

            assert_eq!(error.source_diagnostic().unwrap().code(), expected);
        }
    }
}
