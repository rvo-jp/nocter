//! Analysis-time index of lexical local and block-import visibility.

use crate::ast::{AstFile, Block, ConformanceMember, Expr, InterpolatedStringPart, Item, Stmt};
use crate::semantic::{BodyId, SemanticDb};
use crate::source::ByteSpan;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct VisibleLocalBinding {
    pub(super) name: String,
    pub(super) name_span: ByteSpan,
    pub(super) kind: &'static str,
}

#[derive(Debug, Clone)]
struct LocalVisibility {
    body: BodyId,
    visible: ByteSpan,
    start_is_exclusive: bool,
    binding: VisibleLocalBinding,
}

#[derive(Debug, Clone)]
struct ImportVisibility {
    visible: ByteSpan,
    name_spans: Vec<ByteSpan>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LexicalScopeIndex {
    locals: Vec<LocalVisibility>,
    imports: Vec<ImportVisibility>,
    import_name_spans: HashSet<ByteSpan>,
}

impl LexicalScopeIndex {
    pub(crate) fn new(ast: &AstFile, semantic_db: &SemanticDb) -> Self {
        let mut builder = Builder {
            semantic_db,
            index: Self::default(),
        };
        builder.collect_file(ast);
        builder.index
    }

    pub(super) fn visible_locals(
        &self,
        semantic_db: &SemanticDb,
        source: crate::source::SourceId,
        offset: usize,
    ) -> Vec<VisibleLocalBinding> {
        let Some(body) = semantic_db
            .body_containing(source, offset)
            .map(|body| body.id)
        else {
            return Vec::new();
        };
        let mut candidates = self
            .locals
            .iter()
            .filter(|local| {
                local.body == body
                    && contains_or_touches(local.visible, offset)
                    && (!local.start_is_exclusive || local.visible.start < offset)
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|local| (local.visible.start, local.binding.name_span.start));
        let mut visible = Vec::<VisibleLocalBinding>::new();
        for candidate in candidates {
            visible.retain(|local| local.name != candidate.binding.name);
            visible.push(candidate.binding.clone());
        }
        visible
    }

    pub(super) fn visible_imports(&self, offset: usize) -> HashSet<ByteSpan> {
        self.imports
            .iter()
            .filter(|import| contains(import.visible, offset))
            .flat_map(|import| import.name_spans.iter().copied())
            .collect()
    }

    pub(super) fn import_name_spans(&self) -> &HashSet<ByteSpan> {
        &self.import_name_spans
    }
}

struct Builder<'a> {
    semantic_db: &'a SemanticDb,
    index: LexicalScopeIndex,
}

impl Builder<'_> {
    fn collect_file(&mut self, ast: &AstFile) {
        for item in &ast.items {
            match item {
                Item::Function(function) => {
                    if let Some(body) = &function.body {
                        let body_id = self.body_id(body.span);
                        for parameter in &function.parameters.parameters {
                            self.local(
                                body_id,
                                body.span,
                                &parameter.name,
                                parameter.name_span,
                                "parameter",
                            );
                        }
                        self.collect_block(body_id, body);
                    }
                }
                Item::Test(test) => {
                    let body_id = self.body_id(test.body.span);
                    self.collect_block(body_id, &test.body);
                }
                Item::Instance(instance) => {
                    for callable in instance.callables() {
                        self.collect_callable(callable);
                    }
                }
                Item::Conformance(conformance) => {
                    for member in &conformance.members {
                        if let ConformanceMember::Method(method) = member {
                            self.collect_callable(method);
                        }
                    }
                }
                Item::Interface(interface) => {
                    for method in &interface.methods {
                        self.collect_callable(method);
                    }
                }
                Item::Destruct(destruct) => {
                    let body_id = self.body_id(destruct.body.span);
                    self.local(
                        body_id,
                        destruct.body.span,
                        &destruct.binding.name,
                        destruct.binding.name_span,
                        "parameter",
                    );
                    self.collect_block(body_id, &destruct.body);
                }
                Item::Construct(construct) => {
                    for (_, function) in construct.functions() {
                        if let Some(body) = &function.body {
                            let body_id = self.body_id(body.span);
                            for parameter in &function.parameters.parameters {
                                self.local(
                                    body_id,
                                    body.span,
                                    &parameter.name,
                                    parameter.name_span,
                                    "parameter",
                                );
                            }
                            self.collect_block(body_id, body);
                        }
                    }
                    for (_, literal) in construct.literals() {
                        if let Some(body) = &literal.body {
                            let body_id = self.body_id(body.span);
                            for parameter in &literal.parameters.parameters {
                                self.local(
                                    body_id,
                                    body.span,
                                    &parameter.name,
                                    parameter.name_span,
                                    "parameter",
                                );
                            }
                            if let Some(capture) = &literal.capture {
                                self.local(
                                    body_id,
                                    body.span,
                                    &capture.name,
                                    capture.name_span,
                                    "literal capture",
                                );
                            }
                            self.collect_block(body_id, body);
                        }
                    }
                }
                Item::Import(_)
                | Item::FromImport(_)
                | Item::Primitive(_)
                | Item::TypeAlias(_)
                | Item::Struct(_)
                | Item::Enum(_) => {}
            }
        }
    }

