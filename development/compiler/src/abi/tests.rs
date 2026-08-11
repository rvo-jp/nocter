use super::{
    AbiEnum, AbiEnumVariant, AbiField, AbiReturn, AbiType, ParameterPassing, ReturnPassing,
    ValueClassification, ValueLayout, abi_type_from_type_expr,
    abi_value_from_type_expr_with_resolver, classify_value, function_abi_from_signature,
    function_parameter_abi_word_count_from_signature, function_parameters_abi_from_signature,
    function_success_return_passing_from_signature, layout_of, layout_struct,
};
use crate::ast::{AstFile, Item, TypeExpr, TypeReference, substitute_type_expr_parameters};
use crate::lexer::lex;
use crate::parser::parse;
use crate::resolve::{FunctionSignature, ResolveOutput, SymbolKind, resolve};
use crate::source::SourceMap;
use std::collections::HashMap;

#[test]
fn lays_out_scalar_and_view_values() {
    assert_eq!(layout_of(&AbiType::Bool).unwrap(), ValueLayout::new(1, 1));
    assert_eq!(layout_of(&AbiType::U8).unwrap(), ValueLayout::new(1, 1));
    assert_eq!(layout_of(&AbiType::I32).unwrap(), ValueLayout::new(4, 4));
    assert_eq!(layout_of(&AbiType::Usize).unwrap(), ValueLayout::new(8, 8));
    assert_eq!(
        layout_of(&AbiType::Pointer).unwrap(),
        ValueLayout::new(8, 8)
    );
    assert_eq!(layout_of(&AbiType::Borrow).unwrap(), ValueLayout::new(8, 8));
    assert_eq!(
        layout_of(&AbiType::StrView).unwrap(),
        ValueLayout::new(16, 8)
    );
    assert_eq!(
        layout_of(&AbiType::SliceView).unwrap(),
        ValueLayout::new(16, 8)
    );
}

#[test]
fn lays_out_fixed_array_values() {
    assert_eq!(
        layout_of(&AbiType::Array {
            element: Box::new(AbiType::U8),
            length: 0,
        })
        .unwrap(),
        ValueLayout::new(0, 1)
    );
    assert_eq!(
        layout_of(&AbiType::Array {
            element: Box::new(AbiType::U8),
            length: 4,
        })
        .unwrap(),
        ValueLayout::new(4, 1)
    );
    assert_eq!(
        layout_of(&AbiType::Array {
            element: Box::new(AbiType::I32),
            length: 3,
        })
        .unwrap(),
        ValueLayout::new(12, 4)
    );
    assert_eq!(
        layout_of(&AbiType::Array {
            element: Box::new(AbiType::StrView),
            length: 2,
        })
        .unwrap(),
        ValueLayout::new(32, 8)
    );
}

#[test]
fn lays_out_struct_fields_in_declaration_order_with_padding() {
    let layout = layout_struct(&[
        AbiField::new("tag", AbiType::U8),
        AbiField::new("count", AbiType::I32),
        AbiField::new("ptr", AbiType::Pointer),
    ])
    .unwrap();

    assert_eq!(layout.size, 16);
    assert_eq!(layout.align, 8);
    assert_eq!(layout.fields[0].offset, 0);
    assert_eq!(layout.fields[1].offset, 4);
    assert_eq!(layout.fields[2].offset, 8);
}

#[test]
fn classifies_values_by_direct_size_limit() {
    assert_eq!(
        classify_value(&AbiType::Usize).unwrap(),
        ValueClassification::Direct { words: 1 }
    );
    assert_eq!(
        classify_value(&AbiType::StrView).unwrap(),
        ValueClassification::Direct { words: 2 }
    );
    assert_eq!(
        classify_value(&AbiType::Array {
            element: Box::new(AbiType::U8),
            length: 16,
        })
        .unwrap(),
        ValueClassification::Direct { words: 2 }
    );
    assert_eq!(
        classify_value(&AbiType::Array {
            element: Box::new(AbiType::U8),
            length: 17,
        })
        .unwrap(),
        ValueClassification::Indirect
    );

    let string_like = AbiType::Struct(vec![
        AbiField::new("ptr", AbiType::Pointer),
        AbiField::new("len", AbiType::Usize),
        AbiField::new("capacity", AbiType::Usize),
    ]);
    assert_eq!(layout_of(&string_like).unwrap(), ValueLayout::new(24, 8));
    assert_eq!(
        classify_value(&string_like).unwrap(),
        ValueClassification::Indirect
    );
}

