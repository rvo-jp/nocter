use crate::ast::{AstFile, Block, Expr, Item, Stmt, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;
use crate::typecheck::{TypedHir, normalize_associated_type_expr};

pub(super) fn elaborate_file(
    sources: &SourceMap,
    ast: &mut AstFile,
    resolved: &ResolveOutput,
    facts: &TypedHir,
) -> (Vec<Diagnostic>, bool) {
    let mut diagnostics = Vec::new();
    let mut changed = false;
    for item in &mut ast.items {
        match item {
            Item::Function(function) => {
                changed |= elaborate_callable(
                    sources,
                    &mut function.return_type,
                    function.body.as_ref(),
                    resolved,
                    facts,
                    &mut diagnostics,
                );
            }
            Item::Instance(instance) => {
                for method in instance.callables_mut() {
                    let crate::ast::CallableDecl {
                        return_type, body, ..
                    } = method;
                    changed |= elaborate_callable(
                        sources,
                        return_type,
                        body.as_ref(),
                        resolved,
                        facts,
                        &mut diagnostics,
                    );
                }
            }
            Item::Interface(interface) => {
                for method in &mut interface.methods {
                    let crate::ast::CallableDecl {
                        return_type, body, ..
                    } = &mut method.callable;
                    changed |= elaborate_callable(
                        sources,
                        return_type,
                        body.as_ref(),
                        resolved,
                        facts,
                        &mut diagnostics,
                    );
                }
            }
            Item::Construct(construct) => {
                for member in &mut construct.members {
                    if let crate::ast::ConstructMemberDecl::Function(function) =
                        &mut member.declaration
                    {
                        changed |= elaborate_callable(
                            sources,
                            &mut function.return_type,
                            function.body.as_ref(),
                            resolved,
                            facts,
                            &mut diagnostics,
                        );
                    }
                }
            }
            Item::Import(_)
            | Item::FromImport(_)
            | Item::Test(_)
            | Item::Primitive(_)
            | Item::TypeAlias(_)
            | Item::Struct(_)
            | Item::Enum(_)
            | Item::Destruct(_)
            | Item::Conformance(_) => {}
        }
    }
    (diagnostics, changed)
}

fn elaborate_callable(
    sources: &SourceMap,
    return_type: &mut TypeExpr,
    body: Option<&Block>,
    resolved: &ResolveOutput,
    facts: &TypedHir,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let declared = return_type.clone();
    let Some(opaque) = opaque_payload_mut(return_type) else {
        return false;
    };
    let Some(body) = body else {
        return false;
    };

    let mut expressions = Vec::new();
    collect_return_expressions(body, &mut expressions);
    if let Some(result) = &body.result {
        expressions.push(result.as_ref());
    }

    let mut witness: Option<TypeExpr> = None;
    for expression in expressions {
        let Some(actual) = facts.expression_type_expr(expression.span()) else {
            continue;
        };
        let Some(candidate) = opaque_candidate(&declared, actual) else {
            continue;
        };
        let candidate = normalize_associated_type_expr(candidate, resolved)
            .unwrap_or_else(|| candidate.clone());
        if matches!(candidate, TypeExpr::Opaque(_)) {
            continue;
        }
        if let Some(existing) = &witness {
            if crate::ast::canonical_type_expr(existing)
                != crate::ast::canonical_type_expr(&candidate)
            {
                diagnostics.push(opaque_diagnostic(
                    sources,
                    expression.span(),
                    &format!(
                        "opaque result returns both `{}` and `{}`",
                        crate::ast::canonical_type_expr(existing),
                        crate::ast::canonical_type_expr(&candidate)
                    ),
                    "return one concrete witness type from every reachable result path",
                ));
                return false;
            }
        } else {
            witness = Some(candidate);
        }
    }

    let Some(witness) = witness else {
        diagnostics.push(opaque_diagnostic(
            sources,
            opaque.some_span,
            "could not infer a concrete witness for this opaque result",
            "return one concrete value that conforms to the advertised interface",
        ));
        return false;
    };
    opaque.witness = Some(Box::new(witness));
    true
}

fn opaque_payload_mut(ty: &mut TypeExpr) -> Option<&mut crate::ast::OpaqueType> {
    match ty {
        TypeExpr::Opaque(opaque) => Some(opaque),
        TypeExpr::Optional(optional) => opaque_payload_mut(&mut optional.inner),
        TypeExpr::Fallible(fallible) => opaque_payload_mut(&mut fallible.success),
        _ => None,
    }
}

fn opaque_candidate<'a>(declared: &TypeExpr, actual: &'a TypeExpr) -> Option<&'a TypeExpr> {
    match declared {
        TypeExpr::Opaque(_) => Some(actual),
        TypeExpr::Optional(optional) => match actual {
            TypeExpr::Optional(actual) => opaque_candidate(&optional.inner, &actual.inner),
            TypeExpr::Reference(reference) if reference.name == "none" => None,
            _ => opaque_candidate(&optional.inner, actual),
        },
        TypeExpr::Fallible(fallible) => match actual {
            TypeExpr::Fallible(actual) => opaque_candidate(&fallible.success, &actual.success),
            TypeExpr::Reference(reference) if reference.name == "error" => None,
            _ => opaque_candidate(&fallible.success, actual),
        },
        _ => None,
    }
}

