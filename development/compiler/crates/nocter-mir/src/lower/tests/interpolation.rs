use nocter_test_support::CompilerFixture;

use super::lower_compiler_fixture;
use crate::{MirOperationKind, MirReadMode, MirTerminator};

#[test]
fn lowers_as_ordered_selected_string_operations() {
    let fixture = CompilerFixture::with_app_interpolation_standard_uses(
        "use std.String\n\
         use std.Format\n\
         struct Value {}\n\
         instance Value {\n\
             impl Format\n\
             method &self.try_format_into(output: &+String): void! { return }\n\
         }\n\
         func render(value: &Value): String {\n\
             \"before ${value} after\"\n\
         }\n\
         func main(): void {\n\
             let value = Value {}\n\
             let text = render(&value)\n\
             drop text\n\
             return\n\
         }\n",
        &[&[], &[]],
    );
    let program = lower_compiler_fixture(&fixture).unwrap();
    let function = function_with_text(&program, "before ");
    let steps = function
        .operations()
        .iter()
        .filter_map(|(_, operation)| {
            let MirOperationKind::Call(call) = operation.kind() else {
                return None;
            };
            if call.arguments().is_empty() {
                return Some("constructor".to_owned());
            }
            let text = call.arguments().get(1).and_then(|argument| {
                let value = function.values().get(*argument)?;
                let crate::MirValueDefinition::Operation(operation) = value.definition() else {
                    return None;
                };
                match function.operations().get(operation)?.kind() {
                    MirOperationKind::Constant(crate::MirConstant::Text(text)) => {
                        Some(text.as_ref())
                    }
                    _ => None,
                }
            });
            Some(text.map_or_else(|| "formatter".to_owned(), |text| format!("text:{text}")))
        })
        .collect::<Vec<_>>();

    assert_eq!(
        steps,
        ["constructor", "text:before ", "formatter", "text: after"]
    );
    assert!(function.operations().iter().any(|(_, operation)| {
        matches!(
            operation.kind(),
            MirOperationKind::Read {
                mode: MirReadMode::Move,
                ..
            }
        )
    }));
}

#[test]
fn propagation_drops_the_partial_output_storage() {
    let fixture = CompilerFixture::with_app_interpolation_standard_uses(
        "use std.String\n\
         use std.Format\n\
         struct Value {}\n\
         instance Value {\n\
             impl Format\n\
             method &self.try_format_into(output: &+String): void! { return }\n\
         }\n\
         func render(input: Value?): String? {\n\
             \"prefix ${move input?}\"\n\
         }\n\
         func main(): void {\n\
             let result = render(none)\n\
             drop result\n\
             return\n\
         }\n",
        &[&[], &[]],
    );
    let program = lower_compiler_fixture(&fixture).unwrap();
    let function = function_with_text(&program, "prefix ");
    let output = interpolation_output_storage(function);

    assert!(function.operations().iter().any(|(_, operation)| {
        matches!(operation.kind(), MirOperationKind::InvokeDrop { place, .. } if *place == output)
    }));
    assert!(function.operations().iter().any(|(_, operation)| {
        matches!(
            operation.kind(),
            MirOperationKind::Read {
                place,
                mode: MirReadMode::Move,
            } if *place == output
        )
    }));
    assert!(
        function
            .blocks()
            .iter()
            .any(|(_, block)| matches!(block.terminator(), MirTerminator::Return(Some(_))))
    );
}

#[test]
fn explicit_return_drops_the_partial_output_storage() {
    let fixture = CompilerFixture::with_app_interpolation_standard_uses(
        "use std.String\n\
         use std.Format\n\
         struct Value {}\n\
         instance Value {\n\
             impl Format\n\
             method &self.try_format_into(output: &+String): void! { return }\n\
         }\n\
         func render(exit: bool, value: &Value): String {\n\
             \"prefix ${if exit { return String.empty() } else { value }}\"\n\
         }\n\
         func main(): void {\n\
             let value = Value {}\n\
             let text = render(true, &value)\n\
             drop text\n\
             return\n\
         }\n",
        &[&[], &[]],
    );
    let program = lower_compiler_fixture(&fixture).unwrap();
    let function = function_with_text(&program, "prefix ");
    let output = interpolation_output_storage(function);

    assert!(function.operations().iter().any(|(_, operation)| {
        matches!(operation.kind(), MirOperationKind::InvokeDrop { place, .. } if *place == output)
    }));
}

#[test]
fn forced_operand_traps_without_partial_output_cleanup() {
    let fixture = CompilerFixture::with_app_interpolation_standard_uses(
        "use std.String\n\
         use std.Format\n\
         struct Value {}\n\
         instance Value {\n\
             impl Format\n\
             method &self.try_format_into(output: &+String): void! { return }\n\
         }\n\
         func render(input: Value?): String {\n\
             \"prefix ${move input!}\"\n\
         }\n\
         func main(): void {\n\
             let text = render(none)\n\
             drop text\n\
             return\n\
         }\n",
        &[&[], &[]],
    );
    let program = lower_compiler_fixture(&fixture).unwrap();
    let function = function_with_text(&program, "prefix ");

    assert!(
        function
            .blocks()
            .iter()
            .any(|(_, block)| matches!(block.terminator(), MirTerminator::Trap))
    );
    assert!(
        !function.operations().iter().any(|(_, operation)| {
            matches!(operation.kind(), MirOperationKind::InvokeDrop { .. })
        })
    );
}

fn function_with_text<'program>(
    program: &'program crate::MirProgram,
    expected: &str,
) -> &'program crate::MirFunction {
    program
        .functions()
        .iter()
        .find_map(|(_, function)| {
            function
                .operations()
                .iter()
                .any(|(_, operation)| {
                    matches!(
                        operation.kind(),
                        MirOperationKind::Constant(crate::MirConstant::Text(text))
                            if text.as_ref() == expected
                    )
                })
                .then_some(function)
        })
        .expect("function containing expected text constant")
}

fn interpolation_output_storage(function: &crate::MirFunction) -> nocter_model::MirPlaceId {
    let constructed = function
        .operations()
        .iter()
        .find_map(|(_, operation)| match operation.kind() {
            MirOperationKind::Call(call) if call.arguments().is_empty() => operation.result(),
            _ => None,
        })
        .expect("interpolation constructor result");
    function
        .operations()
        .iter()
        .find_map(|(_, operation)| match operation.kind() {
            MirOperationKind::Initialize { destination, value } if *value == constructed => {
                Some(*destination)
            }
            _ => None,
        })
        .expect("canonical partial output storage")
}
