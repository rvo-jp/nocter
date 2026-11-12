//! Owner-scoped body type construction and canonical replay over one immutable header prefix.
use super::{Program, TypeScope};
use nocter_model::{
    AccessCodeId, AccessExpressionId, TypeCursor, TypeId, TypeKind, ValueApplication, ValueCode,
    ValueCodeRoot, ValueFunctionOrigin, ValueInvocation,
};
use std::collections::BTreeMap;
mod containment;
mod resolution;
pub use resolution::ResolvedBodyTypes;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyTypeError {
    ForeignDeclarations,
    InvalidApplication,
    InvalidExtension,
    RepeatedFailure,
    RecursiveEnvironment(nocter_model::ValueClosure),
}

/// Descriptor readiness is distinct from payload evidence and Copy eligibility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadReadiness {
    Closed,
    Pending,
    Unsupported,
}

/// The sole construction capability for one body's closed type extension, with shared header facts.
#[derive(Clone, Debug)]
pub struct BodyTypes {
    owner: ValueCodeRoot,
    program: Program,
    applications: BTreeMap<(ValueCode, AccessExpressionId), AccessCodeId>,
    next_closure: Option<u32>,
}
impl BodyTypes {
    #[must_use]
    pub const fn owner(&self) -> ValueCodeRoot {
        self.owner
    }
    /// Reserve one lexical code slot before checking its body, including nested producers.
    /// # Errors
    /// Rejects exhaustion of the existing closure-key domain.
    pub fn reserve_closure(&mut self) -> Result<nocter_model::ValueClosure, BodyTypeError> {
        let slot = self.next_closure.ok_or(BodyTypeError::InvalidExtension)?;
        self.next_closure = slot.checked_add(1);
        Ok(nocter_model::ValueClosure {
            owner: self.owner,
            slot,
        })
    }
    /// Intern a structural tuple over admitted component descriptors, possibly awaiting results.
    /// # Errors
    /// Rejects an invalid arity or a component outside this authority's closed data domain.
    pub fn tuple(&mut self, fields: &[TypeId]) -> Result<TypeId, BodyTypeError> {
        let [first, second, remaining @ ..] = fields else {
            return Err(BodyTypeError::InvalidExtension);
        };
        if fields
            .iter()
            .any(|&data| !structural_component(&self.program, data))
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
    /// Rejects an element outside this authority's structural descriptor domain.
    pub fn array(&mut self, element: TypeId, length: u64) -> Result<TypeId, BodyTypeError> {
        if !structural_component(&self.program, element) {
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
    /// Intern an optional descriptor with its exact positive payload dependency.
    /// # Errors
    /// Rejects a payload outside this authority's structural descriptor domain.
    pub fn optional(&mut self, payload: TypeId) -> Result<TypeId, BodyTypeError> {
        self.outcome(payload, TypeKind::Optional)
    }
    /// Intern a fallible descriptor; failure always stores the one built-in error payload.
    /// # Errors
    /// Rejects an unsupported payload or another failure on the uninterrupted outcome chain.
    pub fn fallible(&mut self, payload: TypeId) -> Result<TypeId, BodyTypeError> {
        if self.program.has_failure_layer(payload) != Some(false) {
            return Err(BodyTypeError::RepeatedFailure);
        }
        self.outcome(payload, TypeKind::Fallible)
    }
    fn outcome(
        &mut self,
        payload: TypeId,
        kind: fn(TypeId) -> TypeKind,
    ) -> Result<TypeId, BodyTypeError> {
        if !structural_component(&self.program, payload) {
            return Err(BodyTypeError::InvalidExtension);
        }
        let mut types = self.program.types.transaction();
        let data = types
            .intern(kind(payload))
            .map_err(|_| BodyTypeError::InvalidExtension)?;
        self.program.types = types
            .commit(&self.program.types)
            .map_err(|_| BodyTypeError::InvalidExtension)?;
        Ok(data)
    }
    /// Publish an owned environment descriptor; result identities may still need closure.
    /// It has no inferred external R/D/S and grants no pending payload or Copy evidence.
    /// # Errors
    /// Rejects a foreign/duplicate code identity, unsupported environment or input/result descriptor.
    pub fn closure(
        &mut self,
        id: nocter_model::ValueClosure,
        environment: TypeId,
        contract: super::FunctionConstraint,
    ) -> Result<TypeId, BodyTypeError> {
        let ordinary = |data| ordinary_data(&self.program, data);
        if id.owner != self.owner
            || self.program.closures.contains_key(&id)
            || self.program.payload_readiness(environment) == PayloadReadiness::Unsupported
            || !(ordinary(contract.result)
                || matches!(
                    self.program.data_schema(contract.result),
                    Some(super::PayloadSchema::Function(_))
                ))
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
        if self.next_closure.is_some_and(|next| id.slot >= next) {
            self.next_closure = id.slot.checked_add(1);
        }
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
        owner: ValueCode,
        call: AccessExpressionId,
        callee: AccessCodeId,
    ) -> Result<TypeId, BodyTypeError> {
        if owner.root() != self.owner
            || self
                .applications
                .get(&(owner, call))
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
            owner,
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
        self.applications.insert((owner, call), callee);
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
    projections: BTreeMap<ValueCodeRoot, (TypeCursor, BTreeMap<TypeId, TypeId>)>,
}
impl CanonicalBodyTypes {
    #[must_use]
    pub fn project(&self, owner: impl Into<ValueCodeRoot>, data: TypeId) -> Option<TypeId> {
        let owner = owner.into();
        let (prefix, projection) = self.projections.get(&owner)?;
        project_type(*prefix, projection, data)
    }
    #[must_use]
    pub fn finish(self) -> Program {
        self.program
    }
}
impl Program {
    /// Closed payloads admitted by owned environment and structural value construction.
    /// A structural formal/result witness alone supplies no environment payload evidence.
    #[must_use]
    pub fn owned_payload_supported(&self, data: TypeId) -> bool {
        self.payload_readiness(data) == PayloadReadiness::Closed
    }
    /// Inspect each represented shape once; a pending descriptor grants no storage or Copy proof.
    #[must_use]
    pub fn payload_readiness(&self, data: TypeId) -> PayloadReadiness {
        use super::PayloadSchema;
        let mut pending = vec![data];
        let mut seen = std::collections::BTreeSet::new();
        let mut readiness = PayloadReadiness::Closed;
        while let Some(data) = pending.pop() {
            if !seen.insert(data) {
                continue;
            }
            match self.data_schema(data) {
                Some(PayloadSchema::Builtin(_)) => {}
                Some(PayloadSchema::Aggregate(schema)) => {
                    pending.extend(schema.components().map(|component| component.data));
                }
                Some(PayloadSchema::Optional(payload) | PayloadSchema::Fallible(payload)) => {
                    pending.push(payload);
                }
                Some(PayloadSchema::Array(array)) => {
                    if !array.is_empty() {
                        pending.push(array.element());
                    }
                }
                Some(PayloadSchema::Function(witness)) => match witness.origin {
                    ValueFunctionOrigin::Declaration(_) => {}
                    ValueFunctionOrigin::Closure(id) => {
                        let Some(schema) = self.closure(id) else {
                            return PayloadReadiness::Unsupported;
                        };
                        pending.push(schema.environment());
                    }
                    ValueFunctionOrigin::Result(_) | ValueFunctionOrigin::Application(_) => {
                        readiness = PayloadReadiness::Pending;
                    }
                    ValueFunctionOrigin::Parameter(_) => return PayloadReadiness::Unsupported,
                },
                None => return PayloadReadiness::Unsupported,
            }
        }
        readiness
    }
    pub(super) fn has_type_root(&self, root: ValueCodeRoot) -> bool {
        match root {
            ValueCodeRoot::Declaration(id) => self.codes.get(id).is_some(),
            ValueCodeRoot::Expression(id) => self.codes.is_reserved(id.owner),
        }
    }
    /// Open a body-owned construction branch without changing declarations or their source cache.
    /// # Errors
    /// Rejects an identity outside this declaration program.
    pub fn body_types(&self, owner: impl Into<ValueCodeRoot>) -> Result<BodyTypes, BodyTypeError> {
        let owner = owner.into();
        if self.type_scope != TypeScope::Headers {
            return Err(BodyTypeError::InvalidExtension);
        }
        if !self.has_type_root(owner) {
            return Err(BodyTypeError::InvalidApplication);
        }
        let mut program = self.clone();
        program.type_scope = TypeScope::Body(owner);
        Ok(BodyTypes {
            owner,
            program,
            applications: BTreeMap::new(),
            next_closure: Some(0),
        })
    }
    /// Replay body-owned witnesses and structural shapes into one final authority in owner order.
    /// # Errors
    /// Rejects foreign headers, an unrelated type prefix or a suffix outside its body's domain.
    pub fn canonical_body_types(
        &self,
        bodies: BTreeMap<ValueCodeRoot, &Program>,
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
            if !self.has_type_root(owner) {
                return Err(BodyTypeError::InvalidApplication);
            }
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

// A descriptor can retain an unready nominal identity under an empty containment edge.
// Constructing that descriptor grants no payload or Copy evidence for its components.
fn structural_component(program: &Program, data: TypeId) -> bool {
    let mut pending = vec![data];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(data) = pending.pop() {
        if !seen.insert(data) || program.record_identity(data).is_some() {
            continue;
        }
        match program.data_schema(data) {
            Some(super::PayloadSchema::Builtin(_)) => {}
            Some(super::PayloadSchema::Aggregate(schema)) => {
                pending.extend(schema.components().map(|component| component.data));
            }
            Some(super::PayloadSchema::Array(array)) => pending.push(array.element()),
            Some(
                super::PayloadSchema::Optional(payload) | super::PayloadSchema::Fallible(payload),
            ) => pending.push(payload),
            Some(super::PayloadSchema::Function(witness))
                if !matches!(witness.origin, ValueFunctionOrigin::Parameter(_)) => {}
            _ => return false,
        }
    }
    true
}

fn ordinary_data(program: &Program, data: TypeId) -> bool {
    matches!(
        program.data_schema(data),
        Some(
            super::PayloadSchema::Builtin(_)
                | super::PayloadSchema::Aggregate(_)
                | super::PayloadSchema::Array(_)
                | super::PayloadSchema::Optional(_)
                | super::PayloadSchema::Fallible(_)
        )
    )
}

fn replay_types(
    header: &Program,
    body: &Program,
    owner: ValueCodeRoot,
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
                application.owner.root() == owner
                    && match application.owner {
                        ValueCode::Closure(id) => body.closure(id).is_some(),
                        ValueCode::Declaration(_) | ValueCode::Expression(_) => true,
                    }
                    && header
                        .codes
                        .get(application.callee)
                        .and_then(super::Code::result_contract)
                        .is_some_and(|contract| contract.invocation.can_supply(*invocation))
            }
            TypeKind::Optional(payload) => structural_component(body, *payload),
            TypeKind::Fallible(payload) => {
                structural_component(body, *payload)
                    && body.has_failure_layer(*payload) == Some(false)
            }
            TypeKind::Tuple(fields) => fields.iter().all(|data| structural_component(body, data)),
            TypeKind::FixedArray { element, length } => {
                length.closed_value().is_some() && structural_component(body, *element)
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
            TypeKind::Optional(payload) => TypeKind::Optional(project(*payload)?),
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
