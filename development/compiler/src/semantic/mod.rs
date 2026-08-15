//! Compile-unit semantic identity and, in later phases, typed semantic records.

mod body_declarations;
mod callable_kinds;
mod db;
mod ids;
mod type_identity;

pub(crate) use callable_kinds::OperatorCallableKind;
pub(crate) use db::{DefinitionKind, SemanticDb};
pub(crate) use ids::{
    BodyId, DefId, ExprId, MonoItemId, OpaqueTypeId, SemanticSiteId, StmtId, TyId,
};
pub(crate) use type_identity::TypeIdentity;
