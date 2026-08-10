//! Package-coherent conformances declared by the selected standard library.
//!
//! Every source in a compile unit receives its own qualified type-symbol view.
//! A conformance declared outside the target type's module must therefore be
//! attached to each matching view once the standard source has been loaded.
//! The selected standard package is coherent as a whole; project conformances
//! retain their ordinary import and ownership rules.

use super::conformance::interface_conformance;
use super::signatures::declaration_target_type_name;
use super::{Resolver, SymbolKind};
use crate::ast::Item;
use crate::builtin_types::BuiltinTypeOwner;

impl Resolver<'_> {
    pub(super) fn collect_standard_nominal_conformance_surfaces(&mut self) {
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
                        .is_none()
                        .then(|| (ast, module.clone(), conformance.clone()))
                })
            })
            .collect::<Vec<_>>();
        conformances.sort_by_key(|(_, _, conformance)| {
            (conformance.span.source.raw(), conformance.span.start)
        });

        for (ast, module, conformance) in conformances {
            let mut resolved = interface_conformance(&conformance);
            self.prepare_external_conformance(ast, &module, &mut resolved);
            let Some(target_name) = declaration_target_type_name(&resolved.target_ty) else {
                continue;
            };
            for symbol in &mut self.output.symbols.symbols {
                let SymbolKind::Type(target) = &mut symbol.kind else {
                    continue;
                };
                if target.canonical_name != target_name
                    || target
                        .interface_conformances
                        .iter()
                        .any(|existing| existing.declaration_span == resolved.declaration_span)
                {
                    continue;
                }
                target.interface_conformances.push(resolved.clone());
            }
        }
    }
}
