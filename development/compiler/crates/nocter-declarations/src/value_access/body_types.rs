//! Owner-scoped body type construction and canonical replay over one immutable header prefix.
use super::{Program, TypeScope};
use crate::GenericParameterDomain;
use nocter_model::{
    GenericParameterId, TypeCursor, TypeId, TypeKind, UsizeTerm, ValueCode, ValueCodeRoot,
    ValueFunctionOrigin,
};
use std::collections::BTreeMap;
mod applications;
mod containment;
mod contracts;
mod generics;
pub use generics::GenericFrameError;
pub(super) use generics::GenericFrameTypes;
mod inputs;
mod resolution;
pub use resolution::ResolvedBodyTypes;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyTypeError {
    ForeignDeclarations,
    InvalidApplication,
    InvalidExtension,
    UnsupportedCallableSignature,
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

/// Descriptor dependencies over one canonical payload schema, without storage or access evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayloadDependencies {
    parameters: std::collections::BTreeSet<super::FunctionWitness>,
    producers: std::collections::BTreeSet<super::FunctionWitness>,
    contracts: std::collections::BTreeSet<super::FunctionWitness>,
    requires_future: bool,
}
impl PayloadDependencies {
    /// This shape needs concrete future lifecycle evidence at its value boundary.
    #[must_use]
    pub const fn requires_future(&self) -> bool {
        self.requires_future
    }
    /// Keep each frozen view: one formal can appear at more than one invocation view.
    #[must_use]
    pub const fn parameters(&self) -> &std::collections::BTreeSet<super::FunctionWitness> {
        &self.parameters
    }
    /// Valid type contracts still requiring actual source-value context, never flat inputs.
    #[must_use]
    pub const fn contracts(&self) -> &std::collections::BTreeSet<super::FunctionWitness> {
        &self.contracts
    }
    /// Exact unresolved result/application terms, including their selected invocation views.
    #[must_use]
    pub const fn producers(&self) -> &std::collections::BTreeSet<super::FunctionWitness> {
        &self.producers
    }
    #[must_use]
    pub fn has_pending_producers(&self) -> bool {
        !self.producers.is_empty()
    }
    #[must_use]
    pub fn readiness(&self) -> PayloadReadiness {
        if self.requires_future || !self.parameters.is_empty() || !self.contracts.is_empty() {
            PayloadReadiness::Unsupported
        } else if self.has_pending_producers() {
            PayloadReadiness::Pending
        } else {
            PayloadReadiness::Closed
        }
    }
}

