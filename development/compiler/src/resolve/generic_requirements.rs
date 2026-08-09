//! Resolved generic requirements carried by declaration signatures.
//!
//! Syntax retains authored bounds and spans. Resolver signatures classify each bound once so
//! type checking, specialization, and editor analysis do not repeatedly infer a requirement kind
//! from arbitrary type syntax. Nominal requirements remain subject to interface validation during
//! type checking; preserving that invalid form lets diagnostics point at the authored bound.

use crate::ast::TypeExpr;
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

    pub fn is_empty(&self) -> bool {
        self.requirements.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &GenericRequirement> {
        self.requirements.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut GenericRequirement> {
        self.requirements.iter_mut()
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
}