#[test]
fn maps_fixed_array_type_expr_to_abi_array_layout() {
    let (ast, resolved) = parse_and_resolve(
        r#"func load(): [u8; 4] {
}
"#,
    );
    let return_type = ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "load" => Some(&function.return_type),
            _ => None,
        })
        .expect("expected load function");

    let value = abi_value_from_type_expr_with_resolver(return_type, &resolved, |_| Some(&resolved))
        .unwrap();

    assert_eq!(
        value.ty,
        AbiType::Array {
            element: Box::new(AbiType::U8),
            length: 4,
        }
    );
    assert_eq!(value.layout, ValueLayout::new(4, 1));
    assert_eq!(
        value.classification,
        ValueClassification::Direct { words: 1 }
    );
}

#[test]
fn maps_resolved_struct_type_expr_to_abi_struct_layout() {
    let (ast, resolved) = parse_and_resolve(
        r#"struct Text {
ptr: *u8
len: usize
capacity: usize
}

func make(): Text {
}
"#,
    );
    let return_type = ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "make" => Some(&function.return_type),
            _ => None,
        })
        .expect("expected make function");

    let ty = abi_type_from_type_expr(return_type, &resolved).unwrap();

    assert_eq!(layout_of(&ty).unwrap(), ValueLayout::new(24, 8));
    assert_eq!(classify_value(&ty).unwrap(), ValueClassification::Indirect);
}

#[test]
fn maps_concrete_generic_struct_type_expr_to_abi_struct_layout() {
    let (ast, resolved) = parse_and_resolve(
        r#"struct Box<T> {
value: T
}

func make(): Box<i32> {
}
"#,
    );
    let return_type = ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "make" => Some(&function.return_type),
            _ => None,
        })
        .expect("expected make function");

    let ty = abi_type_from_type_expr(return_type, &resolved).unwrap();

    assert_eq!(
        ty,
        AbiType::Struct(vec![AbiField::new("value", AbiType::I32)])
    );
    assert_eq!(layout_of(&ty).unwrap(), ValueLayout::new(4, 4));
    assert_eq!(
        classify_value(&ty).unwrap(),
        ValueClassification::Direct { words: 1 }
    );
}

#[test]
fn maps_nested_concrete_generic_struct_type_expr_to_abi_struct_layout() {
    let (ast, resolved) = parse_and_resolve(
        r#"struct Pair<T, U> {
first: T
second: U
}

struct Box<T> {
value: Pair<T, usize>
}

func make(): Box<i32> {
}
"#,
    );
    let return_type = ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "make" => Some(&function.return_type),
            _ => None,
        })
        .expect("expected make function");

    let ty = abi_type_from_type_expr(return_type, &resolved).unwrap();

    assert_eq!(
        ty,
        AbiType::Struct(vec![AbiField::new(
            "value",
            AbiType::Struct(vec![
                AbiField::new("first", AbiType::I32),
                AbiField::new("second", AbiType::Usize),
            ])
        )])
    );
    assert_eq!(layout_of(&ty).unwrap(), ValueLayout::new(16, 8));
    assert_eq!(
        classify_value(&ty).unwrap(),
        ValueClassification::Direct { words: 2 }
    );
}

#[test]
fn source_aware_abi_lays_out_generic_struct_with_foreign_type_argument() {
    let mut sources = SourceMap::new();
    let root_ast = parse_source(
        &mut sources,
        "app.nct",
        r#"struct Pair {
left: i32
right: usize
}

func make_pair(): Pair {
}
"#,
    );
    let library_ast = parse_source(
        &mut sources,
        "std/box/index.nct",
        r#"struct Box<T> {
value: T
}

