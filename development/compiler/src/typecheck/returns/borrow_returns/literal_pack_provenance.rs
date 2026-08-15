//! Provenance carried from a typed-literal element pack into its loop binding.
//!
//! A sequence literal capture is an input even though it is not an ordinary ABI
//! parameter. Keeping that distinction behind this translation lets callable
//! contracts and body inference use the same declaration-identity `InputId`.

use super::*;
use crate::ast::LiteralPackForStmt;

pub(in crate::typecheck::returns) fn define_literal_pack_item_provenance(
    statement: &LiteralPackForStmt,
    environment: &TypeEnvironment,
    resolved: &ResolveOutput,
    borrow_provenance: &mut ProvenanceEnvironment,
) {
    let item_type = environment
        .literal_pack_element(&statement.pack_name)
        .cloned()
        .unwrap_or(Type::Unknown);
    let pack = resolved
        .local_symbol_reference_at_offset(statement.pack_span.start)
        .map(|(_, symbol)| symbol)
        .filter(|symbol| symbol.kind == LocalSymbolKind::LiteralCapture);
    let contains_storage = type_may_carry_result_provenance(&item_type, resolved);
    let provenance = if contains_storage {
        pack.map(|symbol| ValueProvenance::input(InputId::resolved_at(resolved, symbol.name_span)))
    } else {
        None
    };
    borrow_provenance.define_binding_at(
        resolved,
        statement.name_span,
        contains_storage,
        provenance,
    );
}
