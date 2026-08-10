//! Package-authorized interface conformances for compiler built-in types.
//!
//! Built-in types remain syntax identities outside the nominal symbol table.
//! The selected standard-library package may nevertheless attach ordinary,
//! source-backed conformance records to their resolver surfaces.

use super::Resolver;
use super::builtin_instances::{BuiltinTypeSurface, empty_builtin_type_symbol};
use super::conformance::interface_conformance;
use crate::ast::Item;
use crate::builtin_types::BuiltinTypeOwner;

impl Resolver<'_> {
    pub(super) fn collect_builtin_conformance_surfaces(&mut self) {
        let mut conformances = self
            .module_index
            .asts()
            .filter_map(|ast| {
                self.output
                    .source_scopes
                    .standard_library_module_path(ast.span.source)
                    .map(|module| (ast, module))
            })
            .flat_map(|(ast, module)| {
                ast.items.iter().filter_map(move |item| {
                    let Item::Conformance(conformance) = item else {
                        return None;
                    };
                    BuiltinTypeOwner::from_conformance_target(&conformance.target_ty)
                        .map(|owner| (ast, module.clone(), owner, conformance.clone()))
                })
            })
            .collect::<Vec<_>>();
        conformances.sort_by_key(|(_, _, owner, conformance)| {
            (
                owner.canonical_name(),
                conformance.span.source.raw(),
                conformance.span.start,
            )
        });

        for (ast, module, owner, conformance) in conformances {
            let mut resolved = interface_conformance(&conformance);
            self.prepare_external_conformance(ast, &module, &mut resolved);
            let surface = self
                .output
                .builtin_type_surfaces
                .entry(owner)
                .or_insert_with(|| BuiltinTypeSurface {
                    owner,
                    declaration_span: conformance.target_ty.span(),
                    symbol: empty_builtin_type_symbol(owner),
                });
            surface.symbol.interface_conformances.push(resolved);
        }
    }
}

#[cfg(test)]
pub(crate) fn attach_test_builtin_conformances(
    output: &mut super::ResolveOutput,
    ast: &crate::ast::AstFile,
) {
    for conformance in ast.items.iter().filter_map(|item| {
        let Item::Conformance(conformance) = item else {
            return None;
        };
        BuiltinTypeOwner::from_conformance_target(&conformance.target_ty)
            .map(|owner| (owner, conformance))
    }) {
        let (owner, conformance) = conformance;
        let surface = output
            .builtin_type_surfaces
            .entry(owner)
            .or_insert_with(|| BuiltinTypeSurface {
                owner,
                declaration_span: conformance.target_ty.span(),
                symbol: empty_builtin_type_symbol(owner),
            });
        surface
            .symbol
            .interface_conformances
            .push(interface_conformance(conformance));
    }
}