fn collect_return_expressions<'a>(block: &'a Block, expressions: &mut Vec<&'a Expr>) {
    for statement in &block.statements {
        match statement {
            Stmt::Return(statement) => {
                if let Some(expression) = &statement.expression {
                    expressions.push(expression);
                    collect_nested_return_expressions(expression, expressions);
                }
            }
            Stmt::If(statement) => {
                collect_nested_return_expressions(&statement.condition, expressions);
                collect_return_expressions(&statement.then_block, expressions);
                if let Some(block) = &statement.else_block {
                    collect_return_expressions(block, expressions);
                }
            }
            Stmt::IfIs(statement) => {
                collect_nested_return_expressions(&statement.expression, expressions);
                collect_return_expressions(&statement.then_block, expressions);
                if let Some(block) = &statement.else_block {
                    collect_return_expressions(block, expressions);
                }
            }
            Stmt::Switch(statement) => {
                collect_nested_return_expressions(&statement.expression, expressions);
                for arm in &statement.arms {
                    collect_return_expressions(&arm.body, expressions);
                }
                if let Some(arm) = &statement.wildcard_arm {
                    collect_return_expressions(&arm.body, expressions);
                }
            }
            Stmt::ForRange(statement) => collect_return_expressions(&statement.body, expressions),
            Stmt::CollectionFor(statement) => {
                collect_return_expressions(&statement.body, expressions)
            }
            Stmt::LiteralPackFor(statement) => {
                collect_return_expressions(&statement.body, expressions)
            }
            Stmt::While(statement) => collect_return_expressions(&statement.body, expressions),
            Stmt::Loop(statement) => collect_return_expressions(&statement.body, expressions),
            Stmt::Region(statement) => collect_return_expressions(&statement.body, expressions),
            Stmt::Binding(statement) => {
                collect_nested_return_expressions(&statement.initializer, expressions)
            }
            Stmt::Assignment(statement) => {
                collect_nested_return_expressions(&statement.target, expressions);
                collect_nested_return_expressions(&statement.value, expressions);
            }
            Stmt::Expression(statement) => {
                collect_nested_return_expressions(&statement.expression, expressions)
            }
            Stmt::Import(_)
            | Stmt::FromImport(_)
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Drop(_) => {}
        }
    }
}

fn collect_nested_return_expressions<'a>(expression: &'a Expr, expressions: &mut Vec<&'a Expr>) {
    match expression {
        Expr::Closure(_) => {}
        Expr::Catch(expression) => {
            collect_nested_return_expressions(&expression.expression, expressions);
            collect_return_expressions(&expression.catch_block, expressions);
        }
        Expr::Otherwise(expression) => {
            collect_nested_return_expressions(&expression.value, expressions);
            collect_return_expressions(&expression.fallback, expressions);
        }
        Expr::If(statement) => {
            collect_return_expressions(&statement.then_block, expressions);
            if let Some(block) = &statement.else_block {
                collect_return_expressions(block, expressions);
            }
        }
        Expr::IfIs(statement) => {
            collect_return_expressions(&statement.then_block, expressions);
            if let Some(block) = &statement.else_block {
                collect_return_expressions(block, expressions);
            }
        }
        Expr::Match(statement) => {
            for arm in &statement.arms {
                collect_return_expressions(&arm.body, expressions);
            }
            if let Some(arm) = &statement.wildcard_arm {
                collect_return_expressions(&arm.body, expressions);
            }
        }
        Expr::Propagate(expression) => {
            collect_nested_return_expressions(&expression.expression, expressions)
        }
        Expr::Force(expression) => {
            collect_nested_return_expressions(&expression.expression, expressions)
        }
        Expr::Borrow(expression) => {
            collect_nested_return_expressions(&expression.expression, expressions)
        }
        Expr::Unary(expression) => {
            collect_nested_return_expressions(&expression.operand, expressions)
        }
        Expr::Binary(expression) => {
            collect_nested_return_expressions(&expression.left, expressions);
            collect_nested_return_expressions(&expression.right, expressions);
        }
        Expr::TypeConversion(expression) => {
            collect_nested_return_expressions(&expression.expression, expressions)
        }
        Expr::Call(expression) => {
            collect_nested_return_expressions(&expression.callee, expressions);
            for argument in &expression.arguments {
                collect_nested_return_expressions(argument, expressions);
            }
        }
        Expr::Member(expression) => {
            collect_nested_return_expressions(&expression.object, expressions)
        }
        Expr::Index(expression) => {
            collect_nested_return_expressions(&expression.object, expressions);
            collect_nested_return_expressions(&expression.index, expressions);
        }
        Expr::Group(expression) => {
            collect_nested_return_expressions(&expression.expression, expressions)
        }
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let crate::ast::InterpolatedStringPart::Expression(part) = part {
                    collect_nested_return_expressions(&part.expression, expressions);
                }
            }
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                collect_nested_return_expressions(element, expressions);
            }
        }
        Expr::TypedSequenceLiteral(expression) => {
            for element in &expression.elements {
                collect_nested_return_expressions(element, expressions);
            }
        }
        Expr::StructLiteral(expression) => {
            for field in &expression.fields {
                collect_nested_return_expressions(&field.value, expressions);
            }
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::TypedStringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}

