//! Resolve callable leaves and replay containing shapes in the same canonical type authority.
use super::{BodyTypeError, Program};
use crate::value_access::{FunctionWitness, TypeScope};
use nocter_model::{TypeId, TypeKind, ValueFunctionOrigin};
use std::collections::BTreeMap;

/// A transient identity projection, consumed before executable body admission.
#[derive(Debug)]
pub struct ResolvedBodyTypes {
    program: Program,
    projection: BTreeMap<TypeId, TypeId>,
}
impl ResolvedBodyTypes {
    #[must_use]
    pub fn project(&self, data: TypeId) -> Option<TypeId> {
        self.projection
            .get(&data)
            .copied()
            .or_else(|| self.program.types.store().get(data).map(|_| data))
    }
    #[must_use]
    pub const fn program(&self) -> &Program {
        &self.program
    }
    #[must_use]
    pub fn finish(self) -> Program {
        self.program
    }
}
impl Program {
    /// Normalize closed result identities and containing body shapes without editing headers.
    /// # Errors
    /// Rejects a noncanonical authority or a substitution that changes the selected contract/view.
    pub fn resolve_body_types(
        &self,
        witnesses: &BTreeMap<FunctionWitness, FunctionWitness>,
    ) -> Result<ResolvedBodyTypes, BodyTypeError> {
        if self.type_scope != TypeScope::Canonical {
            return Err(BodyTypeError::InvalidExtension);
        }
        self.validate_body_witnesses(witnesses)?;
        let mut types = self.types.transaction();
        let mut projection = BTreeMap::new();
        for (data, kind) in self.types.store().iter_from(0) {
            let project = |data| {
                projection
                    .get(&data)
                    .copied()
                    .ok_or(BodyTypeError::InvalidExtension)
            };
            let rebuilt = match kind {
                TypeKind::ValueFunction { origin, invocation } => {
                    let witness = FunctionWitness {
                        origin: *origin,
                        invocation: *invocation,
                    };
                    let target = match witnesses.get(&witness) {
                        Some(&selected) => self
                            .function_data(selected)
                            .ok_or(BodyTypeError::InvalidExtension)?,
                        None => data,
                    };
                    projection.insert(data, target);
                    continue;
                }
                TypeKind::Optional(payload) => TypeKind::Optional(project(*payload)?),
                TypeKind::Future(output) => TypeKind::Future(project(*output)?),
                TypeKind::Fallible(payload) => TypeKind::Fallible(project(*payload)?),
                TypeKind::Tuple(fields) => {
                    let fields = fields.iter().map(project).collect::<Result<Vec<_>, _>>()?;
                    TypeKind::Tuple(nocter_model::TupleElements::new(
                        fields[0],
                        fields[1],
                        fields[2..].iter().copied(),
                    ))
                }
                TypeKind::FixedArray { element, length } => TypeKind::FixedArray {
                    element: project(*element)?,
                    length: *length,
                },
                _ => {
                    projection.insert(data, data);
                    continue;
                }
            };
            let target = types
                .intern(rebuilt)
                .map_err(|_| BodyTypeError::InvalidExtension)?;
            projection.insert(data, target);
        }
        let project = |data| {
            projection
                .get(&data)
                .copied()
                .ok_or(BodyTypeError::InvalidExtension)
        };
        let mut program = self.clone();
        for schema in program.closures.values_mut() {
            schema.environment = project(schema.environment)?;
            schema.contract = schema.contract.try_map_data(project)?;
        }
        for contract in program
            .closure_parameters
            .values_mut()
            .chain(program.callable_contracts.values_mut())
        {
            *contract = contract.try_map_data(project)?;
        }
        program.types = types
            .commit(&self.types)
            .map_err(|_| BodyTypeError::InvalidExtension)?;
        super::containment::validate(&program)?;
        // Nominal metadata is a header fact. Body-dependent structure is never written into it.
        debug_assert!(program.preserves_headers(self));
        Ok(ResolvedBodyTypes {
            program,
            projection,
        })
    }

    fn validate_body_witnesses(
        &self,
        witnesses: &BTreeMap<FunctionWitness, FunctionWitness>,
    ) -> Result<(), BodyTypeError> {
        for (&source, &target) in witnesses {
            if !matches!(
                source.origin,
                ValueFunctionOrigin::Result(_) | ValueFunctionOrigin::Application(_)
            ) || !matches!(
                target.origin,
                ValueFunctionOrigin::Declaration(_)
                    | ValueFunctionOrigin::Closure(_)
                    | ValueFunctionOrigin::Parameter(_)
            ) || source.invocation != target.invocation
                || self.function_data(source).is_none()
            {
                return Err(BodyTypeError::InvalidExtension);
            }
            let expected = self
                .function_contract(source)
                .ok_or(BodyTypeError::InvalidExtension)?;
            let supplied = self
                .function_data(target)
                .ok_or(BodyTypeError::InvalidExtension)?;
            self.constrain_declared_function(supplied, &expected)
                .map_err(|_| BodyTypeError::InvalidExtension)?;
        }
        Ok(())
    }
}
