//! Resolved callable outcome shapes shared by buildability, IR, and tooling.
//!
//! Optional and fallible constructors are independent semantic layers. This module resolves aliases
//! once and preserves their order instead of collapsing both into one backend failure bit.

use crate::ast::TypeExpr;
use crate::resolve::ResolveOutput;
use crate::source::SourceId;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutcomeLayer {
    Optional,
    Fallible,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutcomeShape {
    pub(crate) layers: Vec<OutcomeLayer>,
    pub(crate) payload: TypeExpr,
}

impl OutcomeShape {
    pub(crate) fn is_supported_callable_shape(&self) -> bool {
        match self.layers.as_slice() {
            [] | [OutcomeLayer::Optional] | [OutcomeLayer::Fallible] => true,
            [OutcomeLayer::Fallible, OutcomeLayer::Optional]
            | [OutcomeLayer::Optional, OutcomeLayer::Fallible] => true,
            _ => false,
        }
    }
}

pub(crate) fn outcome_shape_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> OutcomeShape
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let mut layers = Vec::new();
    let payload = collect_outcome_shape(
        ty,
        fallback_resolved,
        &resolver,
        &mut HashSet::new(),
        &mut layers,
    );
    OutcomeShape { layers, payload }
}

fn collect_outcome_shape<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
    layers: &mut Vec<OutcomeLayer>,
) -> TypeExpr
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Optional(optional) => {
            layers.push(OutcomeLayer::Optional);
            collect_outcome_shape(
                &optional.inner,
                fallback_resolved,
                resolver,
                resolving_names,
                layers,
            )
        }
        TypeExpr::Fallible(fallible) => {
            layers.push(OutcomeLayer::Fallible);
            collect_outcome_shape(
                &fallible.success,
                fallback_resolved,
                resolver,
                resolving_names,
                layers,
            )
        }
        TypeExpr::Reference(reference) => {
            let resolved = resolver(ty.span().source).unwrap_or(fallback_resolved);
            let symbol = resolved
                .type_symbol_by_reference_name(&reference.name)
                .or_else(|| fallback_resolved.type_symbol_by_reference_name(&reference.name));
            let Some(symbol) = symbol else {
                return ty.clone();
            };
            let Some(target) = &symbol.alias_target else {
                return ty.clone();
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return ty.clone();
            }
            let payload =
                collect_outcome_shape(target, fallback_resolved, resolver, resolving_names, layers);
            resolving_names.remove(&symbol.canonical_name);
            payload
        }
        _ => ty.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::analyze_text;

    fn shape(source: &str, name: &str) -> OutcomeShape {
        let (sources, analysis) = analyze_text(source);
        assert!(
            analysis.diagnostics().is_empty(),
            "{:?}",
            analysis.diagnostics()
        );
        let file = analysis.root_file().expect("root file");
        let function = file
            .ast
            .items
            .iter()
            .find_map(|item| match item {
                crate::ast::Item::Function(function) if function.name == name => Some(function),
                _ => None,
            })
            .expect("function");
        let _ = sources;
        outcome_shape_with_resolver(&function.return_type, &file.resolved, |_| {
            Some(&file.resolved)
        })
    }

    #[test]
    fn preserves_fallible_optional_layer_order_through_aliases() {
        let shape = shape(
            r#"type MaybeText = &str?
type Lookup = MaybeText!

func lookup(): Lookup {
    return none
}
"#,
            "lookup",
        );

        assert_eq!(
            shape.layers,
            vec![OutcomeLayer::Fallible, OutcomeLayer::Optional]
        );
        assert!(matches!(shape.payload, TypeExpr::Borrow(_)));
        assert!(shape.is_supported_callable_shape());
    }

    #[test]
    fn rejects_repeated_or_deeper_outcome_layers() {
        let shape = shape(
            r#"func nested(): ((i32?)?)! {
    return none
}
"#,
            "nested",
        );

        assert_eq!(shape.layers.len(), 3);
        assert!(shape.layers.len() > 1);
        assert!(!shape.is_supported_callable_shape());
    }
}
