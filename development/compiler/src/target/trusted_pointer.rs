//! Provenance roles for validated pointer ownership primitives.

use crate::ast::{AstFile, Item, Visibility, canonical_type_expr};
use crate::semantics::{TrustedDeclarationFacts, TrustedDeclarationRole};

pub(super) fn trusted_pointer_declarations(ast: &AstFile) -> TrustedDeclarationFacts {
    let mut facts = TrustedDeclarationFacts::default();
    for item in &ast.items {
        let Item::Primitive(primitive) = item else {
            continue;
        };
        if primitive.name == "take_value_at_ptr" && take_value_shape_matches(primitive) {
            facts.insert(
                primitive.name_span,
                TrustedDeclarationRole::OwnedValueTransfer { source: 0 },
            );
        }
    }
    facts
}

fn take_value_shape_matches(primitive: &crate::ast::PrimitiveDecl) -> bool {
    primitive.visibility == Visibility::Nocter
        && primitive.generics.parameters.len() == 1
        && primitive.generics.parameters[0].name == "T"
        && primitive.generics.parameters[0].bounds.is_empty()
        && primitive.parameters.parameters.len() == 2
        && primitive.parameters.parameters[0].name == "pointer"
        && canonical_type_expr(&primitive.parameters.parameters[0].ty) == "*T"
        && primitive.parameters.parameters[1].name == "offset"
        && canonical_type_expr(&primitive.parameters.parameters[1].ty) == "usize"
        && canonical_type_expr(&primitive.return_type) == "T"
        && primitive.result_allocation.is_none()
        && primitive.result_provenance.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::source::SourceMap;

    #[test]
    fn attaches_only_to_the_exact_owned_transfer_shape() {
        let mut sources = SourceMap::new();
        let source = sources.add_source(
            "ptr.nct",
            None,
            "pub(nocter) primitive take_value_at_ptr<T>(pointer: *T, offset: usize): T\n",
        );
        let tokens = lex(&sources, source);
        let ast = parse(&sources, source, &tokens.tokens).ast.unwrap();
        let facts = trusted_pointer_declarations(&ast);
        let declaration = match &ast.items[0] {
            Item::Primitive(primitive) => primitive.name_span,
            _ => unreachable!(),
        };
        assert_eq!(
            facts.role(declaration),
            Some(TrustedDeclarationRole::OwnedValueTransfer { source: 0 })
        );
    }
}