func make_box<T>(): Box<T> {
}
"#,
    );
    let root_resolved = resolve(&sources, &root_ast);
    let library_resolved = resolve(&sources, &library_ast);
    assert!(
        root_resolved.diagnostics.is_empty(),
        "{:?}",
        root_resolved.diagnostics
    );
    assert!(
        library_resolved.diagnostics.is_empty(),
        "{:?}",
        library_resolved.diagnostics
    );

    let pair_ty = function_return_type(&root_ast, "make_pair").clone();
    let box_template_ty = function_return_type(&library_ast, "make_box");
    let box_pair_ty = substitute_type_expr_parameters(
        box_template_ty,
        &HashMap::from([("T".to_string(), pair_ty)]),
    );

    let value = abi_value_from_type_expr_with_resolver(&box_pair_ty, &library_resolved, |source| {
        match source {
            source if source == root_ast.span.source => Some(&root_resolved),
            source if source == library_ast.span.source => Some(&library_resolved),
            _ => None,
        }
    })
    .unwrap();

    assert_eq!(
        value.ty,
        AbiType::Struct(vec![AbiField::new(
            "value",
            AbiType::Struct(vec![
                AbiField::new("left", AbiType::I32),
                AbiField::new("right", AbiType::Usize),
            ])
        )])
    );
    assert_eq!(value.layout, ValueLayout::new(16, 8));
    assert_eq!(
        value.classification,
        ValueClassification::Direct { words: 2 }
    );
}

#[test]
fn source_aware_abi_resolves_qualified_type_name_in_declaring_source() {
    let mut sources = SourceMap::new();
    let library_ast = parse_source(
        &mut sources,
        "std/internal/os/index.nct",
        r#"copy struct SyscallResult {
value: usize
errno: i32
}
"#,
    );
    let library_resolved = resolve(&sources, &library_ast);
    assert!(
        library_resolved.diagnostics.is_empty(),
        "{:?}",
        library_resolved.diagnostics
    );

    let ty = TypeExpr::Reference(TypeReference {
        span: library_ast.span,
        name: "std/internal/os.SyscallResult".to_string(),
    });
    let value = abi_value_from_type_expr_with_resolver(&ty, &library_resolved, |source| {
        (source == library_ast.span.source).then_some(&library_resolved)
    })
    .unwrap();

    assert_eq!(
        value.ty,
        AbiType::Struct(vec![
            AbiField::new("value", AbiType::Usize),
            AbiField::new("errno", AbiType::I32),
        ])
    );
    assert_eq!(value.layout, ValueLayout::new(16, 8));
}

#[test]
fn maps_payloadless_enum_type_expr_to_u8_tag_layout() {
    let (ast, resolved) = parse_and_resolve(
        r#"enum Choice {
yes
no
}

func choose(): Choice {
}
"#,
    );
    let return_type = ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "choose" => Some(&function.return_type),
            _ => None,
        })
        .expect("expected choose function");

    let ty = abi_type_from_type_expr(return_type, &resolved).unwrap();

    assert_eq!(ty, AbiType::U8);
    assert_eq!(layout_of(&ty).unwrap(), ValueLayout::new(1, 1));
    assert_eq!(
        classify_value(&ty).unwrap(),
        ValueClassification::Direct { words: 1 }
    );
}

