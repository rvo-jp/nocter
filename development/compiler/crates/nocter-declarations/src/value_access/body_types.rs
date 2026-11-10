//! Owner-scoped body type construction and canonical replay over one immutable header prefix.
use super::{Program, TypeScope};
use nocter_model::{
    AccessCodeId, AccessExpressionId, TypeCursor, TypeId, TypeKind, ValueApplication,
    ValueFunctionOrigin, ValueInvocation,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyTypeError {
    ForeignDeclarations,
    InvalidApplication,
    InvalidExtension,
}

/// The sole construction capability for one body's closed type extension, with shared header facts.
#[derive(Clone, Debug)]
pub struct BodyTypes {
    owner: AccessCodeId,
    program: Program,
    applications: BTreeMap<AccessExpressionId, AccessCodeId>,
}
impl BodyTypes {
    #[must_use]
    pub const fn owner(&self) -> AccessCodeId {
        self.owner
    }
    /// Intern a structural tuple over admitted closed ordinary component data.
    /// # Errors
    /// Rejects an invalid arity or a component outside this authority's closed data domain.
    pub fn tuple(&mut self, fields: &[TypeId]) -> Result<TypeId, BodyTypeError> {
        let [first, second, remaining @ ..] = fields else {
            return Err(BodyTypeError::InvalidExtension);
        };
        if fields
            .iter()
            .any(|&data| !ordinary_data(&self.program, data))
        {
            return Err(BodyTypeError::InvalidExtension);
        }
        let mut types = self.program.types.transaction();
        let data = types
            .intern(TypeKind::Tuple(nocter_model::TupleElements::new(
                *first,
                *second,
                remaining.iter().copied(),
            )))
            .map_err(|_| BodyTypeError::InvalidExtension)?;
        self.program.types = types
            .commit(&self.program.types)
            .map_err(|_| BodyTypeError::InvalidExtension)?;
        Ok(data)
    }
    /// Intern a homogeneous fixed-array descriptor without expanding its cardinality.
    /// # Errors
    /// Rejects an element outside this authority's closed ordinary data domain.
    pub fn array(&mut self, element: TypeId, length: u64) -> Result<TypeId, BodyTypeError> {
        if !ordinary_data(&self.program, element) {
            return Err(BodyTypeError::InvalidExtension);
        }
        let mut types = self.program.types.transaction();
        let data = types
            .intern(TypeKind::FixedArray {
                element,
                length: length.into(),
            })
            .map_err(|_| BodyTypeError::InvalidExtension)?;
        self.program.types = types
            .commit(&self.program.types)
            .map_err(|_| BodyTypeError::InvalidExtension)?;
        Ok(data)
    }
    /// Publish one closed owned environment; it has no inferred external R/D/S.
    /// # Errors
    /// Rejects a foreign/duplicate code identity or data outside the closed ordinary data domain.
    pub fn closure(
        &mut self,
        id: nocter_model::ValueClosure,
        environment: TypeId,
        contract: super::FunctionConstraint,
    ) -> Result<TypeId, BodyTypeError> {
        let ordinary = |data| ordinary_data(&self.program, data);
        if id.owner != self.owner
            || self.program.closures.contains_key(&id)
            || !ordinary(environment)
            || !ordinary(contract.result)
            || contract.inputs.iter().any(|input| !ordinary(input.data))
        {
            return Err(BodyTypeError::InvalidExtension);
        }
        let declared = contract.invocation;
        let mut types = self.program.types.transaction();
        let mut selected = None;
        for invocation in [
            ValueInvocation::Read,
            ValueInvocation::Edit,
            ValueInvocation::Take,
        ] {
            if declared.can_supply(invocation) {
                let data = types
                    .intern(TypeKind::ValueFunction {
                        origin: ValueFunctionOrigin::Closure(id),
                        invocation,
                    })
                    .map_err(|_| BodyTypeError::InvalidExtension)?;
                if invocation == declared {
                    selected = Some(data);
                }
            }
        }
        self.program.types = types
            .commit(&self.program.types)
            .map_err(|_| BodyTypeError::InvalidExtension)?;
        self.program.closures.insert(
            id,
            super::Closure {
                environment,
                contract,
            },
        );
        selected.ok_or(BodyTypeError::InvalidExtension)
    }
    #[must_use]
    pub const fn program(&self) -> &Program {
        &self.program
    }
    /// Intern all permitted views before a selected call result enters body typing.
    /// # Errors
    /// Rejects a non-factory callee or an inconsistent semantic type extension.
    pub fn application(
        &mut self,
        call: AccessExpressionId,
        callee: AccessCodeId,
    ) -> Result<TypeId, BodyTypeError> {
        if self
            .applications
            .get(&call)
            .is_some_and(|selected| *selected != callee)
        {
            return Err(BodyTypeError::InvalidApplication);
        }
        let declared = self
            .program
            .codes
            .get(callee)
            .and_then(super::Code::result_contract)
            .ok_or(BodyTypeError::InvalidApplication)?
            .invocation;
        let origin = ValueFunctionOrigin::Application(ValueApplication {
            owner: self.owner,
            call,
            callee,
        });
        let mut types = self.program.types.transaction();
        let mut selected = None;
        for invocation in [
            ValueInvocation::Read,
            ValueInvocation::Edit,
            ValueInvocation::Take,
        ] {
            if declared.can_supply(invocation) {
                let data = types
                    .intern(TypeKind::ValueFunction { origin, invocation })
                    .map_err(|_| BodyTypeError::InvalidExtension)?;
                if invocation == declared {
                    selected = Some(data);
                }
            }
        }
        self.program.types = types
            .commit(&self.program.types)
            .map_err(|_| BodyTypeError::InvalidExtension)?;
        self.applications.insert(call, callee);
        selected.ok_or(BodyTypeError::InvalidApplication)
    }
    #[must_use]
    pub fn finish(self) -> Program {
        self.program
    }
}

/// Canonical type authority and exact per-body suffix projections; no public parts constructor.
#[derive(Debug)]
pub struct CanonicalBodyTypes {
    program: Program,
    projections: BTreeMap<AccessCodeId, (TypeCursor, BTreeMap<TypeId, TypeId>)>,
}
impl CanonicalBodyTypes {
    #[must_use]
    pub fn project(&self, owner: AccessCodeId, data: TypeId) -> Option<TypeId> {
        let (prefix, projection) = self.projections.get(&owner)?;
        project_type(*prefix, projection, data)
    }
    #[must_use]
    pub fn finish(self) -> Program {
        self.program
    }
}
impl Program {
    /// Open a body-owned construction branch without changing declarations or their source cache.
    /// # Errors
    /// Rejects an identity outside this declaration program.
    pub fn body_types(&self, owner: AccessCodeId) -> Result<BodyTypes, BodyTypeError> {
        if self.type_scope != TypeScope::Headers {
            return Err(BodyTypeError::InvalidExtension);
        }
        self.codes
            .get(owner)
            .ok_or(BodyTypeError::InvalidApplication)?;
        let mut program = self.clone();
        program.type_scope = TypeScope::Body(owner);
        Ok(BodyTypes {
            owner,
            program,
            applications: BTreeMap::new(),
        })
    }
    /// Replay body-owned witnesses and structural shapes into one final authority in owner order.
    /// # Errors
    /// Rejects foreign headers, an unrelated type prefix or a suffix outside its body's domain.
    pub fn canonical_body_types(
        &self,
        bodies: BTreeMap<AccessCodeId, &Program>,
    ) -> Result<CanonicalBodyTypes, BodyTypeError> {
        if self.type_scope != TypeScope::Headers {
            return Err(BodyTypeError::InvalidExtension);
        }
        let mut types = self.types.transaction();
        let mut projections = BTreeMap::new();
        let mut closures = BTreeMap::new();
        for (owner, body) in bodies {
            if body.type_scope != TypeScope::Body(owner)
                || !self.preserves_headers(body)
                || !body.types.store().preserves_prefix(&body.header_prefix)
            {
                return Err(BodyTypeError::ForeignDeclarations);
            }
            self.codes
                .get(owner)
                .ok_or(BodyTypeError::InvalidApplication)?;
            let prefix = body.header_prefix.cursor();
            let projection = replay_types(self, body, owner, &mut types)?;
            for (&id, schema) in &body.closures {
                if id.owner != owner {
                    return Err(BodyTypeError::InvalidExtension);
                }
                let project = |data| {
                    project_type(prefix, &projection, data).ok_or(BodyTypeError::InvalidExtension)
                };
                let mut schema = schema.clone();
                schema.environment = project(schema.environment)?;
                schema.contract.result = project(schema.contract.result)?;
                for input in &mut schema.contract.inputs {
                    input.data = project(input.data)?;
                }
                if closures.insert(id, schema).is_some() {
                    return Err(BodyTypeError::InvalidExtension);
                }
            }
            projections.insert(owner, (prefix, projection));
        }
        let mut program = self.clone();
        program.type_scope = TypeScope::Canonical;
        program.closures = closures;
        program.types = types
            .commit(&self.types)
            .map_err(|_| BodyTypeError::InvalidExtension)?;
        Ok(CanonicalBodyTypes {
            program,
            projections,
        })
    }
}

fn project_type(
    prefix: TypeCursor,
    projection: &BTreeMap<TypeId, TypeId>,
    data: TypeId,
) -> Option<TypeId> {
    if prefix.suffix_offset(data).is_none() {
        Some(data)
    } else {
        projection.get(&data).copied()
    }
}

fn ordinary_data(program: &Program, data: TypeId) -> bool {
    matches!(
        program.data_schema(data),
        Some(
            super::PayloadSchema::Builtin(_)
                | super::PayloadSchema::Aggregate(_)
                | super::PayloadSchema::Array(_)
        )
    )
}

fn replay_types(
    header: &Program,
    body: &Program,
    owner: AccessCodeId,
    types: &mut nocter_model::TypeTransaction,
) -> Result<BTreeMap<TypeId, TypeId>, BodyTypeError> {
    let prefix = &body.header_prefix;
    let mut projection = BTreeMap::new();
    for (source, kind) in body.types.store().iter_from(prefix.type_count()) {
        let valid = match kind {
            TypeKind::ValueFunction {
                origin: ValueFunctionOrigin::Closure(id),
                invocation,
            } => {
                id.owner == owner
                    && body
                        .closure(*id)
                        .is_some_and(|c| c.contract().invocation.can_supply(*invocation))
            }
            TypeKind::ValueFunction {
                origin: ValueFunctionOrigin::Application(application),
                invocation,
            } => {
                application.owner == owner
                    && header
                        .codes
                        .get(application.callee)
                        .and_then(super::Code::result_contract)
                        .is_some_and(|contract| contract.invocation.can_supply(*invocation))
            }
            TypeKind::Tuple(fields) => fields.iter().all(|data| ordinary_data(body, data)),
            TypeKind::FixedArray { element, length } => {
                length.closed_value().is_some() && ordinary_data(body, *element)
            }
            _ => false,
        };
        if !valid {
            return Err(BodyTypeError::InvalidApplication);
        }
        let project = |data| {
            project_type(prefix.cursor(), &projection, data).ok_or(BodyTypeError::InvalidExtension)
        };
        let kind = match kind {
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
            _ => kind.clone(),
        };
        let target = types
            .intern(kind)
            .map_err(|_| BodyTypeError::InvalidExtension)?;
        projection.insert(source, target);
    }
    Ok(projection)
}

#[cfg(test)]
mod tests;
