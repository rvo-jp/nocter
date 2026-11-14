use super::*;
use nocter_model::{ValueClosure, ValueParameter};

pub(super) struct Fixture {
    pub(super) program: Program,
    pub(super) creator: AccessCodeId,
    pub(super) formal: ValueParameter,
    pub(super) captured: FunctionWitness,
    pub(super) nested: FunctionWitness,
    pub(super) holder: FunctionWitness,
    pub(super) branching: FunctionWitness,
    pub(super) inputs: [ValueParameter; 2],
    pub(super) callbacks: [FunctionWitness; 2],
    pub(super) empty: nocter_model::TypeId,
    pub(super) pair: nocter_model::TypeId,
    pub(super) large: nocter_model::TypeId,
    pub(super) mixed: FunctionWitness,
    pub(super) receiver: FunctionWitness,
}
pub(super) fn fixture() -> Fixture {
    fixture_with_depth(0)
}
pub(super) fn fixture_with_depth(depth: usize) -> Fixture {
    let (headers, [creator, _, plain, _, _]) = headers();
    let formal = headers.codes().get(creator).unwrap().parameters()[0].into();
    let data = headers
        .function_data(FunctionWitness {
            origin: ValueFunctionOrigin::Parameter(formal),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    let void = headers.data_type(BuiltinType::Void).unwrap();
    let contract = FunctionConstraint {
        invocation: ValueInvocation::Read,
        inputs: Box::new([]),
        result: void,
    };
    let mut body = headers.body_types(creator).unwrap();
    let captured = body.reserve_closure().unwrap();
    let captured_data = body.closure(captured, data, contract.clone()).unwrap();
    let empty = body.array(captured_data, 0).unwrap();
    let mut pair = body.tuple(&[captured_data, captured_data]).unwrap();
    for _ in 0..depth {
        pair = body.tuple(&[pair, pair]).unwrap();
    }
    let large = body.array(pair, u64::MAX).unwrap();
    let mixed = body.reserve_closure().unwrap();
    body.closure(mixed, pair, contract.clone()).unwrap();
    let holder = body.reserve_closure().unwrap();
    let inputs = [0, 1].map(|position| ValueParameter::Closure {
        owner: holder,
        position,
    });
    let input_data = [0, 1].map(|position| {
        body.closure_parameter(holder, position, contract.clone())
            .unwrap()
    });
    let nested = body.reserve_closure().unwrap();
    body.closure(nested, input_data[0], contract.clone())
        .unwrap();
    body.closure(
        holder,
        void,
        FunctionConstraint {
            inputs: input_data
                .map(|data| FunctionInput {
                    data,
                    operation: ValueInputOperation::Read,
                })
                .into(),
            ..contract.clone()
        },
    )
    .unwrap();
    let companion = body.reserve_closure().unwrap();
    let wrapped = body
        .closure(companion, input_data[1], contract.clone())
        .unwrap();
    let environment = body
        .tuple(&[
            body.program()
                .function_data(FunctionWitness {
                    origin: ValueFunctionOrigin::Closure(nested),
                    invocation: ValueInvocation::Read,
                })
                .unwrap(),
            wrapped,
        ])
        .unwrap();
    let branching = body.reserve_closure().unwrap();
    body.closure(branching, environment, contract.clone())
        .unwrap();
    let second = body.reserve_closure().unwrap();
    body.closure(second, void, contract.clone()).unwrap();
    let receiver = input_receiver(&mut body, pair, empty, &contract);
    let witness = |code: ValueClosure| FunctionWitness {
        origin: ValueFunctionOrigin::Closure(code),
        invocation: ValueInvocation::Read,
    };
    Fixture {
        program: body.finish(),
        creator,
        formal,
        captured: witness(captured),
        nested: witness(nested),
        holder: witness(holder),
        branching: witness(branching),
        inputs,
        callbacks: [plain.into(), witness(second)],
        empty,
        pair,
        large,
        mixed: witness(mixed),
        receiver,
    }
}
fn input_receiver(
    body: &mut crate::value_access::BodyTypes,
    pair: nocter_model::TypeId,
    empty: nocter_model::TypeId,
    contract: &FunctionConstraint,
) -> FunctionWitness {
    let receiver = body.reserve_closure().unwrap();
    let direct = body
        .closure_parameter(receiver, 1, contract.clone())
        .unwrap();
    body.closure(
        receiver,
        contract.result,
        FunctionConstraint {
            inputs: [pair, direct, empty, contract.result]
                .map(|data| FunctionInput {
                    data,
                    operation: ValueInputOperation::Read,
                })
                .into(),
            ..contract.clone()
        },
    )
    .unwrap();
    FunctionWitness {
        origin: ValueFunctionOrigin::Closure(receiver),
        invocation: ValueInvocation::Read,
    }
}
