//! Grammar-selected value-access analysis through the existing complete source coordinator.
use std::sync::Arc;

use nocter_checking::value_access::{
    SourceFailureReport,
    accepted::{ModuleSourceUnit, Unit},
};
use nocter_declaration_lowering::access_declarations::ModuleQuery;
use nocter_discovery::DiscoveredUnit;

/// Exact checked v2 source product. Only the common source coordinator can create its parts.
/// Headers support guarded current binding; the Unit retains whole-source checking evidence.
#[derive(Debug)]
pub struct AcceptedValueAccess {
    headers: ModuleQuery,
    unit: Arc<Unit>,
}

impl AcceptedValueAccess {
    #[must_use]
    pub const fn headers(&self) -> &ModuleQuery {
        &self.headers
    }

    #[must_use]
    pub fn unit(&self) -> &Unit {
        &self.unit
    }

    #[must_use]
    pub fn unit_handle(&self) -> Arc<Unit> {
        Arc::clone(&self.unit)
    }
}

/// Source outcome for the selected value-access grammar, not released-program conversion.
/// Incomplete syntax retains discovery's exact diagnostics and supplies no partial checked Unit.
#[derive(Debug)]
pub enum ValueAccessAnalysis {
    Accepted(AcceptedValueAccess),
    Rejected(std::sync::Arc<SourceFailureReport>),
    IncompleteSyntax,
}

impl ValueAccessAnalysis {
    #[must_use]
    pub const fn accepted(&self) -> Option<&AcceptedValueAccess> {
        match self {
            Self::Accepted(accepted) => Some(accepted),
            Self::Rejected(_) | Self::IncompleteSyntax => None,
        }
    }
}

pub(super) fn analyze(
    unit: &DiscoveredUnit,
) -> Result<ValueAccessAnalysis, super::SemanticQueryFailure> {
    if unit.has_syntax_errors() {
        return Ok(ValueAccessAnalysis::IncompleteSyntax);
    }
    let input = unit.compile_input()?;
    let result = ModuleSourceUnit::prepare(input).and_then(|source| {
        let headers = source.declarations().query()?;
        Ok(AcceptedValueAccess {
            headers,
            unit: Arc::new(source.into_unit()),
        })
    });
    Ok(match result {
        Ok(accepted) => ValueAccessAnalysis::Accepted(accepted),
        Err(failure) => {
            ValueAccessAnalysis::Rejected(std::sync::Arc::new(failure.project(input).map_err(
                |failure| super::SemanticQueryFailure::ValueAccessProjection(Box::new(failure)),
            )?))
        }
    })
}
