//! Compiler-owned source edit planning shared by editor protocols.

mod imports;
mod interface_members;
mod outcomes;

pub(crate) use imports::plan_top_level_import;
pub(crate) use interface_members::plan_missing_interface_members;
pub(crate) use outcomes::{OutcomeContractKind, plan_outcome_contract};
