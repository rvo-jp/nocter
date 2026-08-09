//! Resolved generic requirements carried by declaration signatures.
//!
//! Syntax retains authored bounds and spans. Resolver signatures classify each bound once so
//! type checking, specialization, and editor analysis do not repeatedly infer a requirement kind
//! from arbitrary type syntax. Nominal requirements remain subject to interface validation during
//! type checking; preserving that invalid form lets diagnostics point at the authored bound.

use crate::ast::{CallableRequirementClause, GenericParam, TypeExpr};
use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenericRequirement {
    Nominal(TypeExpr),
    Callable(TypeExpr),
    Copy { span: ByteSpan },
}

impl GenericRequirement {
    pub fn from_type_expr(bound: TypeExpr) -> Self {
        if matches!(bound, TypeExpr::Callable(_)) {
            Self::Callable(bound)
        } else {
            Self::Nominal(bound)
        }
    }

    pub fn type_expr(&self) -> Option<&TypeExpr> {
        match self {
            Self::Nominal(bound) | Self::Callable(bound) => Some(bound),
            Self::Copy { .. } => None,
        }
    }

    pub fn type_expr_mut(&mut self) -> Option<&mut TypeExpr> {
        match self {
            Self::Nominal(bound) | Self::Callable(bound) => Some(bound),
            Self::Copy { .. } => None,
        }
    }

    pub fn span(&self) -> ByteSpan {
        match self {
            Self::Nominal(bound) | Self::Callable(bound) => bound.span(),
            Self::Copy { span } => *span,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GenericRequirements {
    requirements: Vec<GenericRequirement>,
}

impl GenericRequirements {
    pub fn from_bounds(bounds: &[TypeExpr]) -> Self {
        Self {
            requirements: bounds
                .iter()
                .cloned()
                .map(GenericRequirement::from_type_expr)
                .collect(),
        }
    }

    pub fn from_parameter(parameter: &GenericParam) -> Self {
        let mut requirements = parameter
            .copy_span
            .map(|span| vec![GenericRequirement::Copy { span }])
            .unwrap_or_default();
        requirements.extend(
            parameter
                .bounds
                .iter()
                .cloned()
                .map(GenericRequirement::from_type_expr),
        );
        Self { requirements }
    }

    pub fn extend_from_clause(
        &mut self,
        parameter: &str,
        clause: Option<&CallableRequirementClause>,
    ) {
        let Some(clause) = clause else {
            return;
        };
        for authored in &clause.requirements {
            if authored.name != parameter {
                continue;
            }
            if let Some(span) = authored.copy_span {
                self.requirements.push(GenericRequirement::Copy { span });
            }
            self.requirements.extend(
                authored
                    .bounds
                    .iter()
                    .cloned()
                    .map(GenericRequirement::from_type_expr),
            );
        }
    }

    pub fn is_empty(&self) -> bool {
        self.requirements.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &GenericRequirement> {
        self.requirements.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut GenericRequirement> {
        self.requirements.iter_mut()
    }

    pub fn push(&mut self, requirement: GenericRequirement) {
        self.requirements.push(requirement);
    }

    pub fn type_bounds(&self) -> impl Iterator<Item = &TypeExpr> {
        self.requirements
            .iter()
            .filter_map(GenericRequirement::type_expr)
    }

    pub fn callable_bounds(&self) -> impl Iterator<Item = &TypeExpr> {
        self.requirements
            .iter()
            .filter_map(|requirement| match requirement {
                GenericRequirement::Callable(bound) => Some(bound),
                GenericRequirement::Nominal(_) | GenericRequirement::Copy { .. } => None,
            })
    }

    pub fn has_copy(&self) -> bool {
        self.requirements
            .iter()
            .any(|requirement| matches!(requirement, GenericRequirement::Copy { .. }))
    }

    pub fn copy_span(&self) -> Option<ByteSpan> {
        self.requirements
            .iter()
            .find_map(|requirement| match requirement {
                GenericRequirement::Copy { span } => Some(*span),
                GenericRequirement::Nominal(_) | GenericRequirement::Callable(_) => None,
            })
    }
}
