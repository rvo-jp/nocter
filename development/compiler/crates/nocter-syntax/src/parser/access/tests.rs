use crate::{NodeKind, ParseGoal, SyntaxTree, parse, parse_reusable};
use nocter_json::Value;
use nocter_source::{SourceMap, SourceName};
use std::{fs, path::PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/value-bindings")
}
fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    let Value::Object(members) = value else {
        panic!("object")
    };
    &members
        .iter()
        .find(|member| member.name.as_ref() == name)
        .unwrap()
        .value
}
fn string(value: &Value) -> &str {
    let Value::String(text) = value else {
        panic!("string")
    };
    text
}
fn tree(text: &str, goal: ParseGoal) -> SyntaxTree {
    let mut sources = SourceMap::new();
    let id = sources
        .add_bytes(SourceName::new("binding.nct"), text.as_bytes())
        .unwrap();
    parse(sources.get(id).unwrap(), goal)
}
#[test]
fn corpus_uses_the_production_goal_with_lossless_tokens_and_structural_snapshots() {
    let root = root();
    let manifest =
        nocter_json::parse(&fs::read_to_string(root.join("manifest.json")).unwrap()).unwrap();
    let Value::Array(cases) = field(&manifest, "cases") else {
        panic!("cases")
    };
    let mut counts = [0; 2];
    for case in cases {
        let name = string(field(case, "file"));
        let text = fs::read_to_string(root.join(name)).unwrap();
        let parsed = tree(&text, ParseGoal::V2CoreUnit);
        super::super::snapshots::assert_token_projection(&parsed, name);
        assert_eq!(parsed.root().kind(), NodeKind::V2CoreUnit);
        if string(field(case, "expectation")) == "syntax-reject" {
            assert!(parsed.has_errors(), "{name}");
            counts[1] += 1;
        } else {
            assert!(!parsed.has_errors(), "{name}: {:?}", parsed.diagnostics());
            let shape = super::super::snapshots::syntax_shape(&parsed);
            let path = root.join(name).with_extension("shape");
            if std::env::var_os("NOCTER_UPDATE_ACCESS_SHAPES").is_some() {
                fs::write(&path, &shape).unwrap();
            }
            assert_eq!(shape, fs::read_to_string(path).unwrap(), "{name}");
            counts[0] += 1;
        }
    }
    assert!(counts.iter().all(|&count| count > 0));
}
#[test]
fn operation_nodes_preserve_contextual_names_without_type_lookup() {
    let text = "func edit(edit: i32, take take: i32): i32 {\nvar read = edit\nread = take\nread observed = read\nreturn observed\n}\n";
    let parsed = tree(text, ParseGoal::V2CoreUnit);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    assert_eq!(
        parsed
            .nodes()
            .filter(|(_, n)| n.kind() == NodeKind::InputOperation)
            .count(),
        1
    );
    assert_eq!(
        parsed
            .nodes()
            .filter(|(_, n)| n.kind() == NodeKind::BindingOperation)
            .count(),
        2
    );
    super::super::snapshots::assert_token_projection(&parsed, "contextual");
}
#[test]
fn retired_words_are_names_only_in_the_future_profile_and_throw_is_reserved() {
    for name in [
        "pub",
        "construct",
        "instance",
        "move",
        "noalloc",
        "notrap",
        "blocking",
    ] {
        let text = format!("func {name}(value: i32): i32 {{ return value }}\n");
        assert!(!tree(&text, ParseGoal::V2CoreUnit).has_errors(), "{text}");
        assert!(tree(&text, ParseGoal::SourceFile).has_errors(), "{text}");
    }
    assert!(tree("func throw(): void {}\n", ParseGoal::V2CoreUnit).has_errors());
    assert!(!tree("func throw(): void {}\n", ParseGoal::SourceFile).has_errors());
}
#[test]
fn retired_prefix_assertions_and_type_qualifiers_have_no_fallback() {
    for text in [
        "func f(x: &i32): void {}",
        "func f(x: ^i32): void {}",
        "func f(x: i32): i32 { return ^x }",
        "func f(x: i32): void { f(&x) }",
        "func f(x: i32): void { f((^x)) }",
        "func f(x: i32): void { let y = &&x }",
        "func f<T>(x: T): T { return x }",
        "func f(): void { let c = (^x;) { x } }",
    ] {
        assert!(tree(text, ParseGoal::V2CoreUnit).has_errors(), "{text}");
    }
    assert!(!tree("func f(x: &i32): void {}", ParseGoal::SourceFile).has_errors());
}
#[test]
fn owned_capture_closure_heads_do_not_reinterpret_groups_or_unit() {
    let text = "func run(): void {\nlet unit = ()\nlet grouped = (unit)\nlet action = (unit; edit value: i32, take payload: error): error { payload }\nlet other = (unit, grouped;): void {}\n}";
    let parsed = tree(text, ParseGoal::V2CoreUnit);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    for (kind, expected) in [
        (NodeKind::ClosureExpression, 2),
        (NodeKind::ClosureCapture, 3),
        (NodeKind::UnitExpression, 1),
        (NodeKind::GroupedExpression, 1),
        (NodeKind::InputOperation, 2),
    ] {
        assert_eq!(
            parsed.nodes().filter(|(_, n)| n.kind() == kind).count(),
            expected,
            "{kind:?}"
        );
    }
    super::super::snapshots::assert_token_projection(&parsed, "owned-capture");
}
#[test]
fn unary_and_binary_precedence_retains_the_original_tokens() {
    let text =
        "func f(value: i32, ready: bool): bool { return !ready || value + -2 * 3 < 10 && ready }";
    let parsed = tree(text, ParseGoal::V2CoreUnit);
    assert!(!parsed.has_errors());
    super::super::snapshots::assert_token_projection(&parsed, "precedence");
    for kind in [
        NodeKind::LogicalOrExpression,
        NodeKind::LogicalAndExpression,
        NodeKind::OrderingExpression,
        NodeKind::AdditiveExpression,
        NodeKind::MultiplicativeExpression,
        NodeKind::UnaryExpression,
    ] {
        assert!(parsed.nodes().any(|(_, n)| n.kind() == kind), "{kind:?}");
    }
}
#[test]
fn reusable_parse_rebinds_only_equal_normalized_text() {
    let mut sources = SourceMap::new();
    let text = "func f(take value: error): error { let next = value^\nreturn next^ }\n";
    let first = sources
        .add_bytes(SourceName::new("one.nct"), text.as_bytes())
        .unwrap();
    let second = sources
        .add_bytes(SourceName::new("two.nct"), text.as_bytes())
        .unwrap();
    let changed = sources
        .add_bytes(
            SourceName::new("changed.nct"),
            text.replace("take value", "value").as_bytes(),
        )
        .unwrap();
    let query = parse_reusable(sources.get(first).unwrap(), ParseGoal::V2CoreUnit);
    assert!(query.bind(sources.get(second).unwrap()).is_some());
    assert!(query.bind(sources.get(changed).unwrap()).is_none());
}
#[test]
fn nesting_limit_recovers_without_losing_the_source_projection() {
    let text = format!(
        "func f(): i32 {{ return {}0{} }}",
        "(".repeat(500),
        ")".repeat(500)
    );
    let parsed = tree(&text, ParseGoal::V2CoreUnit);
    assert!(parsed.has_errors());
    super::super::snapshots::assert_token_projection(&parsed, "depth");
}

