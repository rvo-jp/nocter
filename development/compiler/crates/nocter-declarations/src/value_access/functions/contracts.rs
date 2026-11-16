//! One semantic contract fold, independent of stored component paths and physical inputs.
use super::{DataReference, FunctionConstraint, intern_views};
use crate::value_access::data::{ReferenceNode, resolve_reference};
use nocter_model::{
    TypeId, TypeTransaction, ValueCallableContract, ValueContractOwner, ValueFunctionOrigin,
};
pub(in crate::value_access) fn resolve(
    types: &mut TypeTransaction,
    owner: ValueContractOwner,
    data: &DataReference,
) -> (TypeId, Vec<(ValueCallableContract, FunctionConstraint)>) {
    let mut contracts = Vec::new();
    let data = resolve_reference(data, |node| match node {
        ReferenceNode::Data(kind) => types.intern(kind).ok(),
        ReferenceNode::Function(contract) => {
            let id = ValueCallableContract {
                owner,
                ordinal: contracts.len(),
            };
            let data = intern_views(
                types,
                ValueFunctionOrigin::Contract(id),
                contract.invocation,
            );
            contracts.push((id, contract));
            Some(data)
        }
    })
    .expect("validated contract data");
    (data, contracts)
}
