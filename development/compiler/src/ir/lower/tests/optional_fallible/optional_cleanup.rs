use super::*;

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
