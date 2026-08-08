use super::*;

fn count_borrow_calls(instructions: &[Instruction], target: &CallTarget) -> usize {
    instructions
        .iter()
        .map(|instruction| match instruction {
            Instruction::CallBorrow {
                target: call_target,
                ..
            } if call_target == target => 1,
            Instruction::If {
                then_instructions,
                else_instructions,
                ..
            } => {
                count_borrow_calls(then_instructions, target)
                    + count_borrow_calls(else_instructions, target)
            }
            _ => 0,
        })
        .sum()
}

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

#[test]
fn lowers_receiver_coercion_before_the_selected_source_method() {
    let ir = lower_text_with_nocter_home_files(
        r#"struct Text { data: &str }
coerce Text {
    pub &self as &str { return self.data }
}
func main(): usize {
    let text = Text { data: "hello" }
    return text.count()
}
"#,
        &[(
            "std/str/index.nct",
            "impl str { pub method &self.count(): usize { return 5 } }\n",
        )],
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

    let coercion_index = main
        .instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, Instruction::CallStr { target, .. } if target == &coercion.target)
        })
        .expect("receiver coercion call");
    let method_index = main
        .instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, Instruction::CallUsize { target: CallTarget::Imported { name, .. }, .. } | Instruction::TailCall { target: CallTarget::Imported { name, .. }, .. } if name == "str.count")
        })
        .unwrap_or_else(|| panic!("source-declared str method call: {main:#?}"));
    assert!(coercion_index < method_index, "{main:#?}");
}

#[test]
fn lowers_one_coercion_plan_at_each_expected_type_boundary() {
    let ir = lower_text(
        r#"struct Box<T> { value: T }
coerce Box<T> { pub &self as &T from self { return &self.value } }
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
func main(): i32 {
    let box = Box<i32> { value: 42 }
    let projected = project(&box)
    return 0
}
"#,
    );
    let coercion = ir
        .functions
        .iter()
        .find(|function| function.name.starts_with("Box<i32>.__nocter$coerce$"))
        .expect("expected specialized coercion callable");
    let project = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("project"))
        .expect("expected project function");
    let coercion_calls = count_borrow_calls(&project.instructions, &coercion.target);

    assert_eq!(coercion_calls, 6, "{project:#?}");
}

#[test]
fn lowers_explicit_as_through_the_selected_coercion_body() {
    let ir = lower_text(
        r#"struct Box<T> { value: T }
coerce Box<T> { pub &self as &T from self { return &self.value } }
func accept(value: &i32): i32 { return 7 }
func main(): i32 {
    let box = Box<i32> { value: 42 }
    return accept(&box as &i32)
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

    assert!(main.instructions.iter().any(|instruction| {
        matches!(instruction, Instruction::CallBorrow { target, .. } if target == &coercion.target)
    }));
}

#[test]
fn explicit_coercion_evaluates_its_source_once() {
    let ir = lower_text(
        r#"struct Box<T> { value: T }
coerce Box<T> { pub &self as &T from self { return &self.value } }
func borrow(value: &Box<i32>): &Box<i32> from value { return value }
func accept(value: &i32): i32 { return 7 }
func main(): i32 {
    let box = Box<i32> { value: 42 }
    return accept(borrow(&box) as &i32)
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
    let source_calls = main
        .instructions
        .iter()
        .filter(|instruction| {
            matches!(
                instruction,
                Instruction::CallBorrow { target, .. } if target == &CallTarget::same_file("borrow")
            )
        })
        .count();
    let coercion_calls = main
        .instructions
        .iter()
        .filter(|instruction| {
            matches!(instruction, Instruction::CallBorrow { target, .. } if target == &coercion.target)
        })
        .count();

    assert_eq!(source_calls, 1, "{main:#?}");
    assert_eq!(coercion_calls, 1, "{main:#?}");
}

#[test]
fn lowers_contextual_coercions_on_each_compound_expression_result() {
    let ir = lower_text(
        r#"struct Box<T> { value: T }
coerce Box<T> { pub &self as &T from self { return &self.value } }
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
func main(): i32 {
    let box = Box<i32> { value: 42 }
    let result = project(Choice.first, &box)
    return 0
}
"#,
    );
    let coercion = ir
        .functions
        .iter()
        .find(|function| function.name.starts_with("Box<i32>.__nocter$coerce$"))
        .expect("expected specialized coercion callable");
    let project = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("project"))
        .expect("expected project function");
    let coercion_calls = count_borrow_calls(&project.instructions, &coercion.target);

    assert_eq!(coercion_calls, 6, "{project:#?}");
}
