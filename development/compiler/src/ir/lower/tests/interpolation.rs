use super::*;

#[test]
fn lowers_interpolation_through_trusted_constructor_and_formatters() {
    let ir = lower_text_with_development_home(
        r#"func main(): i32 {
    let byte: u8 = 255
    let text = "value ${42} byte ${byte} ready ${true}"
    return 0
}
"#,
    );
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .unwrap();
    let called_names = main
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            Instruction::CallAggregate { target, .. }
            | Instruction::CallDirectAggregate { target, .. }
            | Instruction::CallVoid { target, .. } => Some(match target {
                CallTarget::SameFile(name) | CallTarget::Imported { name, .. } => name.as_str(),
            }),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        &called_names[..7],
        [
            "String.with_capacity",
            "str.format_into",
            "i32.format_into",
            "str.format_into",
            "u8.format_into",
            "str.format_into",
            "bool.format_into",
        ]
    );
    assert_eq!(called_names.last(), Some(&"RawBuffer.drop"));
}

#[test]
fn propagation_from_an_interpolation_part_drops_the_partial_string() {
    let ir = lower_text_with_development_home(
        r#"func value(): i32! {
    return 42
}

func main(): i32! {
    let text = "partial ${value()?}"
    return 0
}
"#,
    );
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .unwrap();
    let cleanup = main
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::CallOutcomeI32 {
                target,
                failure_mode: OutcomeFailureMode::PropagateWithCleanup { instructions, .. },
                ..
            } if target == &CallTarget::same_file("value") => Some(instructions),
            _ => None,
        });
    let cleanup = cleanup.expect("interpolation propagation should carry cleanup");

    assert_eq!(
        cleanup
            .iter()
            .filter(|instruction| matches!(
                instruction,
                Instruction::CallVoid {
                    target: CallTarget::Imported { name, .. },
                    ..
                } if name == "RawBuffer.drop"
            ))
            .count(),
        1
    );
}

#[test]
fn interpolation_caller_uses_the_retained_mir_body() {
    let fixture = analyze_text_fixture_with_development_home(
        r#"func main(): i32 {
    let text = "value ${42}"
    return 0
}
"#,
    );
    lower_executable(&fixture.analysis, &fixture.sources).unwrap();

    let root = fixture.analysis.root_file().unwrap();
    let body = root
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            crate::ast::Item::Function(function) if function.name == "main" => {
                function.body.as_ref()
            }
            _ => None,
        })
        .and_then(|body| fixture.analysis.semantic_db.body_at(body.span))
        .unwrap();
    let cached =
        fixture
            .analysis
            .mir_bodies
            .cached_specialized(root.ast.span.source, body, &HashMap::new());
    assert!(matches!(cached, Some(Ok(_))), "{cached:?}");
}