#[test]
fn maps_payload_enum_type_expr_to_tag_union_layout() {
    let (ast, resolved) = parse_and_resolve(
        r#"enum Status {
missing
found(code: i32)
}

func status(): Status {
}
"#,
    );
    let return_type = ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "status" => Some(&function.return_type),
            _ => None,
        })
        .expect("expected status function");

    let ty = abi_type_from_type_expr(return_type, &resolved).unwrap();

    assert_eq!(
        ty,
        AbiType::Enum(AbiEnum {
            variants: vec![
                AbiEnumVariant::new("missing", 0, None),
                AbiEnumVariant::new("found", 1, Some(AbiType::I32)),
            ],
            payload_offset: 4,
            payload_layout: ValueLayout::new(4, 4),
        })
    );
    assert_eq!(layout_of(&ty).unwrap(), ValueLayout::new(8, 4));
    assert_eq!(
        classify_value(&ty).unwrap(),
        ValueClassification::Direct { words: 1 }
    );
}

#[test]
fn substitutes_generic_payload_enum_layout() {
    let (ast, resolved) = parse_and_resolve(
        r#"enum Maybe<T> {
absent
some(value: T)
}

func maybe(): Maybe<&str> {
}
"#,
    );
    let return_type = ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "maybe" => Some(&function.return_type),
            _ => None,
        })
        .expect("expected maybe function");

    let ty = abi_type_from_type_expr(return_type, &resolved).unwrap();

    assert_eq!(
        ty,
        AbiType::Enum(AbiEnum {
            variants: vec![
                AbiEnumVariant::new("absent", 0, None),
                AbiEnumVariant::new("some", 1, Some(AbiType::StrView)),
            ],
            payload_offset: 8,
            payload_layout: ValueLayout::new(16, 8),
        })
    );
    assert_eq!(layout_of(&ty).unwrap(), ValueLayout::new(24, 8));
    assert_eq!(classify_value(&ty).unwrap(), ValueClassification::Indirect);
}

#[test]
fn maps_borrow_of_str_alias_to_str_view_layout() {
    let (ast, resolved) = parse_and_resolve(
        r#"type Text = str

func view(text: &Text): &Text {
}
"#,
    );
    let return_type = ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "view" => Some(&function.return_type),
            _ => None,
        })
        .expect("expected view function");

    let ty = abi_type_from_type_expr(return_type, &resolved).unwrap();

    assert_eq!(ty, AbiType::StrView);
    assert_eq!(layout_of(&ty).unwrap(), ValueLayout::new(16, 8));
}

#[test]
fn classifies_function_signature_values() {
    let (_ast, resolved) = parse_and_resolve(
        r#"struct Text {
ptr: *u8
len: usize
capacity: usize
}

func passthrough(text: Text, view: &str, count: usize): Text {
}
"#,
    );
    let signature = resolved_function_signature(&resolved, "passthrough");

    let abi = function_abi_from_signature(signature, &resolved).unwrap();

    assert_eq!(abi.parameters.len(), 3);
    assert_eq!(abi.parameters[0].name, "text");
    assert_eq!(abi.parameters[0].value.layout, ValueLayout::new(24, 8));
    assert_eq!(
        abi.parameters[0].value.parameter_passing(),
        ParameterPassing::IndirectPointer
    );
    assert_eq!(
        abi.parameters[0].value.classification,
        ValueClassification::Indirect
    );
    assert_eq!(abi.parameters[1].name, "view");
    assert_eq!(abi.parameters[1].value.ty, AbiType::StrView);
    assert_eq!(
        abi.parameters[1].value.parameter_passing(),
        ParameterPassing::Direct { words: 2 }
    );
    assert_eq!(
        abi.parameters[1].value.classification,
        ValueClassification::Direct { words: 2 }
    );
    assert_eq!(abi.parameters[2].name, "count");
    assert_eq!(
        abi.parameters[2].value.classification,
        ValueClassification::Direct { words: 1 }
    );
    assert_eq!(abi.parameter_abi_word_count(), 4);
    assert!(abi.parameters_fit_registers());
    assert!(abi.uses_indirect_return_pointer());
    assert_eq!(abi.return_value.passing(), ReturnPassing::IndirectPointer);
    assert!(matches!(
        abi.return_value,
        AbiReturn::Value(ref value)
            if value.layout == ValueLayout::new(24, 8)
                && value.classification == ValueClassification::Indirect
    ));
}

