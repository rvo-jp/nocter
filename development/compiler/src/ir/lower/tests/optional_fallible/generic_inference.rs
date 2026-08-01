use super::*;

#[test]
fn lowers_generic_function_call_inferred_from_catch_block_return_type() {
    let ir = lower_text(
        r#"struct Marker<T> {
    code: i32
}

func make<T>(): Marker<T> {
    return Marker<T> { code: 42 }
}

func source(): Marker<u8>! {
    return Marker<u8> { code: 1 }
}

func recover(): Marker<u8> {
    return source() catch error {
        return make()
    }
}

func main(): i32 {
    return recover().code
}
"#,
    );

    let specialized_target = CallTarget::same_file("make<u8>");
    let recover = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("recover"))
        .expect("expected lowered recover function");

    let catch_instructions = recover.instructions.iter().find_map(|instruction| {
        if let Instruction::CallFallibleDirectAggregate {
            failure_mode: FallibleFailureMode::Catch { instructions, .. },
            ..
        } = instruction
        {
            Some(instructions)
        } else {
            None
        }
    });
    let catch_instructions = catch_instructions.expect("expected aggregate catch call");
    assert!(
        catch_instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::CallDirectAggregate { target, .. } if target == &specialized_target
            )
        }),
        "{recover:?}"
    );
    assert!(
        ir.functions
            .iter()
            .any(|function| function.target == specialized_target),
        "{ir:?}"
    );
}
