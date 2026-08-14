//! Concrete destructor dependencies reachable through aggregate storage.

use super::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{TypeExpr, canonical_type_expr, substitute_type_expr_parameters};
use crate::typecheck::{DropTypeSpecialization, drop_type_specialization_from_self_ty};
use std::collections::{HashMap, HashSet};

pub(super) fn concrete_drop_dependencies(
    analysis: &CompileUnitAnalysis,
    fallback_file: &FileAnalysis,
    ty: &TypeExpr,
) -> Vec<DropTypeSpecialization> {
    let mut dependencies = Vec::new();
    collect_drop_dependencies(
        analysis,
        fallback_file,
        ty,
        &mut HashSet::new(),
        &mut dependencies,
    );
    dependencies
}

fn collect_drop_dependencies(
    analysis: &CompileUnitAnalysis,
    fallback_file: &FileAnalysis,
    ty: &TypeExpr,
    visiting: &mut HashSet<String>,
    dependencies: &mut Vec<DropTypeSpecialization>,
) {
    let key = format!("{:?}:{}", ty.span().source, canonical_type_expr(ty));
    if !visiting.insert(key.clone()) {
        return;
    }

    let source_file = analysis
        .file_by_source(ty.span().source)
        .unwrap_or(fallback_file);
    let file = resolver_file_for_type(analysis, source_file, ty).unwrap_or(source_file);
    let resolved = &file.resolved;
    if let Some(specialization) = std::iter::once(file)
        .chain(
            analysis
                .files
                .iter()
                .filter(|candidate| candidate.ast.span.source != file.ast.span.source),
        )
        .find_map(|candidate| {
            drop_type_specialization_from_self_ty(ty, &candidate.resolved, HashSet::new())
        })
    {
        dependencies.push(specialization);
    }

    match ty {
        TypeExpr::Callable(_) => {}
        TypeExpr::Opaque(opaque) => {
            if let Some(witness) = &opaque.witness {
                collect_drop_dependencies(analysis, file, witness, visiting, dependencies)
            }
        }
        TypeExpr::Array(array) => {
            collect_drop_dependencies(analysis, file, &array.element, visiting, dependencies)
        }
        TypeExpr::Optional(optional) => {
            collect_drop_dependencies(analysis, file, &optional.inner, visiting, dependencies)
        }
        TypeExpr::Fallible(fallible) => {
            collect_drop_dependencies(analysis, file, &fallible.success, visiting, dependencies)
        }
        TypeExpr::Reference(reference) => {
            let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
                visiting.remove(&key);
                return;
            };
            if let Some(target) = &symbol.alias_target {
                collect_drop_dependencies(analysis, file, target, visiting, dependencies);
            }
            for field in &symbol.fields {
                collect_drop_dependencies(analysis, file, &field.ty, visiting, dependencies);
            }
            for variant in &symbol.variants {
                for payload in &variant.payload {
                    collect_drop_dependencies(analysis, file, &payload.ty, visiting, dependencies);
                }
            }
        }
        TypeExpr::Generic(generic) => {
            let Some(symbol) = resolved.type_symbol_by_reference_name(&generic.name) else {
                visiting.remove(&key);
                return;
            };
            if symbol.generic_parameters.len() != generic.arguments.len() {
                visiting.remove(&key);
                return;
            }
            let substitutions = symbol
                .generic_parameters
                .iter()
                .cloned()
                .zip(generic.arguments.iter().cloned())
                .collect::<HashMap<_, _>>();
            if let Some(target) = &symbol.alias_target {
                let target = substitute_type_expr_parameters(target, &substitutions);
                collect_drop_dependencies(analysis, file, &target, visiting, dependencies);
            }
            for field in &symbol.fields {
                let field_ty = substitute_type_expr_parameters(&field.ty, &substitutions);
                collect_drop_dependencies(analysis, file, &field_ty, visiting, dependencies);
            }
            for variant in &symbol.variants {
                for payload in &variant.payload {
                    let payload_ty = substitute_type_expr_parameters(&payload.ty, &substitutions);
                    collect_drop_dependencies(analysis, file, &payload_ty, visiting, dependencies);
                }
            }
        }
        TypeExpr::Projection(_) => {
            if let Some(normalized) = crate::typecheck::normalize_associated_type_expr(ty, resolved)
            {
                collect_drop_dependencies(analysis, file, &normalized, visiting, dependencies)
            }
        }
        TypeExpr::Closure(_) | TypeExpr::Pointer(_) | TypeExpr::Borrow(_) | TypeExpr::View(_) => {}
    }

    visiting.remove(&key);
}

fn resolver_file_for_type<'a>(
    analysis: &'a CompileUnitAnalysis,
    preferred: &'a FileAnalysis,
    ty: &TypeExpr,
) -> Option<&'a FileAnalysis> {
    let name = match ty {
        TypeExpr::Reference(reference) => &reference.name,
        TypeExpr::Generic(generic) => &generic.name,
        _ => return Some(preferred),
    };
    std::iter::once(preferred)
        .chain(
            analysis
                .files
                .iter()
                .filter(|candidate| candidate.ast.span.source != preferred.ast.span.source),
        )
        .find(|file| file.resolved.type_symbol_by_reference_name(name).is_some())
}