    fn collect_callable(&mut self, callable: &crate::ast::CallableDecl) {
        let Some(body) = &callable.body else {
            return;
        };
        let body_id = self.body_id(body.span);
        self.local(
            body_id,
            body.span,
            &callable.receiver.name,
            callable.receiver.name_span,
            "parameter",
        );
        for parameter in &callable.parameters.parameters {
            self.local(
                body_id,
                body.span,
                &parameter.name,
                parameter.name_span,
                "parameter",
            );
        }
        self.collect_block(body_id, body);
    }

    fn collect_block(&mut self, body_id: BodyId, block: &Block) {
        for statement in &block.statements {
            match statement {
                Stmt::Import(import) => {
                    self.import(block.span, import.span.end, vec![import.alias.span])
                }
                Stmt::FromImport(import) => self.import(
                    block.span,
                    import.span.end,
                    import.names.iter().map(|name| name.local_span()).collect(),
                ),
                Stmt::Binding(binding) => self.local_after(
                    body_id,
                    ByteSpan::new(block.span.source, binding.span.end, block.span.end),
                    &binding.name,
                    binding.name_span,
                    match binding.kind {
                        crate::ast::BindingKind::Let => "let",
                        crate::ast::BindingKind::Var => "var",
                    },
                ),
                _ => {}
            }
            self.collect_statement(body_id, statement);
        }
        if let Some(result) = &block.result {
            self.collect_expression(body_id, result);
        }
    }

    fn collect_statement(&mut self, body_id: BodyId, statement: &Stmt) {
        match statement {
            Stmt::Return(statement) => {
                if let Some(expression) = &statement.expression {
                    self.collect_expression(body_id, expression);
                }
            }
            Stmt::Binding(statement) => self.collect_expression(body_id, &statement.initializer),
            Stmt::Assignment(statement) => {
                self.collect_expression(body_id, &statement.target);
                self.collect_expression(body_id, &statement.value);
            }
            Stmt::If(statement) => {
                self.collect_expression(body_id, &statement.condition);
                self.collect_block(body_id, &statement.then_block);
                if let Some(block) = &statement.else_block {
                    self.collect_block(body_id, block);
                }
            }
            Stmt::IfIs(statement) => {
                self.collect_expression(body_id, &statement.expression);
                self.payload(
                    body_id,
                    statement.then_block.span,
                    statement.payload.as_ref(),
                );
                self.collect_block(body_id, &statement.then_block);
                if let Some(block) = &statement.else_block {
                    self.collect_block(body_id, block);
                }
            }
            Stmt::Switch(statement) => {
                self.collect_expression(body_id, &statement.expression);
                for arm in &statement.arms {
                    self.payload(body_id, arm.body.span, arm.payload.as_ref());
                    self.collect_block(body_id, &arm.body);
                }
                if let Some(arm) = &statement.wildcard_arm {
                    self.collect_block(body_id, &arm.body);
                }
            }
            Stmt::ForRange(statement) => {
                self.collect_expression(body_id, &statement.start);
                self.collect_expression(body_id, &statement.end);
                self.local(
                    body_id,
                    statement.body.span,
                    &statement.name,
                    statement.name_span,
                    "range",
                );
                self.collect_block(body_id, &statement.body);
            }
            Stmt::CollectionFor(statement) => {
                self.collect_expression(body_id, &statement.source);
                self.local(
                    body_id,
                    statement.body.span,
                    &statement.name,
                    statement.name_span,
                    "collection element",
                );
                self.collect_block(body_id, &statement.body);
            }
            Stmt::LiteralPackFor(statement) => {
                self.local(
                    body_id,
                    statement.body.span,
                    &statement.name,
                    statement.name_span,
                    "literal pack element",
                );
                self.collect_block(body_id, &statement.body);
            }
            Stmt::While(statement) => {
                self.collect_expression(body_id, &statement.condition);
                self.collect_block(body_id, &statement.body);
            }
            Stmt::Loop(statement) => self.collect_block(body_id, &statement.body),
            Stmt::Region(statement) => {
                self.collect_expression(body_id, &statement.allocator);
                self.local(
                    body_id,
                    statement.body.span,
                    &statement.name,
                    statement.name_span,
                    "region",
                );
                self.collect_block(body_id, &statement.body);
            }
            Stmt::Expression(statement) => self.collect_expression(body_id, &statement.expression),
            Stmt::Import(_)
            | Stmt::FromImport(_)
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Drop(_) => {}
        }
    }