#[test]
fn postfix_transfer_is_joint_and_preserves_chain_and_capture_structure() {
    let text = "func run(take value: error): void {\nlet action = (value^;): error { value^ }\nlet result = action^()\nlet _ = buffer^.finish()\n}\n";
    let parsed = tree(text, ParseGoal::V2CoreUnit);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    assert_eq!(
        parsed
            .nodes()
            .filter(|(_, node)| node.kind() == NodeKind::TransferExpression)
            .count(),
        3
    );
    super::super::snapshots::assert_token_projection(&parsed, "postfix-transfer");
    for bad in [
        "func run(take value: error): error { value ^ }",
        "func run(take value: error): error { value /*gap*/ ^ }",
        "func run(take value: error): error { ^value }",
        "func run(): void { let ^value = 1 }",
    ] {
        assert!(tree(bad, ParseGoal::V2CoreUnit).has_errors(), "{bad}");
    }
}

#[test]
fn const_headers_preserve_their_own_node_without_retired_modifier_fallbacks() {
    let parsed = tree(
        "const func increment(edit value: i32): void { value += 1 }\nfunc ordinary(): void {}",
        ParseGoal::V2CoreUnit,
    );
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    assert_eq!(
        parsed
            .nodes()
            .filter(|(_, n)| n.kind() == NodeKind::CompileTimeModifier)
            .count(),
        1
    );
    super::super::snapshots::assert_token_projection(&parsed, "const header");
    for text in [
        "const const func f(): void {}",
        "const async func f(): void {}",
        "const noalloc func f(): void {}",
        "const blocking func f(): void {}",
        "const struct Record {}",
        "const func f(): const func(): void {}",
    ] {
        assert!(tree(text, ParseGoal::V2CoreUnit).has_errors(), "{text}");
    }
}

