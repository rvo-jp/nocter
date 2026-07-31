use super::*;
use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::resolve;
use crate::source::SourceMap;

#[test]
fn type_hover_label_shortens_hidden_canonical_names() {
    let resolved = resolve_text("func main(): i32 {\n    return 0\n}\n");

    assert_eq!(
        type_hover_label(&Type::Named("std/string.String".to_string()), &resolved),
        "String"
    );
    assert_eq!(
        type_hover_label(
            &Type::Generic {
                name: "std/vec.Vec".to_string(),
                arguments: vec![Type::Named("std/string.String".to_string())],
            },
            &resolved,
        ),
        "Vec<String>"
    );
}

#[test]
fn records_method_receiver_kind_facts() {
    let (ast, resolved) = parse_and_resolve_text(
        r#"struct Box {
    value: i32
}

impl Box {
    method self.take(): i32 {
        return self.value
    }

    method &self.read(): i32 {
        return self.value
    }

    method &+self.write(): void {
        self.value = 2
        return
    }
}

func main(): i32 {
    var box = Box { value: 1 }
    box.write()
    let copy = Box { value: 2 }
    return copy.read() + Box { value: 3 }.take()
}
"#,
    );
    let facts = collect_typecheck_facts(&ast, &resolved);
    let receiver_kinds = facts
        .method_call_spans()
        .filter_map(|span| facts.method_call_receiver_kind(span))
        .collect::<Vec<_>>();

    assert!(receiver_kinds.contains(&TypecheckMethodReceiverKind::Owned));
    assert!(receiver_kinds.contains(&TypecheckMethodReceiverKind::ReadonlyBorrow));
    assert!(receiver_kinds.contains(&TypecheckMethodReceiverKind::ReadwriteBorrow));
}

#[test]
fn records_binding_type_expr_facts_for_generic_parameters() {
    let text = r#"func keep<T>(value: T): T {
    let inferred = value
    return inferred
}
"#;
    let (ast, resolved) = parse_and_resolve_text(text);
    let facts = collect_typecheck_facts(&ast, &resolved);
    let start = text.find("inferred").expect("expected binding name");
    let span = ByteSpan::new(ast.span.source, start, start + "inferred".len());

    let Some(TypeExpr::Reference(reference)) = facts.binding_type_expr(span) else {
        panic!("expected inferred binding type expr for generic parameter");
    };
    assert_eq!(reference.name, "T");
}

#[test]
fn records_payload_binding_copy_and_move_modes() {
    let text = r#"struct Detail {
    code: i32
}

enum Result {
    code(value: i32)
    detail(value: Detail)
}

func inspect(result: Result): i32 {
    match move result {
        Result.code(code) { return code }
        Result.detail(detail) { return detail.code }
    }
}
"#;
    let (ast, resolved) = parse_and_resolve_text(text);
    let facts = collect_typecheck_facts(&ast, &resolved);
    let code_span = identifier_span(&ast, text, "code) { return", "code");
    let detail_span = identifier_span(&ast, text, "detail) { return", "detail");

    assert_eq!(
        facts.payload_binding_mode(code_span),
        Some(TypecheckPayloadBindingMode::Copy)
    );
    assert_eq!(
        facts.payload_binding_mode(detail_span),
        Some(TypecheckPayloadBindingMode::Move)
    );
}

#[test]
fn records_expression_type_expr_facts() {
    let text = r#"enum Choice {
    yes
    no
}

func main(choice: Choice): i32 {
    let code = match choice {
        _ { 1 }
    }
    return code
}
"#;
    let (ast, resolved) = parse_and_resolve_text(text);
    let facts = collect_typecheck_facts(&ast, &resolved);
    let function = ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "main" => Some(function),
            _ => None,
        })
        .expect("expected main function");
    let Stmt::Binding(binding) = &function.body.statements[0] else {
        panic!("expected match binding");
    };
    let Expr::Match(match_expression) = binding.initializer.without_groups() else {
        panic!("expected match expression initializer");
    };

    let Some(TypeExpr::Reference(reference)) =
        facts.expression_type_expr(match_expression.expression.span())
    else {
        panic!("expected expression type expr fact");
    };
    assert_eq!(reference.name, "Choice");
}

