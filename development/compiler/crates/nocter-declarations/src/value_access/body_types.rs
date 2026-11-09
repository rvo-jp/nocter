//! Owner-scoped application type construction and canonical replay over one immutable header prefix.
use super::Program;
use nocter_model::{
    AccessCallableId, AccessExpressionId, TypeCursor, TypeId, TypeKind, ValueApplication,
    ValueFunctionOrigin, ValueInvocation,
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyTypeError {
    ForeignDeclarations,
    InvalidApplication,
    InvalidExtension,
}

/// The sole construction capability for one body's application types, with shared header facts.
#[derive(Clone, Debug)]
pub struct BodyTypes {
    owner: AccessCallableId,
    program: Program,
    applications: BTreeMap<AccessExpressionId, AccessCallableId>,
}
impl BodyTypes {
    #[must_use]
    pub const fn owner(&self) -> AccessCallableId {
        self.owner
    }
    /// Publish one closed owned environment; it has no inferred external R/D/S.
    /// # Errors
    /// Rejects a foreign/duplicate code identity or data outside the closed header domain.
    pub fn closure(
        &mut self,
        id: nocter_model::ValueClosure,
        environment: TypeId,
        contract: super::FunctionConstraint,
    ) -> Result<TypeId, BodyTypeError> {
        let ordinary = |data| {
            matches!(
                self.program.data_schema(data),
                Some(super::PayloadSchema::Builtin(_) | super::PayloadSchema::Record(_))
            )
        };
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
        callee: AccessCallableId,
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
            .callables
            .get(callee)
            .and_then(super::Callable::result_contract)
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
    prefix: TypeCursor,
    projections: BTreeMap<AccessCallableId, BTreeMap<TypeId, TypeId>>,
}
impl CanonicalBodyTypes {
    #[must_use]
    pub fn project(&self, owner: AccessCallableId, data: TypeId) -> Option<TypeId> {
        let projection = self.projections.get(&owner)?;
        if self.prefix.suffix_offset(data).is_none() {
            Some(data)
        } else {
            projection.get(&data).copied()
        }
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
    pub fn body_types(&self, owner: AccessCallableId) -> Result<BodyTypes, BodyTypeError> {
        if self.types.store().type_count() != self.header_type_count {
            return Err(BodyTypeError::InvalidExtension);
        }
        self.callables
            .get(owner)
            .ok_or(BodyTypeError::InvalidApplication)?;
        Ok(BodyTypes {
            owner,
            program: self.clone(),
            applications: BTreeMap::new(),
        })
    }
    /// Replay body-owned leaf keys in semantic owner order into a single final authority.
    /// # Errors
    /// Rejects foreign headers, an unrelated type prefix or a suffix outside its body's domain.
    pub fn canonical_body_types(
        &self,
        bodies: BTreeMap<AccessCallableId, &Program>,
    ) -> Result<CanonicalBodyTypes, BodyTypeError> {
        if self.types.store().type_count() != self.header_type_count {
            return Err(BodyTypeError::InvalidExtension);
        }
        let prefix = self.types.store();
        let mut types = self.types.transaction();
        let mut projections = BTreeMap::new();
        let mut closures = BTreeMap::new();
        for (owner, body) in bodies {
            if !Arc::ptr_eq(&self.callables, &body.callables)
                || !body.types.store().preserves_prefix(prefix)
            {
                return Err(BodyTypeError::ForeignDeclarations);
            }
            self.callables
                .get(owner)
                .ok_or(BodyTypeError::InvalidApplication)?;
            let mut projection = BTreeMap::new();
            for (&id, schema) in &body.closures {
                if id.owner != owner || closures.insert(id, schema.clone()).is_some() {
                    return Err(BodyTypeError::InvalidExtension);
                }
            }
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
                            && self
                                .callables
                                .get(application.callee)
                                .and_then(super::Callable::result_contract)
                                .is_some_and(|contract| contract.invocation.can_supply(*invocation))
                    }
                    _ => false,
                };
                if !valid {
                    return Err(BodyTypeError::InvalidApplication);
                }
                let target = types
                    .intern(kind.clone())
                    .map_err(|_| BodyTypeError::InvalidExtension)?;
                projection.insert(source, target);
            }
            projections.insert(owner, projection);
        }
        let mut program = self.clone();
        program.closures = closures;
        program.types = types
            .commit(&self.types)
            .map_err(|_| BodyTypeError::InvalidExtension)?;
        Ok(CanonicalBodyTypes {
            program,
            prefix: prefix.cursor(),
            projections,
        })
    }
}
