use super::*;

#[test]
fn lowers_concrete_generic_impl_method_call() {
    let ir = lower_text(
        r#"struct Box<T> {
    value: T
}

impl Box<i32> {
    method &self.read(): i32 {
        return self.value
    }
}

func main(): i32 {
    let box = Box<i32> { value: 42 }
    return box.read()
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");

    assert!(
        main.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::CallI32 {
                    destination: I32Location::Return,
                    target,
                    arguments,
                } if target == &CallTarget::same_file("Box.read")
                    && arguments == &vec![ScalarArgument::Borrow(BorrowArgument {
                        source: BorrowSource::AggregateSlot(0),
                    })]
            )
        }),
        "{main:?}"
    );
}

#[test]
fn lowers_generic_function_call_with_concrete_arguments() {
    let ir = lower_text(
        r#"func identity<T>(value: T): T {
    return value
}

func main(): i32 {
    return identity(42)
}
"#,
    );

    let specialized_target = CallTarget::same_file("identity<i32>");
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");

    assert!(
        main.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::TailCall {
                    target,
                    arguments,
                } if target == &specialized_target
                    && arguments == &vec![ScalarArgument::I32(i32_const(42))]
            )
        }),
        "{main:?}"
    );

    let function = ir
        .functions
        .iter()
        .find(|function| function.target == specialized_target)
        .expect("expected lowered specialized function");

    assert_eq!(function.name, "identity<i32>");
    assert_eq!(function.return_type, Type::I32);
}

#[test]
fn lowers_generic_associated_function_call_with_concrete_arguments() {
    let ir = lower_text(
        r#"struct Box<T> {
    value: T
}

func Box.unwrap<T>(box: Box<T>): T {
    return box.value
}

func main(): i32 {
    let box = Box<i32> { value: 42 }
    return Box.unwrap(move box)
}
"#,
    );

    let specialized_target = CallTarget::same_file("Box.unwrap<i32>");
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");

    assert!(
        main.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::TailCall {
                    target,
                    arguments,
                } if target == &specialized_target && arguments.len() == 1
            )
        }),
        "{main:?}"
    );

    let function = ir
        .functions
        .iter()
        .find(|function| function.target == specialized_target)
        .expect("expected lowered specialized associated function");

    assert_eq!(function.name, "Box.unwrap<i32>");
    assert_eq!(function.return_type, Type::I32);
}

#[test]
fn lowers_generic_function_call_inferred_from_binding_annotation() {
    let ir = lower_text(
        r#"struct Marker<T> {
    code: i32
}

func make<T>(): Marker<T> {
    return Marker<T> { code: 42 }
}

func main(): i32 {
    let marker: Marker<u8> = make()
    return marker.code
}
"#,
    );

    let specialized_target = CallTarget::same_file("make<u8>");
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");

    assert!(
        main.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::CallAggregate { target, .. }
                    | Instruction::CallDirectAggregate { target, .. }
                    if target == &specialized_target
            )
        }),
        "{main:?}"
    );
    assert!(
        ir.functions
            .iter()
            .any(|function| function.target == specialized_target),
        "{ir:?}"
    );
}

#[test]
fn lowers_generic_function_call_inferred_from_parameter_type() {
    let ir = lower_text(
        r#"struct Marker<T> {
    code: i32
}

func make<T>(): Marker<T> {
    return Marker<T> { code: 42 }
}

func consume(marker: Marker<u8>): i32 {
    return marker.code
}

func main(): i32 {
    return consume(make())
}
"#,
    );

    let specialized_target = CallTarget::same_file("make<u8>");
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");

    assert!(
        main.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::CallDirectAggregate { target, .. } if target == &specialized_target
            )
        }),
        "{main:?}"
    );
    assert!(
        ir.functions
            .iter()
            .any(|function| function.target == specialized_target),
        "{ir:?}"
    );
}