    fn collect_expression(&mut self, body_id: BodyId, expression: &Expr) {
        match expression {
            Expr::Closure(closure) => {
                let closure_id = self.body_id(closure.span);
                for capture in &closure.captures {
                    self.local(
                        closure_id,
                        closure.body.span,
                        &capture.name,
                        capture.name_span,
                        "capture",
                    );
                }
                for parameter in &closure.parameters {
                    self.local(
                        closure_id,
                        closure.body.span,
                        &parameter.name,
                        parameter.name_span,
                        "parameter",
                    );
                }
                self.collect_block(closure_id, &closure.body);
            }
            Expr::Catch(expression) => {
                self.collect_expression(body_id, &expression.expression);
                if let crate::ast::CatchBinding::Named { name, span } = &expression.binding {
                    self.local(body_id, expression.catch_block.span, name, *span, "error");
                }
                self.collect_block(body_id, &expression.catch_block);
            }
            Expr::Otherwise(expression) => {
                self.collect_expression(body_id, &expression.value);
                self.collect_block(body_id, &expression.fallback);
            }
            Expr::If(expression) => {
                self.collect_expression(body_id, &expression.condition);
                self.collect_block(body_id, &expression.then_block);
                if let Some(block) = &expression.else_block {
                    self.collect_block(body_id, block);
                }
            }
            Expr::IfIs(expression) => {
                self.collect_expression(body_id, &expression.expression);
                self.payload(
                    body_id,
                    expression.then_block.span,
                    expression.payload.as_ref(),
                );
                self.collect_block(body_id, &expression.then_block);
                if let Some(block) = &expression.else_block {
                    self.collect_block(body_id, block);
                }
            }
            Expr::Match(expression) => {
                self.collect_expression(body_id, &expression.expression);
                for arm in &expression.arms {
                    self.payload(body_id, arm.body.span, arm.payload.as_ref());
                    self.collect_block(body_id, &arm.body);
                }
                if let Some(arm) = &expression.wildcard_arm {
                    self.collect_block(body_id, &arm.body);
                }
            }
            Expr::InterpolatedString(expression) => {
                for part in &expression.parts {
                    if let InterpolatedStringPart::Expression(part) = part {
                        self.collect_expression(body_id, &part.expression);
                    }
                }
            }
            Expr::ArrayLiteral(expression) => {
                for element in &expression.elements {
                    self.collect_expression(body_id, element);
                }
            }
            Expr::TypedSequenceLiteral(expression) => {
                for element in &expression.elements {
                    self.collect_expression(body_id, element);
                }
                if let Some(using) = &expression.using {
                    self.collect_expression(body_id, &using.allocator);
                }
            }
            Expr::TypedStringLiteral(expression) => {
                if let Some(using) = &expression.using {
                    self.collect_expression(body_id, &using.allocator);
                }
            }
            Expr::StructLiteral(expression) => {
                for field in &expression.fields {
                    self.collect_expression(body_id, &field.value);
                }
            }
            Expr::Propagate(expression) => self.collect_expression(body_id, &expression.expression),
            Expr::Force(expression) => self.collect_expression(body_id, &expression.expression),
            Expr::Borrow(expression) => self.collect_expression(body_id, &expression.expression),
            Expr::Unary(expression) => self.collect_expression(body_id, &expression.operand),
            Expr::Binary(expression) => {
                self.collect_expression(body_id, &expression.left);
                self.collect_expression(body_id, &expression.right);
            }
            Expr::TypeConversion(expression) => {
                self.collect_expression(body_id, &expression.expression)
            }
            Expr::Call(expression) => {
                self.collect_expression(body_id, &expression.callee);
                for argument in &expression.arguments {
                    self.collect_expression(body_id, argument);
                }
            }
            Expr::Member(expression) => self.collect_expression(body_id, &expression.object),
            Expr::Index(expression) => {
                self.collect_expression(body_id, &expression.object);
                self.collect_expression(body_id, &expression.index);
            }
            Expr::Group(expression) => self.collect_expression(body_id, &expression.expression),
            Expr::Identifier(_)
            | Expr::IntegerLiteral(_)
            | Expr::ByteLiteral(_)
            | Expr::StringLiteral(_)
            | Expr::BoolLiteral(_)
            | Expr::NoneLiteral(_) => {}
        }
    }