#[test]
fn control_header_groups_leave_outer_braces_to_control_bodies() {
    let source = "func run(flag: bool): void { if (flag) {}\nwhile (false) {}\nif flag && (if true { false } else { true }) {}\nif ((flag;): bool { flag })() {} }";
    let parsed = tree(source, ParseGoal::V2CoreUnit);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    assert_eq!(
        parsed
            .nodes()
            .filter(|(_, node)| node.kind() == NodeKind::ClosureExpression)
            .count(),
        1
    );
    super::super::snapshots::assert_token_projection(&parsed, "control header groups");
}

#[test]
fn immutable_statics_use_the_initializer_surface_without_modifier_or_type_fallbacks() {
    let source = "static ANSWER: i32 = calculate()\nconst func calculate(): i32 { 42 }";
    let parsed = tree(source, ParseGoal::V2CoreUnit);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    assert_eq!(
        parsed
            .nodes()
            .filter(|(_, node)| node.kind() == NodeKind::StaticDeclaration)
            .count(),
        1
    );
    super::super::snapshots::assert_token_projection(&parsed, "static initializer");
    for source in [
        "static var VALUE: i32 = 1",
        "static VALUE: &i32 = 1",
        "static VALUE: func(): i32 = action",
    ] {
        assert!(tree(source, ParseGoal::V2CoreUnit).has_errors(), "{source}");
    }
}

#[test]
fn contract_header_shapes_share_the_declaration_grammar_without_fabricated_bodies() {
    let source = "func answer(value: i32): i32\nconst VALUE: i32\nstatic ROOT: i32\nstruct Opaque\ncopy struct Visible { value: i32 }";
    let parsed = tree(source, ParseGoal::V2CoreUnit);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    assert!(
        !parsed
            .nodes()
            .any(|(_, node)| node.kind() == NodeKind::Block)
    );
    assert!(
        !parsed
            .nodes()
            .any(|(_, node)| node.kind() == NodeKind::Expression)
    );
    super::super::snapshots::assert_token_projection(&parsed, "contract header shapes");
    let mut sources = SourceMap::new();
    let id = sources
        .add_bytes(SourceName::new("unit.nct"), b"func answer(value: i32): i32")
        .unwrap();
    let header = parse_reusable(sources.get(id).unwrap(), ParseGoal::V2CoreUnit);
    let id = sources
        .add_bytes(
            SourceName::new("body.nct"),
            b"func answer(value: i32): i32 { value }",
        )
        .unwrap();
    let implementation = parse_reusable(sources.get(id).unwrap(), ParseGoal::V2CoreUnit);
    assert_ne!(
        header.declaration_surface(),
        implementation.declaration_surface()
    );
}

