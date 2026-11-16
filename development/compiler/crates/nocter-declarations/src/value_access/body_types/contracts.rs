//! Callable type contracts have no formal input, source producer or stored environment.
use super::{BodyTypeError, BodyTypes};
use crate::value_access::{FunctionConstraint, functions};
use nocter_model::{TypeId, ValueFunctionOrigin};
impl BodyTypes {
    /// Publish a callable type contract without inventing an input, producer or stored value.
    /// Child contracts must already be valid in this type extension; ordinals are scope-local.
    /// # Errors
    /// Rejects duplicate identities and unknown or unsupported signature data atomically.
    pub fn callable_contract(
        &mut self,
        ordinal: usize,
        contract: FunctionConstraint,
    ) -> Result<TypeId, BodyTypeError> {
        let id = nocter_model::ValueCallableContract {
            owner: nocter_model::ValueContractOwner::Body(self.owner),
            ordinal,
        };
        if self.program.callable_contracts.contains_key(&id) {
            return Err(BodyTypeError::InvalidExtension);
        }
        if !super::structural_component(&self.program, contract.result)
            || contract
                .inputs
                .iter()
                .any(|input| !super::structural_component(&self.program, input.data))
        {
            return Err(BodyTypeError::UnsupportedCallableSignature);
        }
        let mut types = self.program.types.transaction();
        let data = functions::intern_views(
            &mut types,
            ValueFunctionOrigin::Contract(id),
            contract.invocation,
        );
        self.program.types = types
            .commit(&self.program.types)
            .map_err(|_| BodyTypeError::InvalidExtension)?;
        self.program.callable_contracts.insert(id, contract);
        Ok(data)
    }
}