    fn body_id(&self, location: ByteSpan) -> BodyId {
        self.semantic_db
            .body_at(location)
            .unwrap_or_else(|| panic!("semantic database omitted lexical body at {location:?}"))
    }

    fn local(
        &mut self,
        body: BodyId,
        visible: ByteSpan,
        name: &str,
        name_span: ByteSpan,
        kind: &'static str,
    ) {
        self.push_local(body, visible, false, name, name_span, kind);
    }

    fn local_after(
        &mut self,
        body: BodyId,
        visible: ByteSpan,
        name: &str,
        name_span: ByteSpan,
        kind: &'static str,
    ) {
        self.push_local(body, visible, true, name, name_span, kind);
    }

    fn push_local(
        &mut self,
        body: BodyId,
        visible: ByteSpan,
        start_is_exclusive: bool,
        name: &str,
        name_span: ByteSpan,
        kind: &'static str,
    ) {
        self.index.locals.push(LocalVisibility {
            body,
            visible,
            start_is_exclusive,
            binding: VisibleLocalBinding {
                name: name.to_string(),
                name_span,
                kind,
            },
        });
    }

    fn payload(
        &mut self,
        body: BodyId,
        visible: ByteSpan,
        payload: Option<&crate::ast::SwitchPayloadPattern>,
    ) {
        if let Some(binding) = payload.and_then(|payload| payload.binding()) {
            self.local(body, visible, &binding.name, binding.span, "payload");
        }
    }

    fn import(&mut self, block: ByteSpan, visible_from: usize, name_spans: Vec<ByteSpan>) {
        self.index
            .import_name_spans
            .extend(name_spans.iter().copied());
        self.index.imports.push(ImportVisibility {
            visible: ByteSpan::new(block.source, visible_from, block.end),
            name_spans,
        });
    }
}

const fn contains_or_touches(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset <= span.end
}

const fn contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

#[cfg(test)]
mod tests {
    use crate::analysis::scoped_imports::recovery_visible_scoped_import_spans_at_offset;
    use crate::analysis::test_support::analyze_text;
    use crate::analysis::visible_locals::recovery_visible_local_bindings_at_offset;

    #[test]
    fn indexed_visibility_matches_the_complete_syntax_walk() {
        let text = r#"func main(input: i32): i32 {
    use std/io.print
    let outer = input
    if true {
        use std/io.read_line
        let inner = outer
        return inner
    }
    let transform = (&outer; value: i32): i32 {
        let nested = value
        return nested + outer
    }
    return transform(outer)
}
"#;
        let (_, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("root file");

        for offset in 0..=text.len() {
            let indexed_locals = file
                .lexical_scopes
                .visible_locals(&file.resolved.semantic_db, file.ast.span.source, offset)
                .into_iter()
                .map(|binding| (binding.name, binding.name_span, binding.kind))
                .collect::<Vec<_>>();
            let walked_locals = recovery_visible_local_bindings_at_offset(&file.ast, offset)
                .into_iter()
                .map(|binding| (binding.name, binding.name_span, binding.kind))
                .collect::<Vec<_>>();
            assert_eq!(indexed_locals, walked_locals, "local mismatch at {offset}");

            assert_eq!(
                file.lexical_scopes.visible_imports(offset),
                recovery_visible_scoped_import_spans_at_offset(&file.ast, offset),
                "import mismatch at {offset}"
            );
        }
    }

    #[test]
    fn a_binding_is_not_visible_inside_or_at_the_end_of_its_declaration() {
        let text = "func main(): i32 {\n    let value = 1\n    return value\n}\n";
        let (_, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("root file");
        let declaration_end = text.find("1\n").expect("initializer") + 1;
        let reference = text.rfind("value").expect("reference");

        let names_at_declaration_end = file
            .lexical_scopes
            .visible_locals(
                &file.resolved.semantic_db,
                file.ast.span.source,
                declaration_end,
            )
            .into_iter()
            .map(|binding| binding.name)
            .collect::<Vec<_>>();
        assert!(!names_at_declaration_end.iter().any(|name| name == "value"));

        let names_at_reference = file
            .lexical_scopes
            .visible_locals(&file.resolved.semantic_db, file.ast.span.source, reference)
            .into_iter()
            .map(|binding| binding.name)
            .collect::<Vec<_>>();
        assert!(names_at_reference.iter().any(|name| name == "value"));
    }
}
