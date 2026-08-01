use super::*;

mod drops;
mod entry;
mod literals;
mod otherwise;
mod sources;
mod support;
mod terminal;

pub(super) use drops::lower_aggregate_drop_instructions_at_root_location;
pub(in crate::ir::lower) use drops::{
    lower_aggregate_drop_instructions_at_location, lower_array_prefix_drop_instructions,
    lower_struct_fields_drop_instructions,
};
pub(super) use entry::*;
pub(super) use literals::*;
pub(super) use otherwise::*;
pub(super) use sources::*;
pub(super) use support::*;
pub(super) use terminal::*;
