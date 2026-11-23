//! Canonical data projections for selected generic signatures and original body frames.
use super::{BodyTypeError, BodyTypes, Program, TypeScope, structural_component};
use crate::value_access::FunctionConstraint;
use nocter_model::{AccessCodeId, GenericApplication, GenericValue, TypeId, TypeSubstitution};
use std::collections::{BTreeMap, BTreeSet};

type TypeProjection = BTreeMap<TypeId, TypeId>;
pub(in crate::value_access) type GenericFrameTypes =
    BTreeMap<AccessCodeId, BTreeMap<GenericApplication, TypeProjection>>;

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
        let substitution = program.generic_substitution(code, application)?;
        let mut types = program.types.transaction();
        let header = program.codes().get(code).ok_or(invalid)?;
        let contract = program.declaration_contract(header).ok_or(invalid)?;
        let contract = contract.try_map_data(|data| project(&substitution, &mut types, data))?;
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

impl Program {
    fn generic_substitution(
        &self,
        code: AccessCodeId,
        application: &GenericApplication,
    ) -> Result<TypeSubstitution, BodyTypeError> {
        let invalid = BodyTypeError::InvalidApplication;
        self.validate_generic_application(code, application)
            .map_err(|_| invalid)?;
        let parameters = self.generic_parameter_ids(code).ok_or(invalid)?;
        if parameters.is_empty() {
            return Err(invalid);
        }
        let mut substitution = TypeSubstitution::default();
        for (&parameter, &value) in parameters.iter().zip(application.iter()) {
            let concrete = match value {
                GenericValue::Type(data) => self.types.store().is_concrete(data) == Some(true),
                GenericValue::Usize(term) => term.closed_value().is_some(),
            };
            if !concrete {
                return Err(invalid);
            }
            substitution.bind_value(parameter, value);
        }
        Ok(substitution)
    }

    /// Intern the concrete types selected by exact body applications into the canonical unit.
    /// The original symbolic declarations and types remain available for source evidence.
    /// # Errors
    /// Rejects a foreign or completed authority and applications with unresolved data.
    pub fn project_generic_frames(
        &self,
        roots: BTreeMap<(AccessCodeId, GenericApplication), BTreeSet<TypeId>>,
    ) -> Result<Self, BodyTypeError> {
        if self.type_scope != TypeScope::Canonical || !self.generic_frames.is_empty() {
            return Err(BodyTypeError::InvalidExtension);
        }
        let invalid = BodyTypeError::InvalidApplication;
        let mut types = self.types.transaction();
        let mut frames = GenericFrameTypes::new();
        for ((code, application), mut data) in roots {
            let substitution = self.generic_substitution(code, &application)?;
            let header = self.codes().get(code).ok_or(invalid)?;
            let signature = self.declaration_contract(header).ok_or(invalid)?;
            data.extend(signature.inputs.iter().map(|input| input.data));
            data.insert(signature.result);
            let mut projection = TypeProjection::new();
            for source in data {
                let target = project(&substitution, &mut types, source)?;
                if source != target {
                    projection.insert(source, target);
                }
            }
            frames
                .entry(code)
                .or_default()
                .insert(application, projection);
        }
        let mut program = self.clone();
        program.types = types.commit(&self.types).map_err(|_| invalid)?;
        if !frames
            .values()
            .flat_map(BTreeMap::values)
            .flat_map(BTreeMap::values)
            .all(|&data| structural_component(&program, data))
        {
            return Err(invalid);
        }
        program.generic_frames = frames.into();
        Ok(program)
    }
}

fn project(
    substitution: &TypeSubstitution,
    types: &mut nocter_model::TypeTransaction,
    data: TypeId,
) -> Result<TypeId, BodyTypeError> {
    if types.is_concrete(data) == Some(true) {
        return Ok(data);
    }
    let invalid = BodyTypeError::InvalidApplication;
    let projected = substitution.apply_type(types, data).map_err(|_| invalid)?;
    (types.is_concrete(projected) == Some(true))
        .then_some(projected)
        .ok_or(invalid)
}