#[test]
fn lowers_nested_generic_function_call_inferred_from_parameter_type() {
    let ir = lower_text(
        r#"copy struct Marker<T> {
    code: i32
}

func make<T>(): Marker<T> {
    return Marker<T> { code: 42 }
}

func forward<T>(value: T): T {
    return value
}

func consume(marker: Marker<u8>): i32 {
    return marker.code
}

func main(): i32 {
    return consume(forward(make()))
}
"#,
    );

    let make_target = CallTarget::same_file("make<u8>");
    let forward_target = CallTarget::same_file("forward<Marker<u8>>");
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");

    assert!(
        main.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::CallDirectAggregate { target, .. } if target == &make_target
            )
        }),
        "{main:?}"
    );
    assert!(
        main.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::CallDirectAggregate { target, .. } if target == &forward_target
            )
        }),
        "{main:?}"
    );
    assert!(
        ir.functions
            .iter()
            .any(|function| function.target == make_target),
        "{ir:?}"
    );
    assert!(
        ir.functions
            .iter()
            .any(|function| function.target == forward_target),
        "{ir:?}"
    );
}

#[test]
fn lowers_generic_function_body_call_with_concrete_arguments() {
    let ir = lower_text(
        r#"func identity<T>(value: T): T {
    return value
}

func forward<T>(value: T): T {
    return identity(value)
}

func main(): i32 {
    return forward(42)
}
"#,
    );

    let forward_target = CallTarget::same_file("forward<i32>");
    let identity_target = CallTarget::same_file("identity<i32>");
    let forward = ir
        .functions
        .iter()
        .find(|function| function.target == forward_target)
        .expect("expected lowered specialized forward function");

    assert_eq!(forward.return_type, Type::I32);
    assert!(
        forward.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::TailCall {
                    target,
                    arguments,
                } if target == &identity_target && arguments.len() == 1
            )
        }),
        "{forward:?}"
    );
    assert!(
        ir.functions
            .iter()
            .any(|function| function.target == identity_target && function.return_type == Type::I32),
        "{ir:?}"
    );
}

#[test]
fn lowers_generic_impl_method_body_function_call_with_concrete_receiver() {
    let ir = lower_text(
        r#"struct Box<T> {
    value: T
}

func identity<T>(value: T): T {
    return value
}

impl<U> Box<U> {
    method self.into_identity(): U {
        return identity(self.value)
    }
}

func main(): i32 {
    let box = Box<i32> { value: 42 }
    return (move box).into_identity()
}
"#,
    );

    let method_target = CallTarget::same_file("Box<i32>.into_identity");
    let identity_target = CallTarget::same_file("identity<i32>");
    let method = ir
        .functions
        .iter()
        .find(|function| function.target == method_target)
        .expect("expected lowered specialized method");

    assert_eq!(method.return_type, Type::I32);
    assert!(
        method.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::TailCall {
                    target,
                    arguments,
                } if target == &identity_target && arguments.len() == 1
            )
        }),
        "{method:?}"
    );
    assert!(
        ir.functions
            .iter()
            .any(|function| function.target == identity_target && function.return_type == Type::I32),
        "{ir:?}"
    );
}

#[test]
fn lowers_generic_function_body_method_call_with_concrete_arguments() {
    let ir = lower_text(
        r#"struct Box<T> {
    value: T
}

impl<U> Box<U> {
    method self.into_value(): U {
        return self.value
    }
}

func forward<T>(box: Box<T>): T {
    return (move box).into_value()
}

func main(): i32 {
    let box = Box<i32> { value: 42 }
    return forward(move box)
}
"#,
    );

    let forward_target = CallTarget::same_file("forward<i32>");
    let method_target = CallTarget::same_file("Box<i32>.into_value");
    let forward = ir
        .functions
        .iter()
        .find(|function| function.target == forward_target)
        .expect("expected lowered specialized forward function");

    assert_eq!(forward.return_type, Type::I32);
    assert!(
        forward.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::TailCall {
                    target,
                    arguments,
                } if target == &method_target && arguments.len() == 1
            )
        }),
        "{forward:?}"
    );
    assert!(
        ir.functions
            .iter()
            .any(|function| function.target == method_target && function.return_type == Type::I32),
        "{ir:?}"
    );
}

