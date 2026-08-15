//! Compile-unit definition and body identity.

use super::body_declarations::{BodyDeclaration, visit_body_declarations};
use super::{BodyId, DefId, ExprId, OpaqueTypeId, StmtId};
use crate::ast::{
    AstFile, Block, ConformanceMember, ConstructMemberDecl, Expr, FromImportItem, GenericParamList,
    ImportItem, InstanceMember, Item, LiteralDecl, OperatorDecl, ParameterList,
    visit_block_expressions_without_nested_closures,
    visit_block_statements_without_nested_closures, visit_type_exprs,
};
use crate::source::ByteSpan;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DefinitionKind {
    Import,
    Function,
    AssociatedFunction,
    GenericParameter,
    Parameter,
    Receiver,
    LiteralCapture,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BodyKind {
    Declaration,
    Closure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BodyDefinition {
    pub(crate) id: BodyId,
    pub(crate) kind: BodyKind,
    pub(crate) owner: DefId,
    pub(crate) parent: Option<BodyId>,
    pub(crate) anchor: ByteSpan,
    pub(crate) span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExpressionDefinition {
    pub(crate) id: ExprId,
    pub(crate) body: BodyId,
    pub(crate) span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatementDefinition {
    pub(crate) id: StmtId,
    pub(crate) body: BodyId,
    pub(crate) span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpaqueTypeDefinition {
    pub(crate) id: OpaqueTypeId,
    pub(crate) owner: DefId,
    pub(crate) anchor: ByteSpan,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SemanticDb {
    definitions: Vec<Definition>,
    definitions_by_location: HashMap<ByteSpan, DefId>,
    bodies: Vec<BodyDefinition>,
    bodies_by_location: HashMap<ByteSpan, BodyId>,
    declaration_bodies_by_owner: HashMap<DefId, BodyId>,
    expressions: Vec<ExpressionDefinition>,
    expressions_by_location: HashMap<ByteSpan, ExprId>,
    statements: Vec<StatementDefinition>,
    statements_by_location: HashMap<ByteSpan, StmtId>,
    opaque_types: Vec<OpaqueTypeDefinition>,
    opaque_types_by_location: HashMap<ByteSpan, OpaqueTypeId>,
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

    pub(crate) fn definition(&self, id: DefId) -> Option<&Definition> {
        self.definitions.get(id.index())
    }

    pub(crate) fn definition_containing(
        &self,
        source: crate::source::SourceId,
        offset: usize,
    ) -> Option<&Definition> {
        self.definitions
            .iter()
            .filter(|definition| {
                definition.span.source == source
                    && definition.span.start <= offset
                    && offset <= definition.span.end
            })
            .min_by_key(|definition| (definition.span.len(), definition.span.start))
    }

    pub(crate) fn definition_ancestor_distance(
        &self,
        ancestor: DefId,
        descendant: DefId,
    ) -> Option<usize> {
        let mut current = Some(descendant);
        let mut distance = 0;
        while let Some(definition) = current {
            if definition == ancestor {
                return Some(distance);
            }
            current = self.definition(definition)?.owner;
            distance += 1;
        }
        None
    }

    pub(crate) fn definition_ancestor_with_kinds(
        &self,
        descendant: DefId,
        kinds: &[DefinitionKind],
    ) -> Option<&Definition> {
        let mut current = Some(descendant);
        while let Some(definition) = current {
            let record = self.definition(definition)?;
            if kinds.contains(&record.kind) {
                return Some(record);
            }
            current = record.owner;
        }
        None
    }

    pub(crate) fn body_at(&self, location: ByteSpan) -> Option<BodyId> {
        self.bodies_by_location.get(&location).copied()
    }

    pub(crate) fn body_anchor(&self, id: BodyId) -> Option<ByteSpan> {
        self.bodies.get(id.index()).map(|body| body.anchor)
    }

    pub(crate) fn body_containing(
        &self,
        source: crate::source::SourceId,
        offset: usize,
    ) -> Option<&BodyDefinition> {
        self.bodies
            .iter()
            .filter(|body| {
                body.span.source == source && body.span.start <= offset && offset <= body.span.end
            })
            .min_by_key(|body| (body.span.len(), body.span.start))
    }

    pub(crate) fn declaration_body_for_owner(&self, owner: DefId) -> Option<&BodyDefinition> {
        self.declaration_bodies_by_owner
            .get(&owner)
            .and_then(|body| self.bodies.get(body.index()))
    }

    pub(crate) fn expression_at(&self, location: ByteSpan) -> Option<ExprId> {
        self.expressions_by_location.get(&location).copied()
    }

    pub(crate) fn expression(&self, id: ExprId) -> Option<&ExpressionDefinition> {
        self.expressions.get(id.index())
    }

    pub(crate) fn statement_at(&self, location: ByteSpan) -> Option<StmtId> {
        self.statements_by_location.get(&location).copied()
    }

    pub(crate) fn statement(&self, id: StmtId) -> Option<&StatementDefinition> {
        self.statements.get(id.index())
    }

    pub(crate) fn opaque_type_at(&self, location: ByteSpan) -> Option<OpaqueTypeId> {
        self.opaque_types_by_location.get(&location).copied()
    }

    pub(crate) fn opaque_type_anchor(&self, id: OpaqueTypeId) -> Option<ByteSpan> {
        self.opaque_types.get(id.index()).map(|ty| ty.anchor)
    }

    pub(crate) fn definitions(&self) -> &[Definition] {
        &self.definitions
    }

    #[cfg(test)]
    pub(crate) fn bodies(&self) -> &[BodyDefinition] {
        &self.bodies
    }

    #[cfg(test)]
    pub(crate) fn expressions(&self) -> &[ExpressionDefinition] {
        &self.expressions
    }

    #[cfg(test)]
    pub(crate) fn statements(&self) -> &[StatementDefinition] {
        &self.statements
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

    fn define_body(
        &mut self,
        kind: BodyKind,
        owner: DefId,
        parent: Option<BodyId>,
        anchor: ByteSpan,
        span: ByteSpan,
    ) -> BodyId {
        let id = BodyId::from_index(self.db.bodies.len());
        self.db.bodies.push(BodyDefinition {
            id,
            kind,
            owner,
            parent,
            anchor,
            span,
        });
        self.db.bodies_by_location.insert(anchor, id);
        self.db.bodies_by_location.entry(span).or_insert(id);
        if kind == BodyKind::Declaration {
            self.db.declaration_bodies_by_owner.insert(owner, id);
        }
        id
    }

    fn define_expression(&mut self, body: BodyId, span: ByteSpan) -> ExprId {
        if let Some(existing) = self.db.expressions_by_location.get(&span) {
            return *existing;
        }
        let id = ExprId::from_index(self.db.expressions.len());
        self.db
            .expressions
            .push(ExpressionDefinition { id, body, span });
        self.db.expressions_by_location.insert(span, id);
        id
    }

    fn define_statement(&mut self, body: BodyId, span: ByteSpan) -> StmtId {
        if let Some(existing) = self.db.statements_by_location.get(&span) {
            return *existing;
        }
        let id = StmtId::from_index(self.db.statements.len());
        self.db
            .statements
            .push(StatementDefinition { id, body, span });
        self.db.statements_by_location.insert(span, id);
        id
    }

    fn collect_type_expr(&mut self, owner: DefId, ty: &crate::ast::TypeExpr) {
        visit_type_exprs(ty, &mut |ty| {
            let crate::ast::TypeExpr::Opaque(opaque) = ty else {
                return;
            };
            if self
                .db
                .opaque_types_by_location
                .contains_key(&opaque.some_span)
            {
                return;
            }
            let id = OpaqueTypeId::from_index(self.db.opaque_types.len());
            self.db.opaque_types.push(OpaqueTypeDefinition {
                id,
                owner,
                anchor: opaque.some_span,
            });
            self.db
                .opaque_types_by_location
                .insert(opaque.some_span, id);
        });
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

    fn collect_body(&mut self, owner: DefId, body: &Block) {
        self.collect_body_declarations(owner, body);
        let body_id = self.define_body(BodyKind::Declaration, owner, None, body.span, body.span);
        self.collect_body_statements(body_id, body);
        self.collect_body_expressions(body_id, body);
        self.collect_closure_bodies(owner, body_id, body);
    }

    fn collect_body_expressions(&mut self, body_id: BodyId, body: &Block) {
        let mut expressions = Vec::new();
        visit_block_expressions_without_nested_closures(body, &mut |expression| {
            expressions.push(expression.span())
        });
        for expression in expressions {
            self.define_expression(body_id, expression);
        }
    }

    fn collect_body_statements(&mut self, body_id: BodyId, body: &Block) {
        let mut statements = Vec::new();
        visit_block_statements_without_nested_closures(body, &mut |statement| {
            statements.push(statement.span())
        });
        for statement in statements {
            self.define_statement(body_id, statement);
        }
    }

    fn collect_closure_bodies(&mut self, owner: DefId, parent: BodyId, body: &Block) {
        let mut closures = Vec::new();
        visit_block_expressions_without_nested_closures(body, &mut |expression| {
            if let Expr::Closure(closure) = expression {
                closures.push(closure);
            }
        });
        for closure in closures {
            let body_id = self.define_body(
                BodyKind::Closure,
                owner,
                Some(parent),
                closure.span,
                closure.body.span,
            );
            self.collect_body_statements(body_id, &closure.body);
            self.collect_body_expressions(body_id, &closure.body);
            self.collect_closure_bodies(owner, body_id, &closure.body);
        }
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
                    if function.owner.is_some() {
                        DefinitionKind::AssociatedFunction
                    } else {
                        DefinitionKind::Function
                    },
                    None,
                    function.member_name_span,
                    function.span,
                );
                self.define_location(id, function.name_span);
                self.define_location(id, function.member_name_span);
                self.collect_generics(id, &function.generics);
                self.collect_parameters(id, &function.parameters);
                self.collect_type_expr(id, &function.return_type);
                if let Some(body) = &function.body {
                    self.collect_body(id, body);
                }
            }
            Item::Test(test) => {
                let id = self.define(DefinitionKind::Test, None, test.name_span, test.span);
                self.collect_body(id, &test.body);
            }
            Item::Primitive(primitive) => {
                let id = self.define(
                    DefinitionKind::Primitive,
                    None,
                    primitive.name_span,
                    primitive.span,
                );
                self.collect_generics(id, &primitive.generics);
                self.collect_parameters(id, &primitive.parameters);
                self.collect_type_expr(id, &primitive.return_type);
            }
            Item::TypeAlias(alias) => {
                let owner =
                    self.define(DefinitionKind::TypeAlias, None, alias.name_span, alias.span);
                self.collect_generics(owner, &alias.generics);
                self.collect_type_expr(owner, &alias.target);
            }
            Item::Struct(struct_) => {
                let owner = self.define(
                    DefinitionKind::Struct,
                    None,
                    struct_.name_span,
                    struct_.span,
                );
                self.collect_generics(owner, &struct_.generics);
                for field in &struct_.fields {
                    let id = self.define(
                        DefinitionKind::StructField,
                        Some(owner),
                        field.name_span,
                        field.span,
                    );
                    self.collect_type_expr(id, &field.ty);
                }
            }
            Item::Enum(enum_) => {
                let owner = self.define(DefinitionKind::Enum, None, enum_.name_span, enum_.span);
                self.collect_generics(owner, &enum_.generics);
                for variant in &enum_.variants {
                    let id = self.define(
                        DefinitionKind::EnumVariant,
                        Some(owner),
                        variant.name_span,
                        variant.span,
                    );
                    for parameter in &variant.payload {
                        self.collect_type_expr(id, &parameter.ty);
                    }
                }
            }
            Item::Interface(interface) => {
                let owner = self.define(
                    DefinitionKind::Interface,
                    None,
                    interface.name_span,
                    interface.span,
                );
                self.collect_generics(owner, &interface.generics);
                for associated in &interface.associated_types {
                    let id = self.define(
                        DefinitionKind::AssociatedType,
                        Some(owner),
                        associated.name_span,
                        associated.span,
                    );
                    for bound in &associated.bounds {
                        self.collect_type_expr(id, bound);
                    }
                }
                for method in &interface.methods {
                    let id = self.define(
                        DefinitionKind::Method,
                        Some(owner),
                        method.name_span,
                        method.span,
                    );
                    self.collect_method_inputs(id, method);
                    if let Some(body) = &method.body {
                        self.collect_body(id, body);
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
                self.collect_generics(owner, &instance.generics);
                self.collect_type_expr(owner, &instance.target_ty);
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
                self.collect_generics(owner, &conformance.generics);
                self.collect_type_expr(owner, &conformance.interface_ty);
                self.collect_type_expr(owner, &conformance.target_ty);
                for member in &conformance.members {
                    match member {
                        ConformanceMember::AssociatedType(binding) => {
                            let id = self.define(
                                DefinitionKind::AssociatedTypeBinding,
                                Some(owner),
                                binding.name_span,
                                binding.span,
                            );
                            self.collect_type_expr(id, &binding.value);
                        }
                        ConformanceMember::Method(method) => {
                            let id = self.define(
                                DefinitionKind::Method,
                                Some(owner),
                                method.name_span,
                                method.span,
                            );
                            self.collect_method_inputs(id, method);
                            if let Some(body) = &method.body {
                                self.collect_body(id, body);
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
                self.collect_generics(id, &destruct.generics);
                self.collect_type_expr(id, &destruct.target_ty);
                let parameter = self.define(
                    DefinitionKind::Parameter,
                    Some(id),
                    destruct.binding.name_span,
                    destruct.binding.span,
                );
                self.collect_type_expr(parameter, &destruct.binding.ty);
                self.collect_body(id, &destruct.body);
            }
            Item::Construct(construct) => {
                let owner = self.define(
                    DefinitionKind::Construct,
                    None,
                    construct.target.span(),
                    construct.span,
                );
                self.collect_type_expr(owner, &construct.target);
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
                            self.collect_generics(id, &function.generics);
                            self.collect_parameters(id, &function.parameters);
                            self.collect_type_expr(id, &function.return_type);
                            if let Some(body) = &function.body {
                                self.collect_body(id, body);
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
                            self.collect_literal_inputs(id, literal);
                            self.collect_type_expr(id, &literal.target);
                            self.collect_type_expr(id, &literal.return_type);
                            if let Some(body) = &literal.body {
                                self.collect_body(id, body);
                            }
                        }
                    }
                }
            }
        }
    }

    fn collect_instance_member(&mut self, owner: DefId, member: &InstanceMember) {
        let (kind, anchor, callable) = match member {
            InstanceMember::Method(method) => {
                (DefinitionKind::Method, method.name_span, &method.callable)
            }
            InstanceMember::Operator(operator) => {
                let kind = match operator {
                    OperatorDecl::Comparison(_) => DefinitionKind::ComparisonOperator,
                    OperatorDecl::Index(_) => DefinitionKind::IndexOperator,
                    OperatorDecl::Expansion(_) => DefinitionKind::ExpansionOperator,
                };
                (kind, operator.anchor_span(), operator.callable())
            }
            InstanceMember::Coercion(coercion) => (
                DefinitionKind::Coercion,
                coercion.as_span,
                coercion.callable(),
            ),
        };
        let id = self.define(kind, Some(owner), anchor, callable.span);
        self.collect_method_inputs(id, callable);
        if let Some(body) = &callable.body {
            self.collect_body(id, body);
        }
    }

    fn collect_method_inputs(&mut self, owner: DefId, method: &crate::ast::CallableDecl) {
        self.collect_generics(owner, &method.generics);
        self.define(
            DefinitionKind::Receiver,
            Some(owner),
            method.receiver.name_span,
            method.receiver.span,
        );
        self.collect_parameters(owner, &method.parameters);
        self.collect_type_expr(owner, &method.return_type);
    }

    fn collect_generics(&mut self, owner: DefId, generics: &GenericParamList) {
        for parameter in &generics.parameters {
            self.define(
                DefinitionKind::GenericParameter,
                Some(owner),
                parameter.name_span,
                parameter.span,
            );
        }
    }

    fn collect_literal_inputs(&mut self, owner: DefId, literal: &LiteralDecl) {
        self.collect_parameters(owner, &literal.parameters);
        if let Some(capture) = &literal.capture {
            let id = self.define(
                DefinitionKind::LiteralCapture,
                Some(owner),
                capture.name_span,
                capture.span,
            );
            self.collect_type_expr(id, &capture.element_type);
        }
    }

    fn collect_parameters(&mut self, owner: DefId, parameters: &ParameterList) {
        for parameter in &parameters.parameters {
            let id = self.define(
                DefinitionKind::Parameter,
                Some(owner),
                parameter.name_span,
                parameter.span,
            );
            self.collect_type_expr(id, &parameter.ty);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::TypeExpr;
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
        assert_eq!(definitions.len(), 10);
        assert_eq!(definitions[0].kind, DefinitionKind::Struct);
        assert_eq!(definitions[1].kind, DefinitionKind::StructField);
        assert_eq!(definitions[2].kind, DefinitionKind::Instance);
        assert_eq!(definitions[3].kind, DefinitionKind::Method);
        assert_eq!(definitions[4].kind, DefinitionKind::Receiver);
        assert_eq!(definitions[5].kind, DefinitionKind::Coercion);
        assert_eq!(definitions[6].kind, DefinitionKind::Receiver);
        assert_eq!(definitions[7].kind, DefinitionKind::ComparisonOperator);
        assert_eq!(definitions[8].kind, DefinitionKind::Receiver);
        assert_eq!(definitions[9].kind, DefinitionKind::Parameter);
        assert_eq!(definitions[3].owner, Some(definitions[2].id));
        assert_eq!(definitions[4].owner, Some(definitions[3].id));
        assert_eq!(definitions[5].owner, Some(definitions[2].id));
        assert_eq!(definitions[6].owner, Some(definitions[5].id));
        assert_eq!(definitions[7].owner, Some(definitions[2].id));
        assert_eq!(definitions[8].owner, Some(definitions[7].id));
        assert_eq!(definitions[9].owner, Some(definitions[7].id));
        assert_eq!(definitions[9].id.raw(), 9);
        for definition in [definitions[3].id, definitions[5].id, definitions[7].id] {
            assert_eq!(
                db.declaration_body_for_owner(definition)
                    .map(|body| body.owner),
                Some(definition)
            );
        }
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
    fn assigns_statement_ids_in_body_source_order() {
        let file = parse_file(
            r#"func main(): void {
    let value = 1
    if true {
        return
    }
    return
}
"#,
        );
        let db = SemanticDb::from_files(std::slice::from_ref(&file));
        let statements = db.statements();
        assert_eq!(statements.len(), 4);
        for (index, statement) in statements.iter().enumerate() {
            assert_eq!(statement.id, StmtId::from_index(index));
            assert_eq!(db.statement_at(statement.span), Some(statement.id));
            assert_eq!(db.statement(statement.id), Some(statement));
            assert_eq!(statement.body, statements[0].body);
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
        assert_eq!(definitions.len(), 3);
        assert_eq!(definitions[0].kind, DefinitionKind::Function);
        assert_eq!(definitions[1].kind, DefinitionKind::Parameter);
        assert_eq!(definitions[1].owner, Some(definitions[0].id));
        assert_eq!(definitions[2].kind, DefinitionKind::Import);
        assert_eq!(definitions[2].owner, Some(definitions[0].id));
        assert_eq!(
            db.definition_at(definitions[2].anchor),
            Some(definitions[2].id)
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
        assert_eq!(
            db.definition_anchor(definition),
            Some(function.member_name_span)
        );
    }

    #[test]
    fn assigns_opaque_results_ids_independent_of_their_source_spans() {
        let file = parse_file(
            r#"interface Source { pub type Item }
func first(): some Source<Item = i32> { loop {} }
func second(): some Source<Item = i32> { loop {} }
"#,
        );
        let (first, second) = match (&file.items[1], &file.items[2]) {
            (Item::Function(first), Item::Function(second)) => {
                (&first.return_type, &second.return_type)
            }
            _ => panic!("expected functions"),
        };
        let TypeExpr::Opaque(first) = first else {
            panic!("expected opaque result");
        };
        let TypeExpr::Opaque(second) = second else {
            panic!("expected opaque result");
        };
        let db = SemanticDb::from_files(std::slice::from_ref(&file));
        let first_id = db.opaque_type_at(first.some_span).unwrap();
        let second_id = db.opaque_type_at(second.some_span).unwrap();
        assert_ne!(first_id, second_id);
        assert_eq!(db.opaque_type_anchor(first_id), Some(first.some_span));
        assert_eq!(db.opaque_type_anchor(second_id), Some(second.some_span));
    }

    #[test]
    fn assigns_nested_authored_bodies_to_their_own_identity_domain() {
        let file = parse_file(
            r#"func main(): i32 {
    let outer = () {
        let inner = () { 1 }
        2
    }
    return 0
}
"#,
        );
        let db = SemanticDb::from_files(std::slice::from_ref(&file));
        let bodies = db.bodies();
        assert_eq!(bodies.len(), 3);
        assert_eq!(bodies[0].kind, BodyKind::Declaration);
        assert_eq!(bodies[0].parent, None);
        assert_eq!(bodies[1].kind, BodyKind::Closure);
        assert_eq!(bodies[1].parent, Some(bodies[0].id));
        assert_eq!(bodies[2].kind, BodyKind::Closure);
        assert_eq!(bodies[2].parent, Some(bodies[1].id));
        assert_eq!(bodies[2].id.raw(), 2);
        for body in bodies {
            assert_eq!(db.body_at(body.anchor), Some(body.id));
            assert_eq!(db.body_anchor(body.id), Some(body.anchor));
        }
        assert!(!db.expressions().is_empty());
        for expression in db.expressions() {
            assert_eq!(db.expression_at(expression.span), Some(expression.id));
            assert!(db.bodies().iter().any(|body| body.id == expression.body));
        }
    }
}
