use super::*;
use crate::value_access::{
    CodeKind, FunctionDefinition, FunctionWitness, ParameterDefinition, Reservations,
    ValueDefinition,
};
use nocter_model::{
    BuiltinType, CompileTimeGuarantee, SymbolTable, ValueFunctionOrigin, ValueInputOperation,
    ValueInvocation,
};

#[test]
fn structural_annotation_constraints_preserve_both_code_and_formal_data_identities_without_interning()
 {
    let symbols = SymbolTable::from_spellings(["left", "right", "action"]);
    let mut slots = Reservations::new(symbols.clone());
    let left = slots.reserve(symbols.get("left").unwrap(), 1);
    let right = slots.reserve(symbols.get("right").unwrap(), 1);
    let mut builder = slots.finish().unwrap();
    for id in [left, right] {
        builder
            .define(
                id,
                CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
                [ParameterDefinition {
                    name: symbols.get("action").unwrap(),
                    operation: ValueInputOperation::Read,
                    data: ValueDefinition::Function(FunctionDefinition {
                        invocation: ValueInvocation::Read,
                        inputs: Box::new([(ValueInputOperation::Read, BuiltinType::I32.into())]),
                        result: BuiltinType::I32.into(),
                    }),
                }],
                BuiltinType::I32,
            )
            .unwrap();
    }
    let program = builder.finish().unwrap();
    let witness = |id| FunctionWitness {
        origin: ValueFunctionOrigin::Declaration(id),
        invocation: ValueInvocation::Read,
    };
    let data = |id| program.function_data(witness(id)).unwrap();
    let left_contract = program.function_contract(witness(left)).unwrap();
    let exact = FunctionPattern::exact(left_contract.clone());
    assert_eq!(
        program.constrain_function_pattern(data(left), &exact),
        Ok(data(left))
    );
    assert!(matches!(
        program.constrain_function_pattern(data(right), &exact),
        Err(FunctionPatternMismatch {
            mismatch: FunctionMismatch::Input { position: 0, .. },
            ..
        })
    ));
    let mut structural = exact;
    let Some(PayloadSchema::Function(formal)) = program.data_schema(left_contract.inputs[0].data)
    else {
        panic!("formal callback");
    };
    structural.inputs[0].data = FunctionDataPattern::Function(Box::new(FunctionPattern::exact(
        program.function_contract(formal).unwrap(),
    )));
    let before = program.types.store().clone();
    for id in [left, right] {
        assert_eq!(
            program.constrain_function_pattern(data(id), &structural),
            Ok(data(id))
        );
        let contract = program.function_contract(witness(id)).unwrap();
        assert_eq!(
            contract.inputs[0].data,
            program
                .parameters()
                .get(program.codes().get(id).unwrap().parameters()[0])
                .unwrap()
                .data()
        );
    }
    assert_ne!(data(left), data(right));
    assert_eq!(
        program.types.store().iter().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    structural.inputs[0].data = FunctionDataPattern::Function(Box::new(FunctionPattern {
        invocation: ValueInvocation::Read,
        inputs: Box::new([]),
        result: FunctionDataPattern::Data(program.data_type(BuiltinType::Bool).unwrap()),
    }));
    let failure = program
        .constrain_function_pattern(data(left), &structural)
        .unwrap_err();
    assert_eq!(failure.path.as_ref(), [FunctionPosition::Input(0)]);
    assert_eq!(
        failure.mismatch,
        FunctionMismatch::InputCount {
            actual: 1,
            expected: 0
        }
    );
}