#[test]
fn lowers_interface_bound_method_call_to_concrete_static_target() {
    let ir = lower_text(
        r#"interface Extract<T> {
    pub method self.into_value(): T
}

struct Box<T> {
    value: T
}

impl<T> Extract<T> for Box<T> {
    method self.into_value(): T {
        return self.value
    }
}

func forward<B: Extract<T>, T>(box: B): T {
    return (move box).into_value()
}

func main(): i32 {
    let box = Box<i32> { value: 42 }
    return forward(move box)
}
"#,
    );

    let forward_target = CallTarget::same_file("forward<Box<i32>, i32>");
    let method_target = CallTarget::same_file("Box<i32>.into_value");
    let forward = ir
        .functions
        .iter()
        .find(|function| function.target == forward_target)
        .expect("expected specialized bounded function");

    assert!(
        forward.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::TailCall { target, .. } if target == &method_target
            )
        }),
        "{forward:?}"
    );
    assert!(
        ir.functions
            .iter()
            .any(|function| function.target == method_target),
        "{ir:?}"
    );
}

#[test]
fn lowers_non_generic_interface_bound_method_call_to_concrete_static_target() {
    let ir = lower_text(
        r#"interface Measure {
    pub method &self.measure(): i32
}

struct Count {
    value: i32
}

impl Measure for Count {
    method &self.measure(): i32 {
        return self.value
    }
}

func read<T: Measure>(value: &T): i32 {
    return value.measure()
}

func main(): i32 {
    let count = Count { value: 42 }
    return read(&count)
}
"#,
    );

    let read_target = CallTarget::same_file("read<Count>");
    let method_target = CallTarget::same_file("Count.measure");
    let read = ir
        .functions
        .iter()
        .find(|function| function.target == read_target)
        .expect("expected specialized bounded function");

    assert!(
        read.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::CallI32 { target, .. } if target == &method_target
            )
        }),
        "{read:?}"
    );
    assert!(
        ir.functions
            .iter()
            .any(|function| function.target == method_target),
        "{ir:?}"
    );
}

#[test]
fn lowers_generic_impl_method_call_with_concrete_receiver() {
    let ir = lower_text(
        r#"struct Box<T> {
    value: T
}

impl<U> Box<U> {
    method self.into_value(): U {
        return self.value
    }
}

func main(): i32 {
    let box = Box<i32> { value: 42 }
    return (move box).into_value()
}
"#,
    );

    let specialized_target = CallTarget::same_file("Box<i32>.into_value");
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");

    assert!(
        main.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::TailCall {
                    target,
                    arguments,
                } if target == &specialized_target && arguments.len() == 1
            )
        }),
        "{main:?}"
    );

    let method = ir
        .functions
        .iter()
        .find(|function| function.target == specialized_target)
        .expect("expected lowered specialized method");

    assert_eq!(method.name, "Box<i32>.into_value");
    assert_eq!(method.return_type, Type::I32);
}

#[test]
fn lowers_generic_impl_method_for_multiple_concrete_receivers() {
    let ir = lower_text(
        r#"struct Box<T> {
    value: T
}

impl<U> Box<U> {
    method self.into_value(): U {
        return self.value
    }
}

func main(): i32 {
    let first_box = Box<i32> { value: 42 }
    let second_box = Box<u8> { value: 7 }
    let first = (move first_box).into_value()
    let second = (move second_box).into_value()
    return first + (second as i32)
}
"#,
    );

    assert!(
        ir.functions.iter().any(|function| function.target
            == CallTarget::same_file("Box<i32>.into_value")
            && function.return_type == Type::I32),
        "{ir:?}"
    );
    assert!(
        ir.functions.iter().any(|function| function.target
            == CallTarget::same_file("Box<u8>.into_value")
            && function.return_type == Type::U8),
        "{ir:?}"
    );
}
