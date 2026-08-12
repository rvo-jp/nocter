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

instance Box {
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
fn records_declared_index_method_as_the_selected_operation() {
    let text = r#"struct Buffer {
    values: [i32; 1]
}

instance Buffer {
    operator (&self[index: usize]): &i32 {
        return &self.values[0]
    }
}

func read(buffer: &Buffer, index: usize): i32 {
    return buffer[index]
}
"#;
    let (ast, resolved) = parse_and_resolve_text(text);
    let facts = collect_typecheck_facts(&ast, &resolved);
    let start = text.rfind("buffer[index]").expect("index expression");
    let span = ByteSpan::new(ast.span.source, start, start + "buffer[index]".len());
    let plan = facts.index_plan(span).expect("index plan");
    assert_eq!(plan.projection, TypecheckIndexProjection::Declared);
    let method = plan.method.as_ref().expect("declared index method");
    assert_eq!(
        method.method_name,
        crate::ast::READONLY_INDEX_OPERATOR_METHOD_NAME
    );
    assert_eq!(
        resolved.semantic_db.definition_at(method.declaration_span),
        Some(method.def_id)
    );
}

#[test]
fn records_generic_function_specialization_with_definition_identity() {
    let (ast, resolved) = parse_and_resolve_text(
        r#"func identity<T>(value: T): T {
    return value
}

func main(): i32 {
    return identity(1)
}
"#,
    );
    let facts = collect_typecheck_facts(&ast, &resolved);
    let specialization = facts
        .function_call_specializations()
        .next()
        .expect("generic function specialization");
    assert_eq!(
        resolved
            .semantic_db
            .definition_at(specialization.declaration_span),
        Some(specialization.def_id)
    );
}

#[test]
fn records_generic_destructor_specialization_with_definition_identity() {
    let (ast, resolved) = parse_and_resolve_text(
        r#"struct Box<T> {
    value: T
}

destruct Box<T>(&+self) {
    return
}

func main(): i32 {
    let box = Box<i32> { value: 1 }
    return box.value
}
"#,
    );
    let facts = collect_typecheck_facts(&ast, &resolved);
    let specialization = facts
        .drop_type_specializations()
        .next()
        .expect("generic destructor specialization");
    assert_eq!(
        resolved
            .semantic_db
            .definition_at(specialization.declaration_span),
        Some(specialization.def_id)
    );
}

#[test]
fn specializes_structural_index_requirement_to_a_declared_operator() {
    let text = r#"struct Buffer {
    values: [i32; 1]
}

instance Buffer {
    operator (&self[index: usize]): &i32 {
        return &self.values[0]
    }
}

func at<C, V>(container: &C, index: usize): V where copy V, (&C[usize]): &V {
    return container[index]
}
"#;
    let (ast, resolved) = parse_and_resolve_text(text);
    let facts = collect_typecheck_facts(&ast, &resolved);
    let start = text.rfind("container[index]").expect("index expression");
    let span = ByteSpan::new(ast.span.source, start, start + "container[index]".len());
    let plan = facts.index_plan(span).expect("generic index plan");
    assert_eq!(plan.projection, TypecheckIndexProjection::Requirement);
    let substitutions = std::collections::HashMap::from([
        (
            "C".to_string(),
            TypeExpr::Reference(crate::ast::TypeReference {
                span,
                name: "Buffer".to_string(),
            }),
        ),
        (
            "V".to_string(),
            TypeExpr::Reference(crate::ast::TypeReference {
                span,
                name: "i32".to_string(),
            }),
        ),
    ]);
    let plan = plan
        .with_context_substitutions(&substitutions)
        .expect("concrete plan");
    let specialized =
        crate::typecheck::specialize_index_plan_across_resolvers(plan, std::iter::once(&resolved))
            .expect("specialized index plan");
    assert_eq!(specialized.projection, TypecheckIndexProjection::Declared);
    assert!(specialized.method.is_some());
}

#[test]
fn records_receiver_coercion_and_builtin_method_declaration_identity() {
    let text = r#"struct Text { value: &str }
instance Text { pub coerce &self as &str { return self.value } }
instance str { pub method &self.count(): usize { return 1 } }
func count(text: &Text): usize { return text.count() }
func main(): i32 { return 0 }
"#;
    let (ast, resolved) = parse_and_resolve_text(text);
    let facts = collect_typecheck_facts(&ast, &resolved);
    let call_start = text.rfind("text.count").expect("method call");
    let receiver_span = ByteSpan::new(ast.span.source, call_start, call_start + "text".len());
    let member_start = call_start + "text.".len();
    let member_span = ByteSpan::new(ast.span.source, member_start, member_start + "count".len());

    let plan = facts
        .coercion_plan(receiver_span)
        .expect("receiver coercion plan");
    assert_eq!(canonical_type_expr(&plan.self_ty), "Text");
    assert_eq!(canonical_type_expr(&plan.target_ty), "&str");
    let target = facts
        .method_call_target(member_span)
        .expect("source method target");
    assert_eq!(&text[target.start..target.end], "count");
}