fn opaque_diagnostic(
    sources: &SourceMap,
    span: crate::source::ByteSpan,
    message: &str,
    help: &str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error("E0459", message);
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(help.to_string());
    diagnostic
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{analyze_import_text, analyze_text};

    const SUPPORT: &str = r#"
interface Value {
    pub type Item
    pub method &self.get(): Self.Item
}

struct Box<T> {
    value: T
}

conform Value for Box<T> {
    type Item = T

    method &self.get(): T {
        return self.value
    }
}
"#;

    #[test]
    fn elaborates_one_static_opaque_witness() {
        let source = format!(
            "{SUPPORT}\nfunc make(): some Value<Item = i32> {{\n    return Box<i32> {{ value: 7 }}\n}}\n\nfunc read(): i32 {{\n    let value = make()\n    return value.get()\n}}\n"
        );
        let (_, analysis) = analyze_text(&source);
        assert!(
            analysis.diagnostics().is_empty(),
            "{:?}",
            analysis.diagnostics()
        );
        assert!(
            analysis.files.iter().all(|file| std::sync::Arc::ptr_eq(
                &analysis.semantic_db,
                &file.resolved.semantic_db
            ))
        );
        let function = analysis
            .root_file()
            .unwrap()
            .ast
            .items
            .iter()
            .find_map(|item| {
                let crate::ast::Item::Function(function) = item else {
                    return None;
                };
                (function.name == "make").then_some(function)
            })
            .unwrap();
        let crate::ast::TypeExpr::Opaque(opaque) = &function.return_type else {
            panic!("expected opaque return");
        };
        assert_eq!(
            opaque
                .witness
                .as_deref()
                .map(crate::ast::canonical_type_expr)
                .as_deref(),
            Some("Box<i32>")
        );
    }

    #[test]
    fn rejects_different_opaque_witnesses_across_return_paths() {
        let source = format!(
            "{SUPPORT}\nfunc make(flag: bool): some Value<Item = i32> {{\n    if flag {{\n        return Box<i32> {{ value: 7 }}\n    }}\n    return Other {{ value: 9 }}\n}}\n\nstruct Other {{ value: i32 }}\nconform Value for Other {{\n    type Item = i32\n    method &self.get(): i32 {{ return self.value }}\n}}\n"
        );
        let (_, analysis) = analyze_text(&source);
        assert!(
            analysis.diagnostics().iter().any(|diagnostic| {
                diagnostic.code == "E0459" && diagnostic.message.contains("returns both")
            }),
            "{:?}",
            analysis.diagnostics()
        );
    }

    #[test]
    fn elaborates_generic_and_optional_opaque_results() {
        let source = format!(
            "{SUPPORT}\nfunc make<T>(value: T, present: bool): some Value<Item = T>? {{\n    if !present {{\n        return none\n    }}\n    return Box<T> {{ value: value }}\n}}\n\nfunc read(): i32 {{\n    let value = make(7, true) otherwise {{ return 0 }}\n    return value.get()\n}}\n"
        );
        let (_, analysis) = analyze_text(&source);
        assert!(
            analysis.diagnostics().is_empty(),
            "{:?}",
            analysis.diagnostics()
        );
    }

    #[test]
    fn rejects_nonconforming_witness_and_wrong_associated_binding() {
        let source = format!(
            "{SUPPORT}\nstruct Plain {{ value: i32 }}\nfunc unconformed(): some Value<Item = i32> {{\n    return Plain {{ value: 7 }}\n}}\nfunc wrong_item(): some Value<Item = bool> {{\n    return Box<i32> {{ value: 7 }}\n}}\n"
        );
        let (_, analysis) = analyze_text(&source);
        assert!(
            analysis.diagnostics().iter().any(|diagnostic| {
                diagnostic.code == "E0459" && diagnostic.message.contains("does not conform")
            }),
            "{:?}",
            analysis.diagnostics()
        );
        assert!(
            analysis.diagnostics().iter().any(|diagnostic| {
                diagnostic.code == "E0459" && diagnostic.message.contains("binds `Item`")
            }),
            "{:?}",
            analysis.diagnostics()
        );
    }

    #[test]
    fn rejects_opaque_types_outside_body_bearing_results() {
        let source = format!(
            "{SUPPORT}\nstruct Holder {{ value: some Value<Item = i32> }}\nfunc consume(value: some Value<Item = i32>): void {{ return }}\ninterface Factory {{\n    pub method &self.make(): some Value<Item = i32>\n}}\n"
        );
        let (_, analysis) = analyze_text(&source);
        let unsupported = analysis
            .diagnostics()
            .iter()
            .filter(|diagnostic| {
                diagnostic.code == "E0459"
                    && diagnostic.message.contains("body-bearing callable results")
            })
            .count();
        assert!(unsupported >= 3, "{:?}", analysis.diagnostics());
    }

    #[test]
    fn imported_opaque_result_keeps_static_dispatch_and_hides_witness() {
        let root = r#"use lib/math

func read(): i32 {
    let value = math.make()
    return value.get()
}
"#;
        let module = r#"pub interface Value {
    pub type Item
    pub method &self.get(): Self.Item
}
struct Hidden { value: i32 }
conform Value for Hidden {
    type Item = i32
    method &self.get(): i32 { return self.value }
}
pub func make(): some Value<Item = i32> {
    return Hidden { value: 7 }
}
"#;
        let (_, analysis) = analyze_import_text(root, module);
        assert!(
            analysis.diagnostics().is_empty(),
            "{:?}",
            analysis.diagnostics()
        );
    }

    #[test]
    fn opaque_contract_stays_move_only_when_witness_is_copyable() {
        let source = r#"interface Value {
    pub method &self.get(): i32
}
copy struct CopyBox { value: i32 }
conform Value for CopyBox {
    method &self.get(): i32 { return self.value }
}
func make(): some Value { return CopyBox { value: 7 } }
func invalid(): i32 {
    let first = make()
    let second = first
    return first.get()
}
"#;
        let (_, analysis) = analyze_text(source);
        assert!(
            analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message.contains("move")),
            "{:?}",
            analysis.diagnostics()
        );
    }

    #[test]
    fn separate_declarations_have_distinct_opaque_identities() {
        let source = format!(
            "{SUPPORT}\nfunc first(): some Value<Item = i32> {{ return Box<i32> {{ value: 1 }} }}\nfunc second(): some Value<Item = i32> {{ return Box<i32> {{ value: 2 }} }}\nfunc invalid(): i32 {{\n    var value = first()\n    value = second()\n    return value.get()\n}}\n"
        );
        let (_, analysis) = analyze_text(&source);
        assert!(
            analysis.diagnostics().iter().any(|diagnostic| {
                diagnostic.message.contains("cannot assign")
                    || diagnostic.message.contains("does not match")
            }),
            "{:?}",
            analysis.diagnostics()
        );
    }

    #[test]
    fn witness_only_members_are_not_part_of_the_opaque_surface() {
        let source = format!(
            "{SUPPORT}\ninstance Box<T> {{\n    method &self.hidden(): T {{ return self.value }}\n}}\nfunc make(): some Value<Item = i32> {{ return Box<i32> {{ value: 7 }} }}\nfunc invalid(): i32 {{\n    let value = make()\n    return value.hidden()\n}}\n"
        );
        let (_, analysis) = analyze_text(&source);
        assert!(
            analysis.diagnostics().iter().any(|diagnostic| {
                diagnostic.message.contains("hidden")
                    && (diagnostic.message.contains("method")
                        || diagnostic.message.contains("member"))
            }),
            "{:?}",
            analysis.diagnostics()
        );
    }
}
