//! Provenance contracts for compiler-owned I/O primitive boundaries.

use crate::ast::{AstFile, Item, canonical_type_expr};
use crate::semantics::{TrustedDeclarationFacts, TrustedDeclarationRole};

pub(super) fn trusted_io_declarations(ast: &AstFile) -> TrustedDeclarationFacts {
    let mut facts = TrustedDeclarationFacts::default();
    for item in &ast.items {
        let Item::Primitive(primitive) = item else {
            continue;
        };
        let expected = match primitive.name.as_str() {
            "open_read_raw" => (&[("path", "*u8")][..], "i32!"),
            "write_text_raw" => (&[("fd", "i32"), ("text", "&str")][..], "void!"),
            "write_bytes_raw" => (&[("fd", "i32"), ("bytes", "&[u8]")][..], "void!"),
            "read_bytes_raw" => (&[("fd", "i32"), ("buffer", "&+[u8]")][..], "usize!"),
            _ => continue,
        };
        if primitive_shape_matches(primitive, expected.0, expected.1) {
            facts.insert(
                primitive.name_span,
                TrustedDeclarationRole::IndependentFallibleError,
            );
        }
    }
    facts
}

fn primitive_shape_matches(
    primitive: &crate::ast::PrimitiveDecl,
    parameters: &[(&str, &str)],
    return_type: &str,
) -> bool {
    primitive.generics.parameters.is_empty()
        && primitive.parameters.parameters.len() == parameters.len()
        && primitive
            .parameters
            .parameters
            .iter()
            .zip(parameters)
            .all(|(actual, (name, ty))| {
                actual.name == *name && canonical_type_expr(&actual.ty) == *ty
            })
        && canonical_type_expr(&primitive.return_type) == return_type
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::source::SourceMap;

    fn parse_text(text: &str) -> AstFile {
        let mut sources = SourceMap::new();
        let source = sources.add_source("io.nct", None, text);
        let lexed = lex(&sources, source);
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        parse(&sources, source, &lexed.tokens)
            .ast
            .expect("expected I/O primitive AST")
    }

    #[test]
    fn marks_validated_raw_io_errors_as_storage_independent() {
        let ast = parse_text(
            r#"pub(/) primitive open_read_raw(path: *u8): i32!
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
pub(/) primitive write_bytes_raw(fd: i32, bytes: &[u8]): void!
pub(/) primitive read_bytes_raw(fd: i32, buffer: &+[u8]): usize!
"#,
        );
        let facts = trusted_io_declarations(&ast);

        for item in &ast.items {
            let Item::Primitive(primitive) = item else {
                continue;
            };
            assert_eq!(
                facts.role(primitive.name_span),
                Some(TrustedDeclarationRole::IndependentFallibleError)
            );
        }
    }

    #[test]
    fn rejects_name_only_raw_io_contracts() {
        let ast = parse_text("pub(/) primitive write_text_raw(text: &str): void!\n");
        let facts = trusted_io_declarations(&ast);
        let Item::Primitive(primitive) = &ast.items[0] else {
            panic!("expected primitive");
        };

        assert_eq!(facts.role(primitive.name_span), None);
    }
}
