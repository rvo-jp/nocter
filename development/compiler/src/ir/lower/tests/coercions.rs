use super::*;

#[test]
fn lowers_a_generic_borrow_coercion_as_a_reachable_callable() {
    let ir = lower_text(
        r#"struct Box<T> { value: T }
coerce Box<T> {
    pub &self as &T from self { return &self.value }
}
func accept(value: &i32): i32 { return 7 }
func main(): i32 {
    let box = Box<i32> { value: 42 }
    return accept(&box)
}
"#,
    );
    let coercion = ir
        .functions
        .iter()
        .find(|function| function.name.starts_with("Box<i32>.__nocter$coerce$"))
        .expect("expected specialized coercion callable");
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected main");

    assert!(
        main.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::CallBorrow { target, .. } if target == &coercion.target
            )
        }),
        "{main:#?}"
    );
    assert!(
        main.instructions.iter().any(|instruction| {
            matches!(
            instruction,
            Instruction::CallI32 { target, arguments, .. }
                if target == &CallTarget::same_file("accept")
                        && matches!(arguments.as_slice(), [ScalarArgument::Borrow(BorrowArgument {
                            source: BorrowSource::BorrowLocal(_),
                        })])
            )
        }),
        "{main:#?}"
    );
}

#[test]
fn lowers_string_view_coercion_through_the_same_plan() {
    let ir = lower_text(
        r#"struct Text { data: &str }
coerce Text {
    pub &self as &str from self { return self.data }
}
func accept(value: &str): i32 { return 1 }
func main(): i32 {
    let text = Text { data: "hello" }
    return accept(&text)
}
        "#,
    );
    let coercion = ir
        .functions
        .iter()
        .find(|function| function.name.starts_with("Text.__nocter$coerce$"))
        .expect("expected coercion");
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected main");

    assert!(
        main.instructions.iter().any(|instruction| {
            matches!(instruction, Instruction::CallStr { target, .. } if target == &coercion.target)
        }),
        "{main:#?}"
    );
}
