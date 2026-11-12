//! Incremental publication of complete contracts into one persistent header lineage.
use super::{
    Builder, Code, DeclarationId, DeclarationRule, DeclarationViolation, FieldDefinition,
    FunctionConstraint, FunctionInput, Parameter, ParameterDefinition, Program, ValueDefinition,
    data::resolved_data, functions, records, validate_definition,
};
use nocter_dependency_query::{
    DependencyProgress, DependencyQuery, DependencyQueryError, DependencyStep,
};
use nocter_model::{
    AccessCodeId, AccessParameterId, NominalTypeId, SparseArenaBuilder, Symbol, TypeAuthority,
    TypeKind, ValueFunctionOrigin, ValueInvocation,
};
use std::collections::BTreeMap;

#[cfg(test)]
mod tests;

/// One atomic header publication step; required records grant no provisional contract.
#[derive(Debug, Eq, PartialEq)]
pub enum HeaderStep {
    Ready,
    Require(Box<[DeclarationId]>),
}

/// Failure to establish one requested declaration contract.
#[derive(Debug, Eq, PartialEq)]
pub enum HeaderFailure {
    UnknownId,
    Rejected(Vec<DeclarationViolation>),
    Inconsistent,
}

/// Single writer for a closed source header domain. Views contain only complete contracts.
/// Names and slots are fixed; definition loading, validation and normalization may be deferred.
#[derive(Debug)]
pub struct HeaderPublication {
    definitions: Builder,
    codes: SparseArenaBuilder<AccessCodeId, Code>,
    parameters: SparseArenaBuilder<AccessParameterId, Parameter>,
    types: TypeAuthority,
    records: records::Publication,
}
impl HeaderPublication {
    /// All reserved code identities; missing contracts grant no body-checking admission.
    #[must_use]
    pub fn code_ids(&self) -> impl ExactSizeIterator<Item = AccessCodeId> + '_ {
        self.definitions.codes.iter().map(|(id, _)| id)
    }
    /// The closed named declaration domain, independently of publication readiness.
    /// Anonymous roots are enumerated separately by `code_ids`.
    pub fn declaration_ids(&self) -> impl Iterator<Item = DeclarationId> + '_ {
        self.definitions.namespace.declarations()
    }
    pub(super) fn new(definitions: Builder) -> Self {
        let mut types = TypeAuthority::new();
        let records = records::Publication::new(&definitions.records, &mut types);
        Self {
            codes: SparseArenaBuilder::new(definitions.codes.len()),
            parameters: SparseArenaBuilder::new(definitions.parameters.len()),
            definitions,
            types,
            records,
        }
    }
    /// Whether this reserved definition has been loaded; no typed readiness is implied.
    #[must_use]
    pub fn is_defined(&self, id: impl Into<DeclarationId>) -> Option<bool> {
        match id.into() {
            DeclarationId::Code(id) => Some(self.definitions.codes.get(id)?.definition.is_some()),
            DeclarationId::Record(id) => self.definitions.records.is_defined(id),
        }
    }
    /// The fixed symbol domain used by raw definitions and every ready view.
    #[must_use]
    pub fn symbols(&self) -> &nocter_model::SymbolTable {
        &self.definitions.symbols
    }
    /// Resolve a raw header leaf through the same closed namespace as complete construction.
    /// # Errors
    /// Reports unknown data names without publishing an incomplete schema.
    pub fn resolve_data(&self, name: Symbol) -> Result<super::DataReference, DeclarationRule> {
        self.definitions.resolve_data(name)
    }
    /// Resolve a leaf in an explicit declaration scope through the same closed name authority.
    /// # Errors
    /// Reports unknown scopes or data names without publishing an incomplete schema.
    pub fn resolve_data_in(
        &self,
        scope: super::ScopeId,
        name: Symbol,
    ) -> Result<super::DataReference, DeclarationRule> {
        self.definitions.resolve_data_in(scope, name)
    }
    /// Load one define-once raw callable, preserving all reserved slots and ready views.
    /// # Errors
    /// Rejects unknown IDs, mismatched arity or a second definition without replacement.
    pub fn define(
        &mut self,
        id: AccessCodeId,
        kind: super::CodeKind,
        parameters: impl Into<Box<[ParameterDefinition]>>,
        result: impl Into<ValueDefinition>,
    ) -> Result<(), crate::DefinitionError> {
        self.definitions.define(id, kind, parameters, result)
    }
    /// Load one define-once raw record into the fixed field domain.
    /// # Errors
    /// Rejects unknown IDs, mismatched arity or a second definition without replacement.
    pub fn define_record(
        &mut self,
        id: NominalTypeId,
        copy: bool,
        fields: impl Into<Box<[FieldDefinition]>>,
    ) -> Result<(), crate::DefinitionError> {
        self.definitions.define_record(id, copy, fields)
    }
    /// Normalize one contract and its record dependencies, retaining all older ready views.
    /// # Errors
    /// Reports invalid payload/Copy contracts or an identity outside this closed domain.
    /// # Panics
    /// Panics only if internal metadata or its exact type transaction loses integrity.
    pub fn publish(&mut self, id: impl Into<DeclarationId>) -> Result<(), HeaderFailure> {
        let mut requests = DependencyQuery::default();
        match requests.resolve_steps(self, id.into()) {
            Ok(_) => Ok(()),
            Err(DependencyQueryError::Computation(failure)) => Err(failure),
            Err(DependencyQueryError::Cycle(path)) => Err(self.reject_cycle(&path)),
        }
    }
    /// Classify an exact active record cycle at its closing field, without source lookup.
    #[must_use]
    pub fn reject_cycle(&self, path: &[DeclarationId]) -> HeaderFailure {
        let records = path
            .iter()
            .map(|id| match id {
                DeclarationId::Record(id) => Some(*id),
                DeclarationId::Code(_) => None,
            })
            .collect::<Option<Vec<_>>>();
        records
            .and_then(|path| self.definitions.records.cycle_violation(&path))
            .map_or(HeaderFailure::Inconsistent, |violation| {
                HeaderFailure::Rejected(vec![violation])
            })
    }
    /// Publish exactly one contract, or report all of its currently missing record contracts.
    /// # Errors
    /// Reports missing/invalid definitions or unknown IDs while preserving established contracts.
    /// # Panics
    /// Panics only if validated internal metadata loses integrity.
    pub fn advance(&mut self, id: impl Into<DeclarationId>) -> Result<HeaderStep, HeaderFailure> {
        match id.into() {
            DeclarationId::Record(id) => {
                if self.records.get(id).is_some() {
                    return Ok(HeaderStep::Ready);
                }
                self.definitions
                    .records
                    .validate(id, &self.definitions.symbols)?;
                let dependencies = self.records.dependencies(&self.definitions.records, id);
                if !dependencies.is_empty() {
                    return Ok(HeaderStep::Require(
                        dependencies
                            .into_iter()
                            .map(DeclarationId::Record)
                            .collect(),
                    ));
                }
                self.records
                    .publish_ready(&self.definitions.records, id, &mut self.types)
                    .map_err(HeaderFailure::Rejected)?;
            }
            DeclarationId::Code(id) => {
                if self.codes.get(id).is_some() {
                    return Ok(HeaderStep::Ready);
                }
                let reservation = self
                    .definitions
                    .codes
                    .get(id)
                    .ok_or(HeaderFailure::UnknownId)?;
                let mut violations = Vec::new();
                validate_definition(
                    &self.definitions.symbols,
                    &self.definitions.namespace,
                    id,
                    reservation,
                    &self.definitions.records,
                    &mut violations,
                );
                if !violations.is_empty() {
                    return Err(HeaderFailure::Rejected(violations));
                }
                let definition = reservation
                    .definition
                    .as_ref()
                    .expect("validated definition");
                let mut records = Vec::new();
                let mut require_record = |data: &super::DataReference| {
                    if let super::DataReference::Record(id) = data
                        && self.records.get(*id).is_none()
                    {
                        records.push(DeclarationId::Record(*id));
                    }
                };
                for parameter in &definition.parameters {
                    parameter.data.visit_payload_data(&mut require_record);
                }
                definition.result_data.visit_payload_data(require_record);
                if !records.is_empty() {
                    return Ok(HeaderStep::Require(records.into()));
                }
                self.publish_code(id);
            }
        }
        Ok(HeaderStep::Ready)
    }
    fn publish_code(&mut self, id: AccessCodeId) {
        let reservation = self.definitions.codes.get(id).expect("reserved code");
        let definition = reservation
            .definition
            .as_ref()
            .expect("validated definition");
        if definition.kind.is_callable() {
            self.types = functions::publish_witnesses(&self.types, std::iter::once(id));
        }
        let mut types = self.types.transaction();
        let parameters: Vec<_> = reservation
            .parameters
            .iter()
            .map(|&id| {
                let slot = self
                    .definitions
                    .parameters
                    .get(id)
                    .expect("reserved parameter");
                let parameter = &definition.parameters[slot.position];
                let (data, contract) = publish_value(
                    &mut types,
                    ValueFunctionOrigin::Parameter(id),
                    &parameter.data,
                );
                (
                    id,
                    Parameter {
                        owner: slot.owner,
                        name: parameter.name,
                        position: slot.position,
                        data,
                        operation: parameter.operation,
                        contract,
                    },
                )
            })
            .collect();
        let (result, result_contract) = publish_value(
            &mut types,
            ValueFunctionOrigin::Result(id),
            &definition.result_data,
        );
        self.types = types
            .commit(&self.types)
            .expect("exact header type authority");
        for (id, parameter) in parameters {
            self.parameters
                .insert(id, parameter)
                .expect("define-once reserved input");
        }
        self.codes
            .insert(
                id,
                Code {
                    name: reservation.name,
                    kind: definition.kind,
                    parameters: reservation.parameters.clone(),
                    result,
                    result_contract,
                },
            )
            .expect("define-once reserved callable");
    }
    /// Read-only evidence for the contracts published so far, never a complete-unit certificate.
    #[must_use]
    pub fn snapshot(&self) -> Program {
        Program {
            closures: BTreeMap::new(),
            symbols: self.definitions.symbols.clone(),
            types: self.types.clone(),
            header_prefix: self.types.store().clone(),
            type_scope: super::TypeScope::Headers,
            codes: self.codes.snapshot(),
            parameters: self.parameters.snapshot(),
            namespace: self.definitions.namespace.clone(),
            records: self.records.snapshot(),
        }
    }
    /// Publish all remaining contracts in canonical declaration order.
    /// # Errors
    /// Reports the first invalid prerequisite, without publishing its dependents.
    pub fn finish(mut self) -> Result<Program, HeaderFailure> {
        let records: Vec<_> = self
            .definitions
            .records
            .names()
            .map(|(id, _, _)| id)
            .collect();
        for id in records {
            self.publish(id)?;
        }
        let ids: Vec<_> = self.definitions.codes.iter().map(|(id, _)| id).collect();
        for id in ids {
            self.publish(id)?;
        }
        Ok(self.snapshot())
    }
}
impl DependencyProgress<DeclarationId, (), HeaderFailure> for HeaderPublication {
    fn advance(
        &mut self,
        _: &DependencyQuery<DeclarationId, ()>,
        id: DeclarationId,
    ) -> Result<DependencyStep<DeclarationId, ()>, HeaderFailure> {
        Ok(match self.advance(id)? {
            HeaderStep::Ready => DependencyStep::Complete(()),
            HeaderStep::Require(dependencies) => DependencyStep::Require(dependencies),
        })
    }
    fn no_progress(&mut self, _: DeclarationId) -> HeaderFailure {
        HeaderFailure::Inconsistent
    }
}

fn publish_value(
    types: &mut nocter_model::TypeTransaction,
    origin: ValueFunctionOrigin,
    definition: &ValueDefinition,
) -> (nocter_model::TypeId, Option<FunctionConstraint>) {
    let definition = match definition {
        ValueDefinition::Data(data) => {
            return (resolved_data(types, data), None);
        }
        ValueDefinition::Function(definition) => definition,
    };
    let contract = FunctionConstraint {
        invocation: definition.invocation,
        inputs: definition
            .inputs
            .iter()
            .map(|(operation, data)| FunctionInput {
                data: resolved_data(types, data),
                operation: *operation,
            })
            .collect(),
        result: resolved_data(types, &definition.result),
    };
    let mut selected = None;
    for invocation in [
        ValueInvocation::Read,
        ValueInvocation::Edit,
        ValueInvocation::Take,
    ] {
        if !contract.invocation.can_supply(invocation) {
            continue;
        }
        let data = types
            .intern(TypeKind::ValueFunction { origin, invocation })
            .expect("witness is a leaf");
        if invocation == contract.invocation {
            selected = Some(data);
        }
    }
    (selected.expect("declared invocation view"), Some(contract))
}
