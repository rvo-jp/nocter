use super::*;

mod drops;
mod entry;
mod literals;
mod otherwise;
mod sources;
mod support;
mod terminal;

pub(in crate::ir::lower) use drops::lower_aggregate_drop_instructions_at_location;
pub(super) use drops::lower_aggregate_drop_instructions_at_root_location;
pub(super) use entry::*;
pub(super) use literals::*;
pub(super) use otherwise::*;
pub(super) use sources::*;
pub(super) use support::*;
pub(super) use terminal::*;