#[test]
fn classifies_void_and_never_returns_without_value_layout() {
    let (_ast, resolved) = parse_and_resolve(
        r#"primitive stop(): never

func done(): void {
}
"#,
    );

    let stop =
        function_abi_from_signature(resolved_function_signature(&resolved, "stop"), &resolved)
            .unwrap();
    let done =
        function_abi_from_signature(resolved_function_signature(&resolved, "done"), &resolved)
            .unwrap();

    assert_eq!(stop.return_value, AbiReturn::Never);
    assert_eq!(done.return_value, AbiReturn::Void);
    assert_eq!(stop.return_value.passing(), ReturnPassing::Never);
    assert_eq!(done.return_value.passing(), ReturnPassing::Void);
    assert!(!stop.uses_indirect_return_pointer());
    assert!(!done.uses_indirect_return_pointer());
}

#[test]
fn classifies_alias_void_and_never_returns_without_value_layout() {
    let (_ast, resolved) = parse_and_resolve(
        r#"type Unit = void
type Bottom = never

primitive stop(): Bottom

func done(): Unit {
}
"#,
    );

    let stop =
        function_abi_from_signature(resolved_function_signature(&resolved, "stop"), &resolved)
            .unwrap();
    let done =
        function_abi_from_signature(resolved_function_signature(&resolved, "done"), &resolved)
            .unwrap();

    assert_eq!(stop.return_value, AbiReturn::Never);
    assert_eq!(done.return_value, AbiReturn::Void);
    assert_eq!(stop.return_value.passing(), ReturnPassing::Never);
    assert_eq!(done.return_value.passing(), ReturnPassing::Void);
}

#[test]
fn detects_when_parameters_exceed_register_window() {
    let (_ast, resolved) = parse_and_resolve(
        r#"func many(
a: &str,
b: &str,
c: &str,
d: &str,
e: usize,
): void {
}
"#,
    );
    let signature = resolved_function_signature(&resolved, "many");

    let abi = function_abi_from_signature(signature, &resolved).unwrap();

    assert_eq!(abi.parameter_abi_word_count(), 9);
    assert!(!abi.parameters_fit_registers());
}

#[test]
fn counts_parameters_for_fallible_return_signatures() {
    let (_ast, resolved) = parse_and_resolve(
        r#"func load(text: &str, count: usize): i32! {
}
"#,
    );
    let signature = resolved_function_signature(&resolved, "load");

    let count = function_parameter_abi_word_count_from_signature(signature, &resolved).unwrap();

    assert_eq!(count, 3);
}

#[test]
fn counts_error_parameters_as_failure_payload_words() {
    let (_ast, resolved) = parse_and_resolve(
        r#"type Failure = error

func relay(error: Failure, tag: i32): i32! {
}
"#,
    );
    let signature = resolved_function_signature(&resolved, "relay");

    let count = function_parameter_abi_word_count_from_signature(signature, &resolved).unwrap();

    assert_eq!(count, 5);
}

#[test]
fn classifies_fallible_signature_success_return_passing() {
    let (_ast, resolved) = parse_and_resolve(
        r#"struct Header {
tag: u64
len: u64
}

struct Text {
ptr: *u8
len: usize
capacity: usize
}

func header(): Header! {
}

func text(): Text! {
}
"#,
    );

    assert_eq!(
        function_success_return_passing_from_signature(
            resolved_function_signature(&resolved, "header"),
            &resolved,
        )
        .unwrap(),
        ReturnPassing::Direct { words: 2 }
    );
    assert_eq!(
        function_success_return_passing_from_signature(
            resolved_function_signature(&resolved, "text"),
            &resolved,
        )
        .unwrap(),
        ReturnPassing::IndirectPointer
    );
}