#[test]
fn structural_tuple_syntax_distinguishes_unit_groups_and_two_or_more_components() {
    let parsed = tree(
        "func run(input: ((i32, bool)), take sink: func(take (i32, bool)): (void, i32)): (i32, (bool, void)) { let pair = (input.0, (input.1, ()),)\npair }",
        ParseGoal::V2CoreUnit,
    );
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    for kind in [
        NodeKind::TupleType,
        NodeKind::TupleExpression,
        NodeKind::GroupedType,
        NodeKind::UnitExpression,
    ] {
        assert!(
            parsed.nodes().any(|(_, node)| node.kind() == kind),
            "{kind:?}"
        );
    }
    super::super::snapshots::assert_token_projection(&parsed, "structural tuple");
    for source in [
        "func run(): void { let pair = (1,) }",
        "func run(value: (i32,)): void {}",
        "func run(value: ()): void {}",
    ] {
        assert!(tree(source, ParseGoal::V2CoreUnit).has_errors(), "{source}");
    }
}

#[test]
fn fixed_array_types_retain_ordinary_length_expressions_in_every_data_context() {
    for text in [
        "struct Data { values: [[i32; inner()]; outer(1 + 2)] }",
        "func f(input: [i32; count()]): [i32; if true { 1 } else { 2 }] {}",
        "func f(action: func(edit [i32; count()], take [bool; 0]): [void; count()]): void {}",
        "func f(): void { let values: [i32; count()] = missing }",
        "const DATA: [i32; count()] = missing\nstatic MORE: [i32; count()] = missing",
        "func f(): [\ni32\n;\nchoose(1,\n2) +\n3\n] {}",
        "func f(): [i32; Settings { count: 3 }.count] {}",
    ] {
        let parsed = tree(text, ParseGoal::V2CoreUnit);
        assert!(!parsed.has_errors(), "{text}: {:?}", parsed.diagnostics());
        for (id, node) in parsed
            .nodes()
            .filter(|(_, n)| n.kind() == NodeKind::FixedArrayType)
        {
            assert!(crate::direct_node(&parsed, id, NodeKind::Type).is_some());
            assert!(crate::direct_node(&parsed, id, NodeKind::Expression).is_some());
            assert!(!node.range().is_empty());
        }
        assert!(
            parsed
                .nodes()
                .any(|(_, n)| n.kind() == NodeKind::FixedArrayType)
        );
        super::super::snapshots::assert_token_projection(&parsed, text);
    }
}

#[test]
fn fixed_array_type_recovery_preserves_tokens_without_admitting_slices_or_missing_lengths() {
    for text in [
        "func f(): [i32] {}",
        "func f(): [i32;] {}",
        "func f(): [; 2] {}",
        "func f(): [i32, 2] {}",
        "func f(): [i32; 2 {}",
        "func f(): [i32; 1 2] {}",
    ] {
        let parsed = tree(text, ParseGoal::V2CoreUnit);
        assert!(parsed.has_errors(), "{text}");
        super::super::snapshots::assert_token_projection(&parsed, text);
    }
    let text = format!(
        "func f(): {}i32{} {{}}",
        "[".repeat(500),
        "; 0]".repeat(500)
    );
    let parsed = tree(&text, ParseGoal::V2CoreUnit);
    assert!(parsed.has_errors());
    super::super::snapshots::assert_token_projection(&parsed, "array type depth");
}

