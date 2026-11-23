//! Concrete generic signatures in the ordinary body-owned type extension.
use super::{BodyTypeError, BodyTypes, structural_component};
use crate::value_access::FunctionConstraint;
use nocter_model::{AccessCodeId, GenericApplication, GenericValue, TypeSubstitution};

impl BodyTypes {
    /// Project the selected generic declaration through a complete concrete application.
    /// The result is a signature with the original input operations, with no code witness.
    /// # Errors
    /// Rejects incomplete, nonconcrete or mismatched applications and invalid resulting data.
    pub fn apply_callable_signature(
        &mut self,
        code: AccessCodeId,
        application: &GenericApplication,
    ) -> Result<FunctionConstraint, BodyTypeError> {
        let program = &mut self.program;
        let invalid = BodyTypeError::InvalidApplication;
        program
            .validate_generic_application(code, application)
            .map_err(|_| invalid)?;
        let parameters = program.generic_parameter_ids(code).ok_or(invalid)?;
        if parameters.is_empty() {
            return Err(invalid);
        }
        let mut substitution = TypeSubstitution::default();
        let mut types = program.types.transaction();
        for (&parameter, &value) in parameters.iter().zip(application.iter()) {
            let concrete = match value {
                GenericValue::Type(data) => types.is_concrete(data) == Some(true),
                GenericValue::Usize(term) => term.closed_value().is_some(),
            };
            if !concrete {
                return Err(invalid);
            }
            substitution.bind_value(parameter, value);
        }
        let header = program.codes().get(code).ok_or(invalid)?;
        let contract = program.declaration_contract(header).ok_or(invalid)?;
        let contract = contract.try_map_data(|data| {
            let projected = substitution
                .apply_type(&mut types, data)
                .map_err(|_| invalid)?;
            if types.is_concrete(projected) != Some(true) {
                return Err(invalid);
            }
            Ok(projected)
        })?;
        let types = types.commit(&program.types).map_err(|_| invalid)?;
        let previous = std::mem::replace(&mut program.types, types);
        let valid = structural_component(program, contract.result)
            && contract
                .inputs
                .iter()
                .all(|input| structural_component(program, input.data));
        if !valid {
            program.types = previous;
            return Err(invalid);
        }
        Ok(contract)
    }
}
