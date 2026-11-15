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
    let leaves = ids[..2]
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
    assert_eq!(program.data_copyable(data[0]), Some(true));
    assert_eq!(program.data_copyable(leaves[0]), Some(false));
}