#[test]
fn array_values_keep_lists_repetition_and_joint_indices_structurally_distinct() {
    for text in [
        "func f(): void { let empty = []\nlet one = [1,]\nlet many = [[1, 2], [3, 4]]\nlet repeated = [make(); count(1, 2) + 3]\nlet selected = repeated[index()]^.field }",
        "func f(): void { let value = [\nmake()\n;\nif true { 1 } else { 2 }\n]\nvalue[\nchoose(1, 2)\n] }",
        "func f(): void { if [Item { value: 1 }; 2][0].value == 1 {} }",
        "func f(): void { let action = (seed;): i32 { 0 }\nlet values = [action; count()] }",
    ] {
        let parsed = tree(text, ParseGoal::V2CoreUnit);
        assert!(!parsed.has_errors(), "{text}: {:?}", parsed.diagnostics());
        for (id, _) in parsed
            .nodes()
            .filter(|(_, node)| node.kind() == NodeKind::ArrayRepeatLiteral)
        {
            let (seed, count) = crate::array_repetition_parts(&parsed, id).unwrap();
            assert_ne!(seed, count);
            assert!(
                parsed.node(seed).unwrap().range().end()
                    < parsed.node(count).unwrap().range().start()
            );
        }
        super::super::snapshots::assert_token_projection(&parsed, text);
    }
    let parsed = tree(
        "func f(): void { [1; count()][index()]^ }",
        ParseGoal::V2CoreUnit,
    );
    for (kind, count) in [
        (NodeKind::ArrayRepeatLiteral, 1),
        (NodeKind::IndexSuffix, 1),
        (NodeKind::TransferExpression, 1),
        (NodeKind::CallSuffix, 2),
    ] {
        assert_eq!(
            parsed
                .nodes()
                .filter(|(_, node)| node.kind() == kind)
                .count(),
            count,
            "{kind:?}"
        );
    }
}

#[test]
fn array_value_recovery_rejects_mixed_delimiters_spread_and_spaced_indexing() {
    for text in [
        "func f(): void { [; 2] }",
        "func f(): void { [1;] }",
        "func f(): void { [1; 2,] }",
        "func f(): void { [1, 2; 3] }",
        "func f(): void { [1; 2; 3] }",
        "func f(): void { [1 2] }",
        "func f(): void { [...items] }",
        "func f(): void { [name: value] }",
        "func f(): void { items[] }",
        "func f(): void { items[1, 2] }",
        "func f(): void { items [0] }",
        "func f(): void { [1; count() }",
    ] {
        let parsed = tree(text, ParseGoal::V2CoreUnit);
        assert!(parsed.has_errors(), "{text}");
        super::super::snapshots::assert_token_projection(&parsed, text);
    }
    let text = format!(
        "func f(): void {{ {}0{} }}",
        "[".repeat(500),
        "; 0]".repeat(500)
    );
    let parsed = tree(&text, ParseGoal::V2CoreUnit);
    assert!(parsed.has_errors());
    super::super::snapshots::assert_token_projection(&parsed, "array value depth");
}

#[test]
fn fallible_suffixes_preserve_grouping_and_cannot_consume_another_suffix() {
    for ty in ["i32!", "i32??!", "(i32!)?", "[i32!; 0]!", "(i32!, error!)"] {
        let text = format!("func forward(take value: {ty}): {ty} {{ value^ }}");
        let parsed = tree(&text, ParseGoal::V2CoreUnit);
        assert!(!parsed.has_errors(), "{text}: {:?}", parsed.diagnostics());
        super::super::snapshots::assert_token_projection(&parsed, &text);
    }
    for ty in ["i32!?", "i32!!", "i32??!?", "(i32!)!?"] {
        let text = format!("func forward(take value: {ty}): void {{}}");
        assert!(tree(&text, ParseGoal::V2CoreUnit).has_errors(), "{text}");
    }
    // Grouped repetition is syntactically complete; the declaration owner rejects its meaning.
    assert!(
        !tree(
            "func invalid(value: (i32!)!): void {}",
            ParseGoal::V2CoreUnit
        )
        .has_errors()
    );
}

