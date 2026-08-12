//! Compile-unit semantic identity and, in later phases, typed semantic records.

mod body_declarations;
mod db;
mod ids;

pub(crate) use db::SemanticDb;
pub(crate) use ids::DefId;
