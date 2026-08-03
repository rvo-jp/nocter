use super::*;
use crate::ir::ComposedOutcomeDestination;
use crate::outcomes::OutcomeLayer;

#[test]
fn lowers_fallible_optional_present_return_with_two_explicit_tags() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func lookup(): i32?! {
    return 42
}
"#,
        "lookup",
    );

    assert_eq!(
        function.return_type,
        Type::ComposedOutcome {
            outer: OutcomeLayer::Fallible,
            inner: OutcomeLayer::Optional,
            payload: Box::new(Type::I32),
        }
    );
    assert_eq!(
        function.instructions,
        vec![
            Instruction::SetI32 {
                destination: I32Location::Return,
                value: I32Value::Const(42),
            },
            Instruction::ReturnFallibleSuccess,
        ]
    );
}

#[test]
fn lowers_fallible_optional_none_as_successful_absence() {
    let function = lower_named_function(
        r#"func main(): i32 {
    return 0
}

func lookup(): i32?! {
    return none
}
"#,
        "lookup",
    );

    assert_eq!(function.instructions, vec![Instruction::ReturnOptionalNone]);
}

#[test]
fn lowers_immediate_fallible_propagation_then_optional_fallback() {
    let function = lower_named_function_with_signatures(
        r#"func main(): i32! {
    let value = lookup(0)? otherwise { return 7 }
    return value
}

func lookup(mode: i32): i32?! {
    if mode == 0 {
        return 42
    }
    return none
}
"#,
        "main",
        function_signatures(vec![(
            "lookup",
            Type::ComposedOutcome {
                outer: OutcomeLayer::Fallible,
                inner: OutcomeLayer::Optional,
                payload: Box::new(Type::I32),
            },
            vec![Type::I32],
        )]),
    )
    .unwrap();

    let call = function.instructions.iter().find_map(|instruction| {
        let Instruction::CallComposedOutcome {
            destination,
            target,
            outer,
            inner,
            outer_mode,
            inner_mode,
            ..
        } = instruction
        else {
            return None;
        };
        Some((destination, target, outer, inner, outer_mode, inner_mode))
    });
    let Some((
        ComposedOutcomeDestination::I32(I32Location::Local(0)),
        target,
        OutcomeLayer::Fallible,
        OutcomeLayer::Optional,
        FallibleFailureMode::Propagate,
        FallibleFailureMode::Handle { instructions },
    )) = call
    else {
        panic!("{function:?}");
    };
    assert_eq!(*target, CallTarget::same_file("lookup"));
    assert!(instructions.contains(&Instruction::ReturnFallibleSuccess));
}
