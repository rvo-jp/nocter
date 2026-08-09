//! Validated process-lifetime view declarations.

use crate::ast::{AstFile, Item, canonical_type_expr};
use crate::semantics::{TrustedDeclarationFacts, TrustedDeclarationRole};

pub(super) fn trusted_process_declarations(ast: &AstFile) -> TrustedDeclarationFacts {
    let mut facts = TrustedDeclarationFacts::default();
    for item in &ast.items {
        let Item::Primitive(primitive) = item else {
            continue;
        };
        if matches!(
            primitive.name.as_str(),
            "arg_raw" | "env_name_raw" | "env_value_raw"
        ) && primitive.generics.parameters.is_empty()
            && primitive.parameters.parameters.len() == 1
            && primitive.parameters.parameters[0].name == "index"
            && canonical_type_expr(&primitive.parameters.parameters[0].ty) == "usize"
            && canonical_type_expr(&primitive.return_type) == "&str"
        {
            facts.insert(primitive.name_span, TrustedDeclarationRole::StaticResult);
        }
    }
    facts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::source::SourceMap;

    fn parse_text(text: &str) -> AstFile {
        let mut sources = SourceMap::new();
        let source = sources.add_source("process.nct", None, text);
        let lexed = lex(&sources, source);
        parse(&sources, source, &lexed.tokens).ast.unwrap()
    }

    #[test]
    fn marks_only_complete_process_view_shapes_as_static() {
        let ast = parse_text(
            "pub(/) primitive arg_raw(index: usize): &str\n\
             pub(/) primitive env_name_raw(index: usize): &str\n\
             pub(/) primitive env_value_raw(index: usize): &str\n\
             pub(/) primitive arg_raw_wrong(index: i32): &str\n",
        );
        let facts = trusted_process_declarations(&ast);
        let roles = ast
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Primitive(primitive) => Some(facts.role(primitive.name_span)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            roles,
            vec![
                Some(TrustedDeclarationRole::StaticResult),
                Some(TrustedDeclarationRole::StaticResult),
                Some(TrustedDeclarationRole::StaticResult),
                None,
            ]
        );
    }
}