#[test]
fn throw_requires_an_operand_and_uses_ordinary_incomplete_statement_continuation() {
    for operand in ["reason^", "\nreason^"] {
        let text = format!("func fail(take reason: error): i32! {{ throw {operand} }}");
        let parsed = tree(&text, ParseGoal::V2CoreUnit);
        assert!(!parsed.has_errors(), "{text}: {:?}", parsed.diagnostics());
        super::super::snapshots::assert_token_projection(&parsed, &text);
    }
    for operand in ["", "\n\nreason^"] {
        let text = format!("func fail(take reason: error): i32! {{ throw {operand} }}");
        assert!(tree(&text, ParseGoal::V2CoreUnit).has_errors(), "{text}");
    }
}

#[test]
fn catch_requires_a_named_or_discard_handler_and_chains_in_source_order() {
    for expression in [
        "operation() catch failure { 42 }",
        "operation() catch _ { 42 }",
        "operation()\ncatch failure { 42 } otherwise { 0 }",
        "operation() otherwise { 42 } catch failure { 0 }",
    ] {
        let text = format!("func run(): i32 {{ {expression} }}");
        let parsed = tree(&text, ParseGoal::V2CoreUnit);
        assert!(!parsed.has_errors(), "{text}: {:?}", parsed.diagnostics());
        super::super::snapshots::assert_token_projection(&parsed, &text);
    }
    for expression in [
        "operation() catch { 42 }",
        "operation() catch failure",
        "operation() catch failure other { 42 }",
    ] {
        let text = format!("func run(): i32 {{ {expression} }}");
        assert!(tree(&text, ParseGoal::V2CoreUnit).has_errors(), "{text}");
    }
}

#[test]
fn plain_text_uses_shared_literal_recognition_without_interpolation_fallback() {
    for text in [
        "\"\"",
        "\"night 夜\\0\\n\"",
        "\"\"\"\n  first\n  second\n  \"\"\"",
    ] {
        let text = format!("func run(): str {{ {text} }}");
        let parsed = tree(&text, ParseGoal::V2CoreUnit);
        assert!(!parsed.has_errors(), "{text}: {:?}", parsed.diagnostics());
        super::super::snapshots::assert_token_projection(&parsed, &text);
    }
    assert!(
        tree(
            "func run(): str { \"before ${value} after\" }",
            ParseGoal::V2CoreUnit
        )
        .has_errors()
    );
}

#[test]
fn primitive_functions_are_bodyless_without_ordinary_implementation_syntax() {
    let text = "primitive func new_error(code: str, message: str): error";
    let parsed = tree(text, ParseGoal::V2CoreUnit);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    super::super::snapshots::assert_token_projection(&parsed, text);
    for text in [
        "primitive func raw(): void {}",
        "const primitive func raw(): void",
    ] {
        assert!(tree(text, ParseGoal::V2CoreUnit).has_errors(), "{text}");
    }
}

#[test]
fn source_and_module_dependencies_share_their_exact_path_grammar_before_items() {
    let text = "see ./body.nct\nsee ../../details/body.nct\nuse ./codec as encoding\nuse ./codec.{Decoder, Format as WireFormat}\nfunc run(): i32 { 42 }";
    let parsed = tree(text, ParseGoal::V2CoreUnit);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    assert_eq!(
        parsed
            .nodes()
            .filter(|(_, n)| n.kind() == NodeKind::SourceVisibilityDeclaration)
            .count(),
        2
    );
    assert_eq!(
        parsed
            .nodes()
            .filter(|(_, n)| n.kind() == NodeKind::UseDeclaration)
            .count(),
        2
    );
    super::super::snapshots::assert_token_projection(&parsed, text);
    for source in [
        "pub use ./codec",
        "see codec.nct",
        "see ./codec",
        "see ./codec/",
        "see ./../body.nct",
        "see ./body.nct as body",
        "func run(): void {}\nsee ./body.nct",
        "func run(): void {}\nuse ./codec",
    ] {
        let parsed = tree(source, ParseGoal::V2CoreUnit);
        assert!(parsed.has_errors(), "{source}");
        super::super::snapshots::assert_token_projection(&parsed, source);
    }
}

