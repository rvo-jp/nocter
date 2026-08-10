//! Assembly of semantic surfaces owned by nominal types.

use super::{TypeSymbol, signatures::attach_behavior_declarations_to_symbol};
use crate::ast::AstFile;

pub(super) fn attach_nominal_type_surfaces(
    symbol: &mut TypeSymbol,
    ast: &AstFile,
    type_name: &str,
) {
    attach_behavior_declarations_to_symbol(symbol, ast, type_name);
    super::literals::attach_literal_definitions_to_symbol(symbol, ast, type_name);
    super::constructions::attach_construction_surfaces_to_symbol(symbol, ast, type_name);
    super::coercions::attach_coercions_to_symbol(symbol, ast, type_name);
}
