//! Editor presentation for resolver-owned construction surfaces.

use crate::analysis::presentation::{
    associated_function_presentation, literal_signature_presentation, type_owner_presentation_label,
};
use crate::resolve::{ConstructionEntryKind, ResolveOutput, TypeSymbol};
use crate::typecheck::{enum_variant_member_label, type_expr_presentation_label};

pub(crate) fn construction_surface_markdown(
    owner: &TypeSymbol,
    resolved: &ResolveOutput,
) -> Option<String> {
    if matches!(
        owner.kind,
        crate::resolve::TypeSymbolKind::Alias | crate::resolve::TypeSymbolKind::Interface
    ) {
        return None;
    }

    let entries = owner
        .construction
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.is_accessible)
        .filter_map(|(index, entry)| {
            construction_entry_label(owner, resolved, &entry.kind).map(|label| {
                let default = if owner.construction.default_entry == Some(index) {
                    "default "
                } else {
                    ""
                };
                format!("- `{default}{label}`")
            })
        })
        .collect::<Vec<_>>();

    if entries.is_empty() {
        return Some(
            "**Construction**\n\nNo direct construction entry is available here.".to_string(),
        );
    }

    Some(format!("**Construction**\n\n{}", entries.join("\n")))
}

pub(crate) fn construction_owns_function(owner: &TypeSymbol, name: &str) -> bool {
    owner.construction.entries.iter().any(|entry| {
        matches!(&entry.kind, ConstructionEntryKind::Function(function) if function == name)
    })
}

pub(crate) fn construction_function_is_default(owner: &TypeSymbol, name: &str) -> bool {
    owner
        .construction
        .default_entry
        .and_then(|index| owner.construction.entries.get(index))
        .is_some_and(|entry| {
            matches!(&entry.kind, ConstructionEntryKind::Function(function) if function == name)
        })
}

fn construction_entry_label(
    owner: &TypeSymbol,
    resolved: &ResolveOutput,
    kind: &ConstructionEntryKind,
) -> Option<String> {
    match kind {
        ConstructionEntryKind::Structural => Some(structural_entry_label(owner, resolved)),
        ConstructionEntryKind::Function(name) => owner
            .associated_functions
            .iter()
            .find(|function| function.name == *name && function.is_accessible)
            .map(|function| associated_function_presentation(owner, function, resolved).render()),
        ConstructionEntryKind::Literal(shape) => owner
            .literals
            .iter()
            .find(|literal| literal.shape == *shape && literal.is_accessible)
            .map(|literal| literal_signature_presentation(owner, literal, resolved).render()),
        ConstructionEntryKind::Variant(name) => owner
            .variants
            .iter()
            .find(|variant| variant.name == *name)
            .map(|variant| {
                let parameters = variant
                    .payload
                    .iter()
                    .map(|parameter| {
                        format!(
                            "{}: {}",
                            parameter.name,
                            type_expr_presentation_label(&parameter.ty, resolved)
                        )
                    })
                    .collect::<Vec<_>>();
                enum_variant_member_label(
                    &type_owner_presentation_label(owner, resolved),
                    &variant.name,
                    &parameters,
                )
            }),
    }
}

fn structural_entry_label(owner: &TypeSymbol, resolved: &ResolveOutput) -> String {
    let fields = owner
        .fields
        .iter()
        .filter(|field| field.is_accessible)
        .map(|field| {
            format!(
                "{}: {}",
                field.name,
                type_expr_presentation_label(&field.ty, resolved)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{} {{ {fields} }}",
        type_owner_presentation_label(owner, resolved)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::analyze_text;

    #[test]
    fn renders_default_and_alternative_construction_entries() {
        let text = r#"pub struct Bucket<T> { value: T }

construct Bucket<T> {
    pub default func new(value: T): Self { return Bucket<T> { value: value } }
    pub literal [](...items: T): Self { return Bucket.new(move items[0]) }
}
"#;
        let (_, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let owner = file
            .resolved
            .type_symbol_by_name("Bucket")
            .expect("expected Bucket");

        let markdown = construction_surface_markdown(owner, &file.resolved)
            .expect("expected construction markdown");
        assert!(markdown.contains("default func Bucket<T>.new(value: T): Bucket<T>"));
        assert!(markdown.contains("literal Bucket<T> [](...items: T): Bucket<T>"));
        assert!(!markdown.contains("new<T>"));
    }
}
