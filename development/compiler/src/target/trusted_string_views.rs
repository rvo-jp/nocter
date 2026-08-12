//! Provenance role for the restricted UTF-8 subview boundary.

use crate::ast::{AstFile, Item, ResultProvenanceOriginKind, Visibility, canonical_type_expr};
use crate::semantics::{TrustedDeclarationInputs, TrustedDeclarationRole};

pub(super) fn trusted_string_view_declarations(ast: &AstFile) -> TrustedDeclarationInputs {
    let mut facts = TrustedDeclarationInputs::default();
    for item in &ast.items {
        let Item::Primitive(primitive) = item else {
            continue;
        };
        if subview_shape_matches(primitive) {
            facts.insert(
                primitive.name_span,
                TrustedDeclarationRole::BorrowedProjection { source: 0 },
            );
        }
    }
    facts
}

fn subview_shape_matches(primitive: &crate::ast::PrimitiveDecl) -> bool {
    primitive.visibility == Visibility::Package
        && primitive.target.is_none()
        && primitive.name == "str_subview_unchecked"
        && primitive.generics.parameters.is_empty()
        && primitive.parameters.parameters.len() == 3
        && parameter_matches(&primitive.parameters.parameters[0], "text", "&str")
        && parameter_matches(&primitive.parameters.parameters[1], "start", "usize")
        && parameter_matches(&primitive.parameters.parameters[2], "len", "usize")
        && canonical_type_expr(&primitive.return_type) == "&str"
        && primitive.result_provenance.as_ref().is_some_and(|clause| {
            matches!(
                clause.origins.as_slice(),
                [origin]
                    if origin.kind
                        == ResultProvenanceOriginKind::Parameter("text".to_string())
            )
        })
}

fn parameter_matches(parameter: &crate::ast::Parameter, name: &str, ty: &str) -> bool {
    parameter.name == name && canonical_type_expr(&parameter.ty) == ty
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::source::SourceMap;

    fn role_for(source_text: &str, module_path: &str) -> Option<TrustedDeclarationRole> {
        let mut sources = SourceMap::new();
        let source = sources.add_source("string_views.nct", None, source_text);
        let tokens = lex(&sources, source);
        let ast = parse(&sources, source, &tokens.tokens).ast.unwrap();
        let declaration = ast.items.iter().find_map(|item| match item {
            Item::Primitive(primitive) => Some(primitive.name_span),
            _ => None,
        })?;
        crate::target::trusted::trusted_declarations_for_module(module_path, &ast).role(declaration)
    }

    #[test]
    fn attaches_only_to_the_exact_owned_module_and_shape() {
        let exact = r#"pub(/) primitive str_subview_unchecked(
    text: &str,
    start: usize,
    len: usize,
): &str from text
"#;
        assert_eq!(
            role_for(exact, "std/string"),
            Some(TrustedDeclarationRole::BorrowedProjection { source: 0 })
        );
        assert_eq!(role_for(exact, "app/string_views"), None);

        let wrong_result = exact.replace(": &str from text", ": &[u8] from text");
        assert_eq!(role_for(&wrong_result, "std/string"), None);

        let missing_contract = exact.replace(" from text", "");
        assert_eq!(role_for(&missing_contract, "std/string"), None);

        let near_misses = [
            exact.replace("pub(/)", "pub"),
            exact.replace(
                "pub(/) primitive",
                "#target: \"arm64-darwin\"\npub(/) primitive",
            ),
            exact.replace("str_subview_unchecked", "str_subview"),
            exact.replace("str_subview_unchecked(", "str_subview_unchecked<T>("),
            exact.replace("text: &str,", "source: &str,"),
            exact.replace("start: usize,", "start: i32,"),
            exact.replace("    len: usize,\n", ""),
            exact.replace("from text", "from start"),
            exact.replace("from text", "from text | start"),
        ];
        for near_miss in near_misses {
            assert_eq!(role_for(&near_miss, "std/string"), None, "{near_miss}");
        }
    }
}
