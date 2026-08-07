//! Resolution-independent presentation for semantic symbols whose defining file is unavailable.

use super::CallablePresentation;
use crate::resolve::{FunctionSignature, Symbol, SymbolKind, TypeSymbol, TypeSymbolKind};

pub(crate) fn symbol_presentation_without_resolution(symbol: &Symbol) -> String {
    match &symbol.kind {
        SymbolKind::Function(signature) => callable("func", &symbol.name, signature),
        SymbolKind::Primitive(signature) => callable("primitive", &symbol.name, signature),
        SymbolKind::Type(type_symbol) => type_declaration(&symbol.name, type_symbol),
        SymbolKind::Imported(imported) => format!("import {} from {}", symbol.name, imported.path),
    }
}

fn callable(kind: &str, name: &str, signature: &FunctionSignature) -> String {
    let generics = signature
        .generic_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let bounds = signature
                .generic_parameter_bounds
                .get(index)
                .into_iter()
                .flatten()
                .map(crate::ast::canonical_type_expr)
                .collect::<Vec<_>>();
            if bounds.is_empty() {
                parameter.clone()
            } else {
                format!("{parameter}: {}", bounds.join(" + "))
            }
        })
        .collect();
    let parameters = signature
        .parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name,
                crate::ast::canonical_type_expr(&parameter.ty)
            )
        })
        .collect();
    CallablePresentation::new(
        kind,
        name,
        generics,
        parameters,
        crate::ast::canonical_type_expr(&signature.return_type),
        super::result_origin_labels(signature.result_provenance.as_ref()),
    )
    .render()
}

fn type_declaration(name: &str, symbol: &TypeSymbol) -> String {
    let keyword = match symbol.kind {
        TypeSymbolKind::Alias => "type",
        TypeSymbolKind::Struct => "struct",
        TypeSymbolKind::Enum => "enum",
        TypeSymbolKind::Interface => "interface",
    };
    let copy = if symbol.kind == TypeSymbolKind::Struct && symbol.is_copy {
        "copy "
    } else {
        ""
    };
    let generics = symbol
        .generic_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let bounds = symbol
                .generic_parameter_bounds
                .get(index)
                .into_iter()
                .flatten()
                .map(crate::ast::canonical_type_expr)
                .collect::<Vec<_>>();
            if bounds.is_empty() {
                parameter.clone()
            } else {
                format!("{parameter}: {}", bounds.join(" + "))
            }
        })
        .collect::<Vec<_>>();
    let generics = if generics.is_empty() {
        String::new()
    } else {
        format!("<{}>", generics.join(", "))
    };
    let target = symbol
        .alias_target
        .as_ref()
        .map(|target| format!(" = {}", crate::ast::canonical_type_expr(target)))
        .unwrap_or_default();
    format!("{copy}{keyword} {name}{generics}{target}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::analyze_text;

    #[test]
    fn fallback_symbol_presentation_is_normalized_without_source_slices() {
        let text = "func choose<T>(value: (&T)?): (&T)? from value { return value }\n";
        let (_, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("root file");
        let symbol = file
            .resolved
            .symbols
            .symbol_by_name("choose")
            .expect("function symbol");
        assert_eq!(
            symbol_presentation_without_resolution(symbol),
            "func choose<T>(value: &T?): &T? from value"
        );
    }
}