#[test]
fn classifies_optional_signature_success_return_passing() {
    let (_ast, resolved) = parse_and_resolve(
        r#"type MaybeHeader = Header?

struct Header {
tag: u64
len: u64
}

struct Text {
ptr: *u8
len: usize
capacity: usize
}

func header(): Header? {
}

func aliased_header(): MaybeHeader {
}

func text(): Text? {
}
"#,
    );

    assert_eq!(
        function_success_return_passing_from_signature(
            resolved_function_signature(&resolved, "header"),
            &resolved,
        )
        .unwrap(),
        ReturnPassing::Direct { words: 2 }
    );
    assert_eq!(
        function_success_return_passing_from_signature(
            resolved_function_signature(&resolved, "aliased_header"),
            &resolved,
        )
        .unwrap(),
        ReturnPassing::Direct { words: 2 }
    );
    assert_eq!(
        function_success_return_passing_from_signature(
            resolved_function_signature(&resolved, "text"),
            &resolved,
        )
        .unwrap(),
        ReturnPassing::IndirectPointer
    );
}

#[test]
fn classifies_parameters_without_return_layout() {
    let (_ast, resolved) = parse_and_resolve(
        r#"struct Text {
ptr: *u8
len: usize
capacity: usize
}

func load(text: Text, view: &str): i32! {
}
"#,
    );
    let signature = resolved_function_signature(&resolved, "load");

    let parameters = function_parameters_abi_from_signature(signature, &resolved).unwrap();

    assert_eq!(parameters.len(), 2);
    assert_eq!(parameters[0].name, "text");
    assert_eq!(parameters[0].value.layout, ValueLayout::new(24, 8));
    assert_eq!(
        parameters[0].value.classification,
        ValueClassification::Indirect
    );
    assert_eq!(parameters[1].name, "view");
    assert_eq!(parameters[1].value.ty, AbiType::StrView);
    assert_eq!(
        parameters[1].value.classification,
        ValueClassification::Direct { words: 2 }
    );
}

#[test]
fn classifies_stored_outcome_parameters_by_recursive_layout() {
    let (_ast, resolved) = parse_and_resolve(
        r#"func inspect(optional: i32?, fallible: i32!, nested: i32?!): i32 {
    return 0
}
"#,
    );
    let parameters = function_parameters_abi_from_signature(
        resolved_function_signature(&resolved, "inspect"),
        &resolved,
    )
    .unwrap();

    assert_eq!(parameters[0].value.layout, ValueLayout::new(16, 8));
    assert_eq!(
        parameters[0].value.classification,
        ValueClassification::Direct { words: 2 }
    );
    assert_eq!(parameters[1].value.layout, ValueLayout::new(40, 8));
    assert_eq!(
        parameters[1].value.classification,
        ValueClassification::Indirect
    );
    assert_eq!(parameters[2].value.layout, ValueLayout::new(40, 8));
    assert_eq!(
        parameters[2].value.classification,
        ValueClassification::Indirect
    );
}

fn parse_and_resolve(text: &str) -> (crate::ast::AstFile, crate::resolve::ResolveOutput) {
    let mut sources = SourceMap::new();
    let ast = parse_source(&mut sources, "app.nct", text);
    let resolved = resolve(&sources, &ast);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    (ast, resolved)
}

fn parse_source(sources: &mut SourceMap, display_path: &str, text: &str) -> AstFile {
    let source = sources.add_source(display_path, None, text);
    let lexed = lex(sources, source);
    assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
    let parsed = parse(sources, source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.ast.unwrap()
}

fn function_return_type<'a>(ast: &'a AstFile, name: &str) -> &'a TypeExpr {
    ast.items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == name => Some(&function.return_type),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected function `{name}`"))
}

fn resolved_function_signature<'a>(
    resolved: &'a ResolveOutput,
    name: &str,
) -> &'a FunctionSignature {
    let symbol = resolved
        .symbols
        .symbol_by_name(name)
        .unwrap_or_else(|| panic!("expected symbol `{name}`"));
    match &symbol.kind {
        SymbolKind::Function(signature) | SymbolKind::Primitive(signature) => signature,
        SymbolKind::Type(_) | SymbolKind::Imported(_) => {
            panic!("expected function or primitive symbol `{name}`")
        }
    }
}