#[test]
fn grouped_item_directives_attach_once_and_keep_field_and_use_visibility_structural() {
    use crate::{ParseDiagnosticKind, child_node_iter, direct_node, item_declaration};
    let text = "#visibility: \"/\"\n// keep the same prefix across trivia\n\n#target: \"arm64-darwin\"\nfunc first(): i32\nfunc second(): i32\n#visibility: \"./\"\nstruct Data {\n#visibility: \"/\"\nvalue: i32\n}";
    let parsed = tree(text, ParseGoal::V2CoreUnit);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let items = child_node_iter(&parsed, parsed.root_id()).collect::<Vec<_>>();
    assert_eq!(items.len(), 3);
    assert!(direct_node(&parsed, items[0], NodeKind::Visibility).is_some());
    assert!(direct_node(&parsed, items[0], NodeKind::TargetDirective).is_some());
    assert!(direct_node(&parsed, items[1], NodeKind::Visibility).is_none());
    assert!(
        items
            .iter()
            .all(|item| item_declaration(&parsed, *item).is_some())
    );
    let field = parsed
        .nodes()
        .find(|(_, node)| node.kind() == NodeKind::StructField)
        .unwrap()
        .0;
    assert!(direct_node(&parsed, field, NodeKind::Visibility).is_some());
    let name = crate::declaration_name_token(&parsed, field).unwrap();
    assert_eq!(
        &text[usize::try_from(name.range().start().get()).unwrap()
            ..usize::try_from(name.range().end().get()).unwrap()],
        "value"
    );
    super::super::snapshots::assert_token_projection(&parsed, text);
    let parsed = tree(
        "#visibility: \"/\"\nuse ./codec.{Data, Item as Other}",
        ParseGoal::V2CoreUnit,
    );
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let dependency = child_node_iter(&parsed, parsed.root_id()).next().unwrap();
    assert_eq!(
        parsed.node(dependency).unwrap().kind(),
        NodeKind::UseDeclaration
    );
    assert!(direct_node(&parsed, dependency, NodeKind::Visibility).is_some());
    for text in [
        "#visibility: \"/\"\n#visibility: \"./\"\nfunc f(): void",
        "#target: \"arm64-darwin\"\n#visibility: \"/\"\n#target: \"arm64-darwin\"\nfunc f(): void",
    ] {
        let parsed = tree(text, ParseGoal::V2CoreUnit);
        assert!(
            parsed
                .diagnostics()
                .iter()
                .any(|d| d.kind() == ParseDiagnosticKind::DuplicateItemDirective)
        );
    }
    for text in [
        "#visibility: \"/\"",
        "#visibility: \"/\" func f(): void",
        "#unknown: \"/\"\nfunc f(): void",
        "#visibility: \"/\"\nsee ./body.nct",
        "#visibility: \"/\"\nfunc f(): void\n#visibility: \"/\"\nuse ./codec",
    ] {
        assert!(tree(text, ParseGoal::V2CoreUnit).has_errors(), "{text}");
    }
    assert!(tree("#visibility: \"/\"\nfunc f(): void", ParseGoal::SourceFile).has_errors());
}

#[test]
fn block_imports_precede_executable_entries_and_retain_exact_dependency_nodes() {
    for text in [
        "func run(): i32 { use ./codec\n42 }",
        "func run(): void { use ./codec }",
        "func run(): i32 { if true { use ./codec as encoding\n42 } else { 0 } }",
    ] {
        let parsed = tree(text, ParseGoal::V2CoreUnit);
        assert!(!parsed.has_errors(), "{text}: {:?}", parsed.diagnostics());
        assert_eq!(
            parsed
                .nodes()
                .filter(|(_, node)| node.kind() == NodeKind::BlockUseDeclaration)
                .count(),
            1
        );
    }
    let parsed = tree(
        "func run(): void { let value = 42\nuse ./codec }",
        ParseGoal::V2CoreUnit,
    );
    assert!(parsed.diagnostics().iter().any(
        |diagnostic| diagnostic.kind() == crate::ParseDiagnosticKind::LateDependencyDeclaration
    ));
}