#[test]
fn records_enum_pattern_variant_reference_facts() {
    let text = r#"enum Choice {
    hit(value: i32)
    miss(value: i32)
}

func main(choice: Choice): i32 {
    if choice is Choice.hit(_) {
    }
    let code = match choice {
        Choice.hit(_) { 1 }
        Choice.miss(_) { 2 }
    }
    return code
}
"#;
    let (ast, resolved) = parse_and_resolve_text(text);
    let facts = collect_typecheck_facts(&ast, &resolved);
    let hit_declaration = identifier_span(&ast, text, "hit(value", "hit");
    let miss_declaration = identifier_span(&ast, text, "miss(value", "miss");

    for start in [
        text.find("hit(_)").expect("expected if-is hit pattern"),
        text.rfind("hit(_)").expect("expected match hit pattern"),
    ] {
        let span = ByteSpan::new(ast.span.source, start, start + "hit".len());
        assert_eq!(facts.enum_variant_target(span), Some(hit_declaration));
    }

    let miss_start = text.rfind("miss(_)").expect("expected match miss pattern");
    let miss_span = ByteSpan::new(ast.span.source, miss_start, miss_start + "miss".len());
    assert_eq!(facts.enum_variant_target(miss_span), Some(miss_declaration));

    let discard_start = text.find("_)").expect("expected discard payload");
    let discard_span = ByteSpan::new(ast.span.source, discard_start, discard_start + 1);
    assert_eq!(facts.enum_variant_target(discard_span), None);
}

#[test]
fn records_concrete_field_type_expr_facts_for_generic_struct_fields() {
    let text = r#"copy struct Box<T> {
    values: [T; 2]
}

func main(): i32 {
    let box = Box<i32> { values: [1, 2] }
    return box.values[0]
}
"#;
    let (ast, resolved) = parse_and_resolve_text(text);
    let facts = collect_typecheck_facts(&ast, &resolved);
    let literal_start = text.find("values: [1, 2]").expect("expected literal field");
    let literal_span = ByteSpan::new(
        ast.span.source,
        literal_start,
        literal_start + "values".len(),
    );
    let member_start = text.rfind("values[0]").expect("expected member field");
    let member_span = ByteSpan::new(ast.span.source, member_start, member_start + "values".len());

    assert_concrete_i32_pair_type_expr(facts.field_type_expr(literal_span));
    assert_concrete_i32_pair_type_expr(facts.field_type_expr(member_span));
}

fn assert_concrete_i32_pair_type_expr(ty: Option<&TypeExpr>) {
    let Some(TypeExpr::Array(array)) = ty else {
        panic!("expected concrete fixed array field type expr");
    };
    let TypeExpr::Reference(element) = array.element.as_ref() else {
        panic!("expected fixed array element type");
    };
    assert_eq!(element.name, "i32");
    assert_eq!(array.length.value, "2");
}

fn identifier_span(ast: &AstFile, text: &str, needle: &str, identifier: &str) -> ByteSpan {
    let start = text.find(needle).expect("expected identifier");
    ByteSpan::new(ast.span.source, start, start + identifier.len())
}

fn resolve_text(text: &str) -> ResolveOutput {
    parse_and_resolve_text(text).1
}

fn parse_and_resolve_text(text: &str) -> (AstFile, ResolveOutput) {
    let mut sources = SourceMap::new();
    let source = sources.add_source("test.nct", None, text.to_string());
    let lex_output = lex(&sources, source);
    assert!(
        lex_output.diagnostics.is_empty(),
        "unexpected lex diagnostics: {:?}",
        lex_output.diagnostics
    );
    let parse_output = parse(&sources, source, &lex_output.tokens);
    assert!(
        parse_output.diagnostics.is_empty(),
        "unexpected parse diagnostics: {:?}",
        parse_output.diagnostics
    );
    let ast = parse_output.ast.expect("expected ast");
    let resolved = resolve(&sources, &ast);
    (ast, resolved)
}
