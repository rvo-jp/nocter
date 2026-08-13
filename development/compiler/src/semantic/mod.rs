//! Compile-unit semantic identity and, in later phases, typed semantic records.

mod body_declarations;
mod callable_kinds;
mod db;
mod ids;

pub(crate) use callable_kinds::OperatorCallableKind;
pub(crate) use db::{DefinitionKind, SemanticDb};
pub(crate) use ids::{BodyId, DefId, ExprId, OpaqueTypeId, TyId};
