use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_source_index::{SemanticEntity, SourceRole};

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{
    CallTarget, CheckedOperation, CoercedReceiverPreparation, ReceiverPreparation, StaticDispatch,
    prepare_program_checking,
};

fn check(source: &str) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let fixture = Fixture::new(source);
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

fn receiver_dispatches(output: &crate::CheckedProgramOutput) -> Vec<(StaticDispatch, usize)> {
    output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Call(call) if call.receiver().is_some() => match call.target() {
                CallTarget::Static(selection) => Some((
                    selection.dispatch(),
                    selection.generic_arguments().as_slice().len(),
                )),
                CallTarget::CallableValue { .. } | CallTarget::ClosureValue { .. } => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn inherent_method_call_freezes_receiver_preparation_and_dispatch() {
    let output = check(
        "struct Value { field: i32 }\n\
         instance Value {\n\
             pub method &self.read(): i32 { self.field }\n\
         }\n\
         func read_owned(input: Value): i32 { input.read() }\n\
         func read_borrowed(input: &Value): i32 { input.read() }\n",
    )
    .unwrap();
    let calls = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Call(call) if call.receiver().is_some() => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[0].receiver().unwrap().preparation(),
        ReceiverPreparation::BorrowPlace(nocter_model::BorrowCapability::Readonly)
    );
    assert_eq!(
        calls[1].receiver().unwrap().preparation(),
        ReceiverPreparation::PreserveBorrow(nocter_model::BorrowCapability::Readonly)
    );
    let CallTarget::Static(selection) = calls[0].target() else {
        panic!("method must retain static dispatch")
    };
    let StaticDispatch::Direct(callable) = selection.dispatch() else {
        panic!("inherent method must retain its declaration identity")
    };
    assert!(
        output
            .source_index()
            .bindings_for(SemanticEntity::Callable(callable))
            .iter()
            .any(|binding| binding.role() == SourceRole::Reference)
    );
}

#[test]
fn owned_method_receiver_consumes_move_only_place_without_move_syntax() {
    let output = check(
        "struct Value { field: i32 }\n\
         instance Value {\n\
             pub method self.take(): i32 { self.field }\n\
         }\n\
         func take(input: Value): i32 { input.take() }\n",
    )
    .unwrap();
    let receiver = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter().map(move |(id, node)| (body, id, node)))
        .find_map(|(body, _, node)| match node.operation() {
            CheckedOperation::Call(call) => call.receiver().map(|receiver| {
                (
                    receiver.preparation(),
                    body.nodes().get(receiver.value()).unwrap().operation(),
                )
            }),
            _ => None,
        })
        .expect("method receiver");

    assert_eq!(receiver.0, ReceiverPreparation::Owned);
    assert!(matches!(receiver.1, CheckedOperation::Move(_)));
}

#[test]
fn instance_and_method_generics_share_identity_keyed_call_arguments() {
    let output = check(
        "struct Box<T> { value: T }\n\
         instance Box<T> {\n\
             pub method &self.echo<U>(value: U): U { move value }\n\
         }\n\
         func echo(input: Box<i32>): bool { input.echo(true) }\n",
    )
    .unwrap();
    let arguments = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .find_map(|(_, node)| match node.operation() {
            CheckedOperation::Call(call) if call.receiver().is_some() => match call.target() {
                CallTarget::Static(selection) => {
                    Some(selection.generic_arguments().as_slice().to_vec())
                }
                CallTarget::CallableValue { .. } | CallTarget::ClosureValue { .. } => None,
            },
            _ => None,
        })
        .expect("generic method call");

    assert_eq!(arguments.len(), 2);
}

#[test]
fn missing_inherent_method_is_a_call_boundary_error() {
    let error = check(
        "struct Value {}\n\
         func invalid(input: Value): i32 { input.missing() }\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0390");
}

