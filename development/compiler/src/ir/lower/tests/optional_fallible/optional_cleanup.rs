use super::*;

#[test]
fn optional_propagation_drops_owned_locals_before_returning_none() {
    let token_type = Type::DirectAggregate {
        layout: ValueLayout::new(4, 4),
        words: 1,
    };
    let function = lower_named_function_with_signatures(
        r#"struct Token {
    value: i32
}

impl Token {
    drop &+self {
        return
    }
}

func main(): i32 {
    return forward() otherwise { 7 }
}

func forward(): i32? {
    var token = Token { value: 1 }
    let value = maybe()?
    drop token
    return value
}

func maybe(): i32? {
    return none
}
"#,
        "forward",
        function_signatures(vec![
            (
                "Token.drop",
                Type::Void,
                vec![Type::Borrow {
                    is_readwrite: true,
                    inner: Box::new(token_type),
                }],
            ),
            ("maybe", Type::Fallible(Box::new(Type::I32)), vec![]),
        ]),
    )
    .unwrap();

    let Some(Instruction::CallFallibleI32 {
        failure_mode: FallibleFailureMode::Handle { instructions },
        ..
    }) = function
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, Instruction::CallFallibleI32 { .. }))
    else {
        panic!("{function:?}");
    };
    assert!(matches!(
        instructions.last(),
        Some(Instruction::ReturnOptionalNone)
    ));
    assert!(
        instructions.iter().any(|instruction| matches!(
            instruction,
            Instruction::CallVoid { target, .. } if *target == CallTarget::same_file("Token.drop")
        )),
        "{function:?}"
    );
}

#[test]
fn optional_aggregate_exiting_fallback_does_not_drop_an_uninitialized_destination() {
    let ir = lower_text(
        r#"struct Token {
    label: &str
}

impl Token {
    drop &+self {
        return
    }
}

func maybe(flag: bool): Token? {
    if flag {
        return Token { label: "value" }
    }
    return none
}

func main(): i32 {
    loop {
        let token = maybe(false) otherwise { break }
        drop token
    }
    return 0
}
"#,
    );
    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    let Instruction::While {
        body_instructions, ..
    } = &main.instructions[0]
    else {
        panic!("{main:?}");
    };
    let failure_mode = body_instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::CallFallibleDirectAggregate { failure_mode, .. }
            | Instruction::CallFallibleAggregate { failure_mode, .. } => Some(failure_mode),
            _ => None,
        });

    assert_eq!(
        failure_mode,
        Some(&FallibleFailureMode::Handle {
            instructions: vec![Instruction::Break],
        }),
        "{main:?}"
    );
}