#[test]
fn receiver_coerced_method_result_keeps_concrete_nested_generics() {
    let text = r#"struct View { marker: usize }
struct Text { view: View }
struct Iter<T> { marker: usize }
interface Iterator {
    pub type Item
    pub method &+self.next(): Self.Item?
}
instance Text { pub coerce &self as &View { return &self.view } }
instance View {
    pub method &self.bytes_iter(): Iter<u8> {
        return Iter<u8> { marker: 0 }
    }
}
conform Iterator for Iter<T> {
    type Item = &T
    method &+self.next(): &T? { return none }
}
func main(): i32 {
    let text = Text { view: View { marker: 0 } }
    var bytes = text.bytes_iter()
    let first = bytes.next() otherwise { return 0 }
    return 0
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
    let Stmt::Binding(bytes) = &function.body.as_ref().unwrap().statements[1] else {
        panic!("expected bytes binding");
    };
    let Stmt::Binding(first) = &function.body.as_ref().unwrap().statements[2] else {
        panic!("expected first binding");
    };

    assert_eq!(facts.binding_type_label(bytes.name_span), Some("Iter<u8>"));
    assert_eq!(facts.binding_type_label(first.name_span), Some("&u8"));
}

#[test]
fn records_a_concrete_coercion_plan_at_the_expected_type_boundary() {
    let text = r#"struct Box<T> { value: T }
instance Box<T> {
    pub coerce &self as &T from self { return &self.value }
}
func accept(value: &i32): void { return }
func demo(value: &Box<i32>): void { accept(value) return }
func main(): i32 { return 0 }
"#;
    let (ast, resolved) = parse_and_resolve_text(text);
    let facts = collect_typecheck_facts(&ast, &resolved);
    let argument_start = text.rfind("value)").expect("expected call argument");
    let argument_span = ByteSpan::new(ast.span.source, argument_start, argument_start + 5);
    let plan = facts
        .coercion_plan(argument_span)
        .expect("expected coercion plan");

    assert_eq!(canonical_type_expr(&plan.self_ty), "Box<i32>");
    assert_eq!(canonical_type_expr(&plan.target_ty), "&i32");
    assert_eq!(plan.receiver_mode, MethodReceiverMode::ReadonlyBorrow);
    assert!(!plan.source_is_readwrite);
    assert_eq!(
        plan.def_id,
        resolved.semantic_db.definition_at(plan.focus_span),
        "the persisted conversion must retain the selected coercion identity"
    );
}

#[test]
fn records_coercion_plans_at_all_concrete_expected_type_boundaries() {
    let text = r#"struct Box<T> { value: T }
instance Box<T> { pub coerce &self as &T from self { return &self.value } }
struct Holder { value: &i32 }
func accept(value: &i32): void { return }
func project(value: &Box<i32>): &i32 from value {
    let bound: &i32 = value
    var assigned: &i32 = bound
    assigned = value
    let holder = Holder { value: value }
    let elements: [&i32; 1] = [value]
    accept(value)
    return value
}
func main(): i32 { return 0 }
"#;
    let (ast, resolved) = parse_and_resolve_text(text);
    let facts = collect_typecheck_facts(&ast, &resolved);
    let plans = facts.coercion_plans().collect::<Vec<_>>();

    assert_eq!(plans.len(), 6, "expected one plan per contextual boundary");
    assert!(plans.iter().all(|(_, plan)| {
        canonical_type_expr(&plan.self_ty) == "Box<i32>"
            && canonical_type_expr(&plan.target_ty) == "&i32"
    }));
}

#[test]
fn records_explicit_borrow_coercion_on_the_as_expression_boundary() {
    let text = r#"struct Box<T> { value: T }
instance Box<T> { pub coerce &self as &T from self { return &self.value } }
func project(value: &Box<i32>): &i32 from value { return value as &i32 }
func main(): i32 { return 0 }
"#;
    let (ast, resolved) = parse_and_resolve_text(text);
    let facts = collect_typecheck_facts(&ast, &resolved);
    let as_start = text.rfind("as &i32").expect("expected explicit as");
    let expression_start = text.rfind("value as").expect("expected source");
    let expression_span = ByteSpan::new(
        ast.span.source,
        expression_start,
        as_start + "as &i32".len(),
    );
    let plan = facts
        .conversion_plan(expression_span)
        .expect("expected explicit conversion plan");

    assert_eq!(plan.operator_span.unwrap().start, as_start);
    assert_eq!(plan.source_span.start, expression_start);
    assert_eq!(canonical_type_expr(&plan.source_ty), "&Box<i32>");
    assert_eq!(canonical_type_expr(&plan.target_ty), "&i32");
    assert!(matches!(
        plan.kind,
        TypecheckConversionKind::BorrowCoercion(_)
    ));
}

#[test]
fn records_lossless_integer_and_capability_conversion_kinds() {
    let text = r#"struct Cell { value: i32 }
func project(value: &+Cell): &Cell from value {
    let widened = 1 as i64
    return value as &Cell
}
func main(): i32 { return 0 }
"#;
    let (ast, resolved) = parse_and_resolve_text(text);
    let facts = collect_typecheck_facts(&ast, &resolved);
    let kinds = facts
        .conversion_plans()
        .map(|(_, plan)| &plan.kind)
        .collect::<Vec<_>>();

    assert!(
        kinds
            .iter()
            .any(|kind| matches!(kind, TypecheckConversionKind::LosslessInteger))
    );
    assert!(
        kinds
            .iter()
            .any(|kind| matches!(kind, TypecheckConversionKind::CapabilityWeakening))
    );
}

#[test]
fn records_contextual_coercions_inside_typed_sequence_literals() {
    let text = r#"struct Box<T> { value: T }
instance Box<T> { pub coerce &self as &T from self { return &self.value } }
struct Vec<T> { marker: i32 }
construct Vec<T> {
    pub default literal [](...items: T): Self { return Self { marker: 0 } }
}
func collect(value: &Box<i32>): void {
    let views: Vec<&i32> = Vec [value]
    return
}
func main(): i32 { return 0 }
"#;
    let (ast, resolved) = parse_and_resolve_text(text);
    let facts = collect_typecheck_facts(&ast, &resolved);
    let value_start = text.rfind("value]").expect("expected literal element");
    let value_span = ByteSpan::new(ast.span.source, value_start, value_start + "value".len());
    let plan = facts
        .coercion_plan(value_span)
        .expect("expected element coercion plan");

    assert_eq!(canonical_type_expr(&plan.self_ty), "Box<i32>");
    assert_eq!(canonical_type_expr(&plan.target_ty), "&i32");
}

#[test]
fn records_contextual_coercions_on_compound_expression_results() {
    let text = r#"struct Box<T> { value: T }
instance Box<T> { pub coerce &self as &T from self { return &self.value } }
enum Choice { first second }
func maybe(value: &Box<i32>): &Box<i32>? from value { return value }
func project(choice: Choice, value: &Box<i32>): &i32 from value {
    let grouped: &i32 = (value)
    let selected: &i32 = if true { value } else { value }
    let matched: &i32 = match choice {
        Choice.first { value }
        Choice.second { value }
    }
    let forced: &i32 = maybe(value)!
    return grouped
}
func main(): i32 { return 0 }
"#;
    let (ast, resolved) = parse_and_resolve_text(text);
    let facts = collect_typecheck_facts(&ast, &resolved);
    let plans = facts.coercion_plans().collect::<Vec<_>>();

    assert_eq!(
        plans.len(),
        6,
        "expected one coercion per compound result leaf: {plans:#?}"
    );
    assert!(plans.iter().all(|(_, plan)| {
        canonical_type_expr(&plan.self_ty) == "Box<i32>"
            && canonical_type_expr(&plan.target_ty) == "&i32"
    }));
}

#[test]
fn records_contextual_coercion_for_an_enum_payload_argument() {
    let text = r#"struct Box<T> { value: T }
instance Box<T> { pub coerce &self as &T from self { return &self.value } }
enum View<T> { one(value: &T) }
func project(value: &Box<i32>): View<i32> from value { return View.one(value) }
func main(): i32 { return 0 }
"#;
    let (ast, resolved) = parse_and_resolve_text(text);
    let facts = collect_typecheck_facts(&ast, &resolved);
    let argument_start = text.rfind("value)").expect("expected payload argument");
    let argument_span = ByteSpan::new(ast.span.source, argument_start, argument_start + 5);

    assert!(facts.coercion_plan(argument_span).is_some());
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
    let Stmt::Binding(binding) = &function.body.as_ref().unwrap().statements[0] else {
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