#[test]
fn concrete_and_bounded_generic_receivers_select_interface_dispatch() {
    let output = check(
        "pub interface Readable {\n\
             pub method &self.read(): i32\n\
         }\n\
         struct Value {}\n\
         conform Readable for Value {\n\
             method &self.read(): i32 { 42 }\n\
         }\n\
         func concrete(input: &Value): i32 { input.read() }\n\
         func generic<T>(input: &T): i32 where T: Readable { input.read() }\n",
    )
    .unwrap();
    let dispatches = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Call(call) if call.receiver().is_some() => match call.target() {
                CallTarget::Static(selection) => Some(selection.dispatch()),
                CallTarget::CallableValue { .. } | CallTarget::ClosureValue { .. } => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(dispatches.len(), 2);
    assert!(
        dispatches
            .iter()
            .any(|dispatch| matches!(dispatch, StaticDispatch::Direct(_)))
    );
    assert!(
        dispatches
            .iter()
            .any(|dispatch| matches!(dispatch, StaticDispatch::InterfaceMethod { .. }))
    );
}

#[test]
fn conformance_default_method_uses_specialized_interface_contract() {
    let output = check(
        "pub interface DefaultValue<T> {\n\
             pub method &self.value(): i64 { 0 }\n\
         }\n\
         struct Value {}\n\
         conform DefaultValue<T> for Value where T = i64 {}\n\
         func value(input: &Value): i64 { input.value() }\n",
    )
    .unwrap();
    let selection = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .find_map(|(_, node)| match node.operation() {
            CheckedOperation::Call(call) if call.receiver().is_some() => match call.target() {
                CallTarget::Static(selection) => Some(selection),
                CallTarget::CallableValue { .. } | CallTarget::ClosureValue { .. } => None,
            },
            _ => None,
        })
        .expect("default method call");

    assert!(matches!(selection.dispatch(), StaticDispatch::Direct(_)));
    assert_eq!(selection.generic_arguments().as_slice().len(), 1);
}

#[test]
fn associated_method_results_specialize_for_concrete_and_generic_receivers() {
    check(
        "pub interface Source {\n\
             pub type Item\n\
             pub method &self.get(): Self.Item\n\
         }\n\
         struct Buffer {}\n\
         conform Source for Buffer {\n\
             type Item = i32\n\
             method &self.get(): i32 { 0 }\n\
         }\n\
         func concrete(source: &Buffer): i32 { source.get() }\n\
         func generic<S>(source: &S): S.Item where S: Source { source.get() }\n",
    )
    .unwrap();
}

#[test]
fn inherent_and_interface_methods_with_the_same_name_are_ambiguous() {
    let error = check(
        "pub interface Readable { pub method &self.read(): i32 }\n\
         struct Value {}\n\
         instance Value { pub method &self.read(): i32 { 1 } }\n\
         conform Readable for Value { method &self.read(): i32 { 2 } }\n\
         func invalid(input: &Value): i32 { input.read() }\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0390");
}

#[test]
fn newly_created_temporary_can_supply_a_readwrite_receiver() {
    let output = check(
        "struct Counter {}\n\
         construct Counter { pub func new(): Self { loop {} } }\n\
         instance Counter { pub method &+self.reset(): void { return } }\n\
         func reset(): void { Counter.new().reset() }\n",
    )
    .unwrap();
    let preparation = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .find_map(|(_, node)| match node.operation() {
            CheckedOperation::Call(call) => {
                call.receiver().map(crate::CheckedCallReceiver::preparation)
            }
            _ => None,
        })
        .expect("method receiver");

    assert_eq!(
        preparation,
        ReceiverPreparation::BorrowTemporary(nocter_model::BorrowCapability::ReadWrite)
    );
}

#[test]
fn method_lookup_uses_one_selected_receiver_coercion() {
    let output = check(
        "struct Text { value: i32 }\n\
         struct Wrapper { text: Text }\n\
         instance Text { pub method &self.len(): usize { 0 } }\n\
         instance Wrapper {\n\
             pub coerce &self as &Text { &self.text }\n\
         }\n\
         func len(wrapper: &Wrapper): usize { wrapper.len() }\n",
    )
    .unwrap();
    let receiver = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .find_map(|(_, node)| match node.operation() {
            CheckedOperation::Call(call) => call
                .receiver()
                .filter(|receiver| receiver.coercion().is_some()),
            _ => None,
        })
        .expect("coerced method receiver");

    assert_eq!(
        receiver.preparation(),
        ReceiverPreparation::PreserveBorrow(nocter_model::BorrowCapability::Readonly)
    );
    assert_eq!(
        receiver.coercion().unwrap().result_preparation(),
        CoercedReceiverPreparation::PreserveReadonly
    );
}

#[test]
fn readwrite_receiver_coercion_preserves_mutation_authority() {
    let output = check(
        "struct Target { value: i32 }\n\
         struct Wrapper { target: Target }\n\
         instance Target { pub method &+self.clear(): void { self.value = 0 } }\n\
         instance Wrapper {\n\
             pub coerce &+self as &+Target { &+self.target }\n\
         }\n\
         func clear(wrapper: &+Wrapper): void { wrapper.clear() }\n",
    )
    .unwrap();
    let receiver = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .find_map(|(_, node)| match node.operation() {
            CheckedOperation::Call(call) => call
                .receiver()
                .filter(|receiver| receiver.coercion().is_some()),
            _ => None,
        })
        .expect("coerced readwrite receiver");

    assert_eq!(
        receiver.preparation(),
        ReceiverPreparation::PreserveBorrow(nocter_model::BorrowCapability::ReadWrite)
    );
    assert_eq!(
        receiver.coercion().unwrap().result_preparation(),
        CoercedReceiverPreparation::PreserveReadwrite
    );
}

#[test]
fn exact_receiver_method_has_priority_over_coercion_routes() {
    let output = check(
        "struct Text {}\n\
         struct Wrapper { text: Text }\n\
         instance Text { pub method &self.len(): usize { 1 } }\n\
         instance Wrapper {\n\
             pub method &self.len(): usize { 2 }\n\
             pub coerce &self as &Text { &self.text }\n\
         }\n\
         func len(wrapper: &Wrapper): usize { wrapper.len() }\n",
    )
    .unwrap();
    let receiver = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .find_map(|(_, node)| match node.operation() {
            CheckedOperation::Call(call) => call.receiver(),
            _ => None,
        })
        .expect("method receiver");

    assert!(receiver.coercion().is_none());
}

#[test]
fn equally_ranked_receiver_coercion_routes_are_ambiguous() {
    let error = check(
        "struct First {}\n\
         struct Second {}\n\
         struct Wrapper {\n\
             first: First\n\
             second: Second\n\
         }\n\
         instance First { pub method &self.len(): usize { 1 } }\n\
         instance Second { pub method &self.len(): usize { 2 } }\n\
         instance Wrapper {\n\
             pub coerce &self as &First { &self.first }\n\
             pub coerce &self as &Second { &self.second }\n\
         }\n\
         func invalid(wrapper: &Wrapper): usize { wrapper.len() }\n",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0390");
}

#[test]
fn method_selection_is_independent_of_compile_unit_input_order() {
    let fixture = Fixture::new(
        "pub interface Readable { pub method &self.read(): i32 }\n\
         struct Value {}\n\
         conform Readable for Value { method &self.read(): i32 { 1 } }\n\
         func read(input: &Value): i32 { input.read() }\n",
    );
    let mut outputs = Vec::new();
    for reverse in [false, true] {
        let (input, prelude) = fixture.input(reverse);
        let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
        let (program, source_index) = lowered.into_parts();
        let prepared = prepare_program_checking(&input, program, source_index).unwrap();
        outputs.push(check_prepared_program(&input, prepared).unwrap());
    }

    assert_eq!(
        receiver_dispatches(&outputs[0]),
        receiver_dispatches(&outputs[1])
    );
}
