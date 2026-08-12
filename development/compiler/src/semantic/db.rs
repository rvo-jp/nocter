//! Compile-unit definition and body identity.

use super::DefId;
use super::body_declarations::{BodyDeclaration, visit_body_declarations};
use crate::ast::{
    AstFile, Block, ConformanceMember, ConstructMemberDecl, FromImportItem, ImportItem,
    InstanceMember, Item, OperatorDecl,
};
use crate::source::ByteSpan;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DefinitionKind {
    Import,
    Function,
    Test,
    Primitive,
    TypeAlias,
    Struct,
    StructField,
    Enum,
    EnumVariant,
    Interface,
    AssociatedType,
    Instance,
    Method,
    ComparisonOperator,
    IndexOperator,
    ExpansionOperator,
    Coercion,
    Conformance,
    AssociatedTypeBinding,
    Destruct,
    Construct,
    Literal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Definition {
    pub(crate) id: DefId,
    pub(crate) kind: DefinitionKind,
    pub(crate) owner: Option<DefId>,
    pub(crate) anchor: ByteSpan,
    pub(crate) span: ByteSpan,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SemanticDb {
    definitions: Vec<Definition>,
    definitions_by_location: HashMap<ByteSpan, DefId>,
}

impl SemanticDb {
    pub(crate) fn from_files(files: &[AstFile]) -> Self {
        let mut builder = SemanticDbBuilder::default();
        for file in files {
            builder.collect_file(file);
        }
        builder.finish()
    }

    pub(crate) fn definition_at(&self, location: ByteSpan) -> Option<DefId> {
        self.definitions_by_location.get(&location).copied()
    }

    pub(crate) fn definition_span(&self, id: DefId) -> Option<ByteSpan> {
        self.definitions
            .get(id.index())
            .map(|definition| definition.span)
    }

    pub(crate) fn definition_anchor(&self, id: DefId) -> Option<ByteSpan> {
        self.definitions
            .get(id.index())
            .map(|definition| definition.anchor)
    }

    #[cfg(test)]
    pub(crate) fn definitions(&self) -> &[Definition] {
        &self.definitions
    }
}

#[derive(Default)]
struct SemanticDbBuilder {
    db: SemanticDb,
}

impl SemanticDbBuilder {
    fn finish(self) -> SemanticDb {
        self.db
    }

    fn define(
        &mut self,
        kind: DefinitionKind,
        owner: Option<DefId>,
        anchor: ByteSpan,
        span: ByteSpan,
    ) -> DefId {
        let id = DefId::from_index(self.db.definitions.len());
        let definition = Definition {
            id,
            kind,
            owner,
            anchor,
            span,
        };
        self.db.definitions.push(definition);
        self.db.definitions_by_location.insert(anchor, id);
        self.db.definitions_by_location.entry(span).or_insert(id);
        id
    }

    fn define_location(&mut self, id: DefId, location: ByteSpan) {
        self.db
            .definitions_by_location
            .entry(location)
            .or_insert(id);
    }

    fn collect_file(&mut self, file: &AstFile) {
        for item in &file.items {
            self.collect_item(item);
        }
    }

    fn collect_import(&mut self, owner: Option<DefId>, import: &ImportItem) {
        self.define(
            DefinitionKind::Import,
            owner,
            import.alias.span,
            import.span,
        );
    }

    fn collect_from_import(&mut self, owner: Option<DefId>, import: &FromImportItem) {
        for name in &import.names {
            let anchor = name
                .alias
                .as_ref()
                .map_or(name.name_span, |alias| alias.span);
            self.define(DefinitionKind::Import, owner, anchor, name.span);
        }
    }

    fn collect_body_declarations(&mut self, owner: DefId, body: &Block) {
        visit_body_declarations(body, &mut |declaration| match declaration {
            BodyDeclaration::Import(import) => self.collect_import(Some(owner), import),
            BodyDeclaration::FromImport(import) => self.collect_from_import(Some(owner), import),
        });
    }

    fn collect_item(&mut self, item: &Item) {
        match item {
            Item::Import(import) => {
                self.collect_import(None, import);
            }
            Item::FromImport(import) => {
                self.collect_from_import(None, import);
            }
            Item::Function(function) => {
                let id = self.define(
                    DefinitionKind::Function,
                    None,
                    function.name_span,
                    function.span,
                );
                self.define_location(id, function.member_name_span);
                if let Some(body) = &function.body {
                    self.collect_body_declarations(id, body);
                }
            }
            Item::Test(test) => {
                let id = self.define(DefinitionKind::Test, None, test.name_span, test.span);
                self.collect_body_declarations(id, &test.body);
            }
            Item::Primitive(primitive) => {
                self.define(
                    DefinitionKind::Primitive,
                    None,
                    primitive.name_span,
                    primitive.span,
                );
            }
            Item::TypeAlias(alias) => {
                self.define(DefinitionKind::TypeAlias, None, alias.name_span, alias.span);
            }
            Item::Struct(struct_) => {
                let owner = self.define(
                    DefinitionKind::Struct,
                    None,
                    struct_.name_span,
                    struct_.span,
                );
                for field in &struct_.fields {
                    self.define(
                        DefinitionKind::StructField,
                        Some(owner),
                        field.name_span,
                        field.span,
                    );
                }
            }
            Item::Enum(enum_) => {
                let owner = self.define(DefinitionKind::Enum, None, enum_.name_span, enum_.span);
                for variant in &enum_.variants {
                    self.define(
                        DefinitionKind::EnumVariant,
                        Some(owner),
                        variant.name_span,
                        variant.span,
                    );
                }
            }
            Item::Interface(interface) => {
                let owner = self.define(
                    DefinitionKind::Interface,
                    None,
                    interface.name_span,
                    interface.span,
                );
                for associated in &interface.associated_types {
                    self.define(
                        DefinitionKind::AssociatedType,
                        Some(owner),
                        associated.name_span,
                        associated.span,
                    );
                }
                for method in &interface.methods {
                    let id = self.define(
                        DefinitionKind::Method,
                        Some(owner),
                        method.name_span,
                        method.span,
                    );
                    if let Some(body) = &method.body {
                        self.collect_body_declarations(id, body);
                    }
                }
            }
            Item::Instance(instance) => {
                let owner = self.define(
                    DefinitionKind::Instance,
                    None,
                    instance.target_ty.span(),
                    instance.span,
                );
                for member in &instance.members {
                    self.collect_instance_member(owner, member);
                }
            }
            Item::Conformance(conformance) => {
                let owner = self.define(
                    DefinitionKind::Conformance,
                    None,
                    conformance.interface_ty.span(),
                    conformance.span,
                );
                for member in &conformance.members {
                    match member {
                        ConformanceMember::AssociatedType(binding) => {
                            self.define(
                                DefinitionKind::AssociatedTypeBinding,
                                Some(owner),
                                binding.name_span,
                                binding.span,
                            );
                        }
                        ConformanceMember::Method(method) => {
                            let id = self.define(
                                DefinitionKind::Method,
                                Some(owner),
                                method.name_span,
                                method.span,
                            );
                            if let Some(body) = &method.body {
                                self.collect_body_declarations(id, body);
                            }
                        }
                    }
                }
            }
            Item::Destruct(destruct) => {
                let id = self.define(
                    DefinitionKind::Destruct,
                    None,
                    destruct.keyword_span,
                    destruct.span,
                );
                self.collect_body_declarations(id, &destruct.body);
            }
            Item::Construct(construct) => {
                let owner = self.define(
                    DefinitionKind::Construct,
                    None,
                    construct.target.span(),
                    construct.span,
                );
                for member in &construct.members {
                    match &member.declaration {
                        ConstructMemberDecl::Function(function) => {
                            let id = self.define(
                                DefinitionKind::Function,
                                Some(owner),
                                function.name_span,
                                member.span,
                            );
                            self.define_location(id, function.span);
                            self.define_location(id, function.member_name_span);
                            if let Some(body) = &function.body {
                                self.collect_body_declarations(id, body);
                            }
                        }
                        ConstructMemberDecl::Literal(literal) => {
                            let id = self.define(
                                DefinitionKind::Literal,
                                Some(owner),
                                literal.shape_span,
                                member.span,
                            );
                            self.define_location(id, literal.span);
                            if let Some(body) = &literal.body {
                                self.collect_body_declarations(id, body);
                            }
                        }
                    }
                }
            }
        }
    }

    fn collect_instance_member(&mut self, owner: DefId, member: &InstanceMember) {
        let (kind, anchor, callable) = match member {
            InstanceMember::Method(method) => (DefinitionKind::Method, method.name_span, method),
            InstanceMember::Operator(operator) => {
                let kind = match operator {
                    OperatorDecl::Comparison(_) => DefinitionKind::ComparisonOperator,
                    OperatorDecl::Index(_) => DefinitionKind::IndexOperator,
                    OperatorDecl::Expansion(_) => DefinitionKind::ExpansionOperator,
                };
                let callable = operator.callable_method();
                (kind, callable.name_span, callable)
            }
            InstanceMember::Coercion(coercion) => (
                DefinitionKind::Coercion,
                coercion.as_span,
                coercion.callable_method(),
            ),
        };
        let id = self.define(kind, Some(owner), anchor, callable.span);
        if let Some(body) = &callable.body {
            self.collect_body_declarations(id, body);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::source::SourceMap;

    fn parse_file(text: &str) -> AstFile {
        let mut sources = SourceMap::new();
        let source = sources.add_source("semantic.nct", None, text);
        let lexed = lex(&sources, source);
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        let parsed = parse(&sources, source, &lexed.tokens);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        parsed.ast.expect("AST")
    }

    #[test]
    fn assigns_distinct_source_ordered_definition_and_body_ids() {
        let file = parse_file(
            r#"struct Text { value: i32 }
instance Text {
    method &self.len(): i32 { return self.value }
    pub coerce &self as &str { return "text" }
    pub operator (&self == other: &Self): bool { return true }
}
"#,
        );
        let db = SemanticDb::from_files(std::slice::from_ref(&file));
        let definitions = db.definitions();
        assert_eq!(definitions.len(), 6);
        assert_eq!(definitions[0].kind, DefinitionKind::Struct);
        assert_eq!(definitions[1].kind, DefinitionKind::StructField);
        assert_eq!(definitions[2].kind, DefinitionKind::Instance);
        assert_eq!(definitions[3].kind, DefinitionKind::Method);
        assert_eq!(definitions[4].kind, DefinitionKind::Coercion);
        assert_eq!(definitions[5].kind, DefinitionKind::ComparisonOperator);
        assert_eq!(definitions[3].owner, Some(definitions[2].id));
        assert_eq!(definitions[4].owner, Some(definitions[2].id));
        assert_eq!(definitions[5].owner, Some(definitions[2].id));
        assert_eq!(definitions[5].id.raw(), 5);
    }

    #[test]
    fn rebuilding_the_same_compile_unit_assigns_the_same_ids() {
        let file = parse_file("func first(): void { return }\nfunc second(): void { return }\n");
        let first = SemanticDb::from_files(std::slice::from_ref(&file));
        let second = SemanticDb::from_files(std::slice::from_ref(&file));
        assert_eq!(first, second);
        for definition in first.definitions() {
            assert_eq!(first.definition_at(definition.anchor), Some(definition.id));
            assert_eq!(second.definition_at(definition.anchor), Some(definition.id));
        }
    }

    #[test]
    fn indexes_nested_block_imports_under_their_callable() {
        let file = parse_file(
            r#"func main(debug: bool): void {
    if debug {
        use std/io.print
    }
    return
}
"#,
        );
        let db = SemanticDb::from_files(std::slice::from_ref(&file));
        let definitions = db.definitions();
        assert_eq!(definitions.len(), 2);
        assert_eq!(definitions[0].kind, DefinitionKind::Function);
        assert_eq!(definitions[1].kind, DefinitionKind::Import);
        assert_eq!(definitions[1].owner, Some(definitions[0].id));
        assert_eq!(
            db.definition_at(definitions[1].anchor),
            Some(definitions[1].id)
        );
    }

    #[test]
    fn associated_function_locations_share_one_definition() {
        let file = parse_file(
            r#"struct File { fd: i32 }
func File.open(): Self { return File { fd: 1 } }
"#,
        );
        let function = match &file.items[1] {
            Item::Function(function) => function,
            _ => panic!("expected associated function"),
        };
        let db = SemanticDb::from_files(std::slice::from_ref(&file));
        let definition = db.definition_at(function.span).unwrap();
        assert_eq!(db.definition_at(function.name_span), Some(definition));
        assert_eq!(
            db.definition_at(function.member_name_span),
            Some(definition)
        );
        assert_eq!(db.definition_anchor(definition), Some(function.name_span));
    }
}
