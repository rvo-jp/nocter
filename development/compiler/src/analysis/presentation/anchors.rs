//! Source anchors for callable presentation and semantic annotations.

use crate::ast::{
    AstFile, ConstructMemberDecl, FunctionDecl, ImplMember, Item, LiteralDecl, MethodDecl,
    PrimitiveDecl, TestDecl,
};
use crate::source::ByteSpan;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CallableAnchors {
    pub(crate) declaration: ByteSpan,
    pub(crate) name: ByteSpan,
    pub(crate) return_type: Option<ByteSpan>,
    pub(crate) explicit_result_provenance: Option<ByteSpan>,
    pub(crate) signature_end: usize,
}

impl CallableAnchors {
    fn with_return_type(
        declaration: ByteSpan,
        name: ByteSpan,
        return_type: ByteSpan,
        explicit_result_provenance: Option<ByteSpan>,
    ) -> Self {
        Self {
            declaration,
            name,
            return_type: Some(return_type),
            explicit_result_provenance,
            signature_end: explicit_result_provenance
                .map_or(return_type.end, |provenance| provenance.end),
        }
    }

    fn test(test: &TestDecl) -> Self {
        Self {
            declaration: test.name_span,
            name: test.name_span,
            return_type: None,
            explicit_result_provenance: None,
            signature_end: test.name_span.end,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CallableDeclarationIndex {
    entries: HashMap<ByteSpan, CallableAnchors>,
}

impl CallableDeclarationIndex {
    pub(crate) fn new(ast: &AstFile) -> Self {
        let mut index = Self::default();
        for item in &ast.items {
            match item {
                Item::Function(function) => index.insert_function(function),
                Item::Test(test) => index.insert(CallableAnchors::test(test)),
                Item::Primitive(primitive) => index.insert_primitive(primitive),
                Item::Interface(interface) => {
                    for method in &interface.methods {
                        index.insert_method(method);
                    }
                }
                Item::Impl(impl_) => {
                    for member in &impl_.members {
                        if let ImplMember::Method(method) = member {
                            index.insert_method(method);
                        }
                    }
                }
                Item::Construct(construct) => {
                    for member in &construct.members {
                        match &member.declaration {
                            ConstructMemberDecl::Function(function) => {
                                index.insert_function(function)
                            }
                            ConstructMemberDecl::Literal(literal) => index.insert_literal(literal),
                        }
                    }
                }
                Item::Import(_)
                | Item::FromImport(_)
                | Item::TypeAlias(_)
                | Item::Struct(_)
                | Item::Enum(_) => {}
            }
        }
        index
    }

    #[cfg(test)]
    pub(crate) fn get(&self, declaration: ByteSpan) -> Option<&CallableAnchors> {
        self.entries.get(&declaration)
    }

    fn insert(&mut self, anchors: CallableAnchors) {
        self.entries.insert(anchors.declaration, anchors);
    }

    fn insert_function(&mut self, function: &FunctionDecl) {
        let declaration = if function.owner.is_some() {
            function.member_name_span
        } else {
            function.name_span
        };
        self.insert(CallableAnchors::with_return_type(
            declaration,
            function.member_name_span,
            function.return_type.span(),
            function
                .result_provenance
                .as_ref()
                .map(|provenance| provenance.span),
        ));
    }

    fn insert_primitive(&mut self, primitive: &PrimitiveDecl) {
        self.insert(CallableAnchors::with_return_type(
            primitive.name_span,
            primitive.name_span,
            primitive.return_type.span(),
            primitive
                .result_provenance
                .as_ref()
                .map(|provenance| provenance.span),
        ));
    }

    fn insert_method(&mut self, method: &MethodDecl) {
        self.insert(CallableAnchors::with_return_type(
            method.name_span,
            method.name_span,
            method.return_type.span(),
            method
                .result_provenance
                .as_ref()
                .map(|provenance| provenance.span),
        ));
    }

    fn insert_literal(&mut self, literal: &LiteralDecl) {
        self.insert(CallableAnchors::with_return_type(
            literal.span,
            literal.shape_span,
            literal.return_type.span(),
            literal
                .result_provenance
                .as_ref()
                .map(|provenance| provenance.span),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceMap;

    #[test]
    fn indexes_signature_ends_for_every_callable_shape() {
        let text = r#"func plain(): &str {
    return "static"
}

primitive raw(): &str from static

interface Build {
    pub method &self.make(): &str from self
}

impl Build for i32 {
    method &self.make(): &str {
        return "static"
    }
}

construct i32 {
    pub func new(): i32 { return 0 }
    pub literal ""(text: &str): i32 { return 0 }
}

test works {
    return
}
"#;
        let mut sources = SourceMap::new();
        let source = sources.add_source("test.nct", None, text.to_string());
        let tokens = crate::lexer::lex(&sources, source);
        let parsed = crate::parser::parse(&sources, source, &tokens.tokens);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let ast = parsed.ast.expect("AST");
        let declarations = CallableDeclarationIndex::new(&ast);

        let plain = declarations
            .get(ByteSpan::new(
                ast.span.source,
                text.find("plain").unwrap(),
                text.find("plain").unwrap() + "plain".len(),
            ))
            .expect("function anchors");
        assert_eq!(
            plain.signature_end,
            text.find("&str {").unwrap() + "&str".len()
        );

        for marker in ["raw", "make", "new"] {
            let start = text.find(marker).unwrap();
            assert!(
                declarations
                    .get(ByteSpan::new(ast.span.source, start, start + marker.len()))
                    .is_some(),
                "missing {marker}"
            );
        }

        let literal = declarations
            .entries
            .values()
            .find(|anchors| anchors.name.start == text.rfind("\"\"").unwrap())
            .expect("literal anchors");
        assert_eq!(
            literal.return_type.map(|span| span.end),
            Some(literal.signature_end)
        );
    }
}
