//! Callable constraints under empty arrays never establish positive witness equivalence.
use super::*;
use crate::value_access::FunctionDefinition;
use nocter_model::{ValueCallableResult, ValueFunctionOrigin, ValueInvocation};

#[test]
fn empty_descriptor_matching_checks_signatures_and_preserves_inhabited_identity() {
    let symbols = SymbolTable::from_spellings(["a", "b", "wrong", "positive_a", "positive_b"]);
    let mut reservations = Reservations::new(symbols.clone());
    let ids = ["a", "b", "wrong", "positive_a", "positive_b"]
        .map(|name| reservations.reserve(symbols.get(name).unwrap(), 0));
    let mut builder = reservations.finish().unwrap();
    for (index, id) in ids.into_iter().enumerate() {
        let result = if index == 2 {
            BuiltinType::Bool
        } else {
            BuiltinType::I32
        };
        let element = DataReference::Function(Box::new(FunctionDefinition {
            invocation: ValueInvocation::Read,
            inputs: Box::new([]),
            result: result.into(),
        }));
        builder
            .define(
                id,
                CodeKind::Function(CompileTimeGuarantee::Evaluatable),
                [],
                array(element, u64::from(index > 2)),
            )
            .unwrap();
    }
    let program = builder.finish().unwrap();
    let data = ids.map(|id| program.codes().get(id).unwrap().result());
    assert!(program.matches_uninhabited_metadata(data[0], data[1]));
    assert!(program.matches_uninhabited_metadata(data[1], data[0]));
    for other in &data[2..] {
        assert!(!program.matches_uninhabited_metadata(data[0], *other));
    }
    assert!(!program.matches_uninhabited_metadata(data[3], data[4]));
    let leaves = ids[..3]
        .iter()
        .map(|&owner| {
            program
                .function_data(crate::value_access::FunctionWitness {
                    origin: ValueFunctionOrigin::Result(ValueCallableResult { owner, ordinal: 0 }),
                    invocation: ValueInvocation::Read,
                })
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(!program.matches_uninhabited_metadata(leaves[0], leaves[1]));
    let empty = |witness: crate::value_access::FunctionWitness| matches!(witness.origin, ValueFunctionOrigin::Result(result) if ids[..3].contains(&result.owner));
    assert!(program.matches_uninhabited_metadata_with(leaves[0], leaves[1], empty));
    assert!(!program.matches_uninhabited_metadata_with(leaves[0], leaves[2], empty));
    assert!(
        !program.matches_uninhabited_metadata_with(leaves[0], leaves[1], |witness| {
            matches!(witness.origin, ValueFunctionOrigin::Result(result) if result.owner == ids[0])
        })
    );
    let keys = leaves
        .iter()
        .map(|&leaf| program.instantiate_uninhabited_data(leaf).unwrap())
        .collect::<Vec<_>>();
    assert!(keys[0].has_same_representation(&program, &keys[1]));
    assert!(!keys[0].has_same_representation(&program, &keys[2]));
    assert!(!program.matches_guarded_transport_metadata(leaves[0], leaves[1], empty));
    let mut body = program.body_types(ids[0]).unwrap();
    let outcomes = leaves
        .iter()
        .map(|&leaf| body.optional(leaf).unwrap())
        .collect::<Vec<_>>();
    let program = body.finish();
    let source_empty = |witness: crate::value_access::FunctionWitness| matches!(witness.origin, ValueFunctionOrigin::Result(result) if result.owner == ids[0]);
    assert!(program.matches_guarded_transport_metadata(outcomes[0], outcomes[1], source_empty));
    assert!(!program.matches_guarded_transport_metadata(outcomes[1], outcomes[0], source_empty));
    assert!(!program.matches_guarded_transport_metadata(outcomes[0], outcomes[2], source_empty));
    assert!(!program.matches_guarded_transport_metadata(outcomes[0], outcomes[1], |_| false));
    assert!(!program.matches_guarded_transport_metadata(leaves[0], leaves[1], source_empty));
    assert_eq!(program.data_copyable(data[0]), Some(true));
    assert_eq!(program.data_copyable(leaves[0]), Some(false));
}