/// The sole construction capability for one body's closed type extension, with shared header facts.
#[derive(Clone, Debug)]
pub struct BodyTypes {
    owner: ValueCodeRoot,
    program: Program,
    next_closure: Option<u32>,
}
impl BodyTypes {
    /// Publish a callable input signature before its anonymous body is checked. The formal carries
    /// no concrete environment or Copy proof. Canonical publication requires its completed owner.
    /// # Errors
    /// Rejects foreign/repeated/finalized inputs or a signature outside ordinary data.
    pub fn closure_parameter(
        &mut self,
        owner: nocter_model::ValueClosure,
        position: usize,
        contract: super::FunctionConstraint,
    ) -> Result<TypeId, BodyTypeError> {
        self.publish_closure_parameter(
            owner,
            nocter_model::ValueParameter::Closure { owner, position },
            contract,
        )
    }
    /// Publish one callable leaf of an explicit compound input without adding a runtime input.
    /// # Errors
    /// Rejects foreign/repeated/finalized identities or unsupported callable signatures.
    pub fn closure_component(
        &mut self,
        owner: nocter_model::ValueClosure,
        position: usize,
        ordinal: usize,
        contract: super::FunctionConstraint,
    ) -> Result<TypeId, BodyTypeError> {
        self.publish_closure_parameter(
            owner,
            nocter_model::ValueParameter::Component {
                input: nocter_model::ValueInput::Closure { owner, position },
                ordinal,
            },
            contract,
        )
    }
    fn publish_closure_parameter(
        &mut self,
        owner: nocter_model::ValueClosure,
        parameter: nocter_model::ValueParameter,
        contract: super::FunctionConstraint,
    ) -> Result<TypeId, BodyTypeError> {
        if owner.owner != self.owner
            || self.program.closures.contains_key(&owner)
            || self.program.closure_parameters.contains_key(&parameter)
        {
            return Err(BodyTypeError::InvalidExtension);
        }
        if !signature_data(&self.program, contract.result)
            || contract
                .inputs
                .iter()
                .any(|input| !signature_data(&self.program, input.data))
        {
            return Err(BodyTypeError::UnsupportedCallableSignature);
        }
        let mut types = self.program.types.transaction();
        let selected = super::functions::intern_views(
            &mut types,
            ValueFunctionOrigin::Parameter(parameter),
            contract.invocation,
        );
        self.program.types = types
            .commit(&self.program.types)
            .map_err(|_| BodyTypeError::InvalidExtension)?;
        self.program.closure_parameters.insert(parameter, contract);
        if self.next_closure.is_some_and(|next| owner.slot >= next) {
            self.next_closure = owner.slot.checked_add(1);
        }
        Ok(selected)
    }
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
    /// Rejects an invalid arity or a component outside this authority's structural descriptor domain.
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
    /// Rejects an element outside the structural descriptor domain or a foreign length parameter.
    pub fn array(
        &mut self,
        element: TypeId,
        length: impl Into<UsizeTerm>,
    ) -> Result<TypeId, BodyTypeError> {
        let length = length.into();
        if !structural_component(&self.program, element) || !length_in_scope(&self.program, length)
        {
            return Err(BodyTypeError::InvalidExtension);
        }
        let mut types = self.program.types.transaction();
        let data = types
            .intern(TypeKind::FixedArray { element, length })
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
        self.unary_type(payload, TypeKind::Optional)
    }
    /// Intern a future descriptor without supplying an activation or lifecycle proof.
    /// # Errors
    /// Rejects an output outside this authority's structural descriptor domain.
    pub fn future(&mut self, output: TypeId) -> Result<TypeId, BodyTypeError> {
        self.unary_type(output, TypeKind::Future)
    }
    /// Intern a fallible descriptor; failure always stores the one built-in error payload.
    /// # Errors
    /// Rejects an unsupported payload or another failure on the uninterrupted outcome chain.
    pub fn fallible(&mut self, payload: TypeId) -> Result<TypeId, BodyTypeError> {
        if self.program.has_failure_layer(payload) != Some(false) {
            return Err(BodyTypeError::RepeatedFailure);
        }
        self.unary_type(payload, TypeKind::Fallible)
    }
    fn unary_type(
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
    /// Publish an owned environment descriptor; formal and result dependencies may remain.
    /// It has no inferred external R/D/S and grants no dependent payload or Copy evidence.
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
            || self
                .program
                .payload_dependencies(environment)
                .is_none_or(|dependencies| dependencies.requires_future())
            || !(ordinary(contract.result)
                || matches!(
                    self.program.data_schema(contract.result),
                    Some(super::PayloadSchema::Function(_))
                ))
            || !inputs::validate(&self.program, id, &contract.inputs)
        {
            return Err(BodyTypeError::InvalidExtension);
        }
        let declared = contract.invocation;
        let mut types = self.program.types.transaction();
        let selected =
            super::functions::intern_views(&mut types, ValueFunctionOrigin::Closure(id), declared);
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
        Ok(selected)
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
    /// Inspect each represented shape once; a dependent descriptor grants no storage or Copy proof.
    #[must_use]
    pub fn payload_readiness(&self, data: TypeId) -> PayloadReadiness {
        self.payload_dependencies(data)
            .map_or(PayloadReadiness::Unsupported, |dependencies| {
                dependencies.readiness()
            })
    }
    /// Derive formal and producer dependencies from the canonical schema, without another registry.
    /// Unsupported descriptors return `None`; valid formal descriptors remain unavailable payloads.
    #[must_use]
    pub fn payload_dependencies(&self, data: TypeId) -> Option<PayloadDependencies> {
        self.payload_dependencies_all([data])
    }
    /// Union dependencies across original data roots with one shared schema traversal.
    /// This batches metadata only and grants no payload, ownership or source receipt.
    #[must_use]
    pub fn payload_dependencies_all(
        &self,
        roots: impl IntoIterator<Item = TypeId>,
    ) -> Option<PayloadDependencies> {
        use super::PayloadSchema;
        let mut pending = roots.into_iter().collect::<Vec<_>>();
        let mut seen = std::collections::BTreeSet::new();
        let mut dependencies = PayloadDependencies {
            parameters: std::collections::BTreeSet::new(),
            producers: std::collections::BTreeSet::new(),
            contracts: std::collections::BTreeSet::new(),
            requires_future: false,
        };
        while let Some(data) = pending.pop() {
            if !seen.insert(data) {
                continue;
            }
            match self.data_schema(data)? {
                PayloadSchema::Builtin(_) => {}
                PayloadSchema::Enumeration(schema) => {
                    for &variant in schema.variants() {
                        pending.extend(
                            self.variant_components(data, variant)?
                                .map(|field| field.data),
                        );
                    }
                }
                PayloadSchema::Future(_) => dependencies.requires_future = true,
                PayloadSchema::Aggregate(schema) => {
                    pending.extend(schema.components().map(|component| component.data));
                }
                PayloadSchema::Optional(payload) | PayloadSchema::Fallible(payload) => {
                    pending.push(payload);
                }
                PayloadSchema::Array(array) => {
                    if !array.is_empty() {
                        pending.push(array.element());
                    }
                }
                PayloadSchema::Function(witness) => match witness.origin {
                    ValueFunctionOrigin::Declaration(_) => {}
                    ValueFunctionOrigin::Contract(_) => {
                        self.function_contract(witness)?;
                        dependencies.contracts.insert(witness);
                    }
                    ValueFunctionOrigin::Closure(id) => {
                        pending.push(self.closure(id)?.environment());
                    }
                    ValueFunctionOrigin::Result(_) | ValueFunctionOrigin::Application(_) => {
                        dependencies.producers.insert(witness);
                    }
                    ValueFunctionOrigin::Parameter(parameter) => {
                        self.parameter_contract(parameter)?;
                        dependencies.parameters.insert(witness);
                    }
                },
            }
        }
        Some(dependencies)
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
        let mut closure_parameters = BTreeMap::new();
        let mut applications = BTreeMap::new();
        let mut callable_contracts = BTreeMap::new();
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
            let projection = replay_types(body, owner, &mut types)?;
            for (&application, &template) in &body.applications {
                if application.application.owner.root() != owner
                    || body.function_data(template).is_none()
                    || applications.insert(application, template).is_some()
                {
                    return Err(BodyTypeError::InvalidApplication);
                }
            }
            for (&id, contract) in &body.callable_contracts {
                if id.owner != nocter_model::ValueContractOwner::Body(owner) {
                    return Err(BodyTypeError::InvalidExtension);
                }
                let projected = contract.try_map_data(|data| {
                    project_type(prefix, &projection, data).ok_or(BodyTypeError::InvalidExtension)
                })?;
                if callable_contracts.insert(id, projected).is_some() {
                    return Err(BodyTypeError::InvalidExtension);
                }
            }
            for (&parameter, contract) in &body.closure_parameters {
                let closure = inputs::owner(parameter).ok_or(BodyTypeError::InvalidExtension)?;
                if closure.owner != owner || body.closure(closure).is_none() {
                    return Err(BodyTypeError::InvalidExtension);
                }
                let project = |data| {
                    project_type(prefix, &projection, data).ok_or(BodyTypeError::InvalidExtension)
                };
                let contract = contract.try_map_data(project)?;
                if closure_parameters.insert(parameter, contract).is_some() {
                    return Err(BodyTypeError::InvalidExtension);
                }
            }
            for (&id, schema) in &body.closures {
                if id.owner != owner || !inputs::validate(body, id, &schema.contract.inputs) {
                    return Err(BodyTypeError::InvalidExtension);
                }
                let project = |data| {
                    project_type(prefix, &projection, data).ok_or(BodyTypeError::InvalidExtension)
                };
                let mut schema = schema.clone();
                schema.environment = project(schema.environment)?;
                schema.contract = schema.contract.try_map_data(project)?;
                if closures.insert(id, schema).is_some() {
                    return Err(BodyTypeError::InvalidExtension);
                }
            }
            projections.insert(owner, (prefix, projection));
        }
        let mut program = self.clone();
        program.type_scope = TypeScope::Canonical;
        program.closures = closures;
        program.closure_parameters = closure_parameters;
        program.applications = applications;
        program.callable_contracts = callable_contracts;
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
        if !seen.insert(data) || program.nominal_identity(data).is_some() {
            continue;
        }
        if let Some((slot, element)) = program.symbolic_type_parameter(data) {
            if !matches!(program.type_scope, TypeScope::Body(owner) if slot.owner() == owner.source_owner())
            {
                return false;
            }
            pending.extend(element);
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
            Some(super::PayloadSchema::Function(witness)) => {
                if let ValueFunctionOrigin::Parameter(parameter) = witness.origin
                    && program.parameter_contract(parameter).is_none()
                {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

fn length_in_scope(program: &Program, length: UsizeTerm) -> bool {
    length
        .parameter()
        .is_none_or(|id| parameter_in_scope(program, id, GenericParameterDomain::UsizeConstant))
}

fn parameter_in_scope(
    program: &Program,
    id: GenericParameterId,
    domain: GenericParameterDomain,
) -> bool {
    matches!(program.type_scope, TypeScope::Body(owner)
        if program.generic_parameter(id).is_some_and(|slot| slot.owner() == owner.source_owner() && slot.domain() == domain))
}

// A named signature can retain a reserved nominal identity without demanding its payload.
// Actual parameter values, invocation and ABI geometry require their independent readiness.
fn signature_data(program: &Program, data: TypeId) -> bool {
    program.nominal_identity(data).is_some() || ordinary_data(program, data)
}

fn ordinary_data(program: &Program, data: TypeId) -> bool {
    matches!(
        program.data_schema(data),
        Some(
            super::PayloadSchema::Builtin(_)
                | super::PayloadSchema::Enumeration(_)
                | super::PayloadSchema::Aggregate(_)
                | super::PayloadSchema::Array(_)
                | super::PayloadSchema::Optional(_)
                | super::PayloadSchema::Fallible(_)
        )
    )
}

fn replay_types(
    body: &Program,
    owner: ValueCodeRoot,
    types: &mut nocter_model::TypeTransaction,
) -> Result<BTreeMap<TypeId, TypeId>, BodyTypeError> {
    let prefix = &body.header_prefix;
    let mut projection = BTreeMap::new();
    for (source, kind) in body.types.store().iter_from(prefix.type_count()) {
        let valid = match kind {
            TypeKind::ValueFunction {
                origin: ValueFunctionOrigin::Parameter(parameter),
                invocation,
            } => {
                inputs::owner(*parameter).is_some_and(|closure| closure.owner == owner)
                    && body
                        .parameter_contract(*parameter)
                        .is_some_and(|contract| contract.invocation.can_supply(*invocation))
            }
            TypeKind::ValueFunction {
                origin: ValueFunctionOrigin::Contract(id),
                invocation,
            } => {
                id.owner == nocter_model::ValueContractOwner::Body(owner)
                    && body
                        .callable_contracts
                        .get(id)
                        .is_some_and(|contract| contract.invocation.can_supply(*invocation))
            }
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
                application.application.owner.root() == owner
                    && match application.application.owner {
                        ValueCode::Closure(id) => body.closure(id).is_some(),
                        ValueCode::Declaration(_) | ValueCode::Expression(_) => true,
                    }
                    && body
                        .application_template(*application)
                        .is_some_and(|template| template.invocation.can_supply(*invocation))
            }
            TypeKind::Optional(payload) | TypeKind::Future(payload) => {
                structural_component(body, *payload)
            }
            TypeKind::Fallible(payload) => {
                structural_component(body, *payload)
                    && body.has_failure_layer(*payload) == Some(false)
            }
            TypeKind::Tuple(fields) => fields.iter().all(|data| structural_component(body, data)),
            TypeKind::FixedArray { element, length } => {
                length_in_scope(body, *length) && structural_component(body, *element)
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
            TypeKind::Future(payload) => TypeKind::Future(project(*payload)?),
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
