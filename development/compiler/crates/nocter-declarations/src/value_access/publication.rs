//! Incremental publication of complete contracts into one persistent header lineage.
use super::{
    Builder, Code, CodeKind, DeclarationId, DeclarationRule, DeclarationViolation, FieldDefinition,
    FunctionConstraint, Parameter, ParameterDefinition, Program, ValueDefinition, functions,
    nominals, validate_definition,
};
use nocter_dependency_query::{
    DependencyProgress, DependencyQuery, DependencyQueryError, DependencyStep,
};
use nocter_model::{
    AccessCodeId, AccessParameterId, NominalTypeId, SparseArenaBuilder, Symbol, TypeAuthority,
    TypeKind, ValueFunctionOrigin,
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
    nominals: nominals::Publication,
    aliases: SparseArenaBuilder<nocter_model::TypeAliasId, super::TypeAlias>,
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
        let nominals = nominals::Publication::new(&definitions.nominals, &mut types);
        Self {
            codes: SparseArenaBuilder::new(definitions.codes.len()),
            parameters: SparseArenaBuilder::new(definitions.parameters.len()),
            aliases: SparseArenaBuilder::new(definitions.aliases.len()),
            definitions,
            types,
            nominals,
        }
    }
    /// Compare closed names and reserved slot assignments against a complete retained program.
    /// This is structural encoding evidence only: it grants no signature equality, typed lineage,
    /// source equivalence or permission to reuse a body. The retained program is never replaced.
    #[must_use]
    pub fn has_same_reserved_layout(&self, retained: &Program) -> bool {
        retained.headers_complete()
            && self.definitions.symbols.as_ref() == retained.symbols.as_ref()
            && self.definitions.namespace.as_ref() == retained.namespace.as_ref()
            && self.definitions.codes.len() == retained.codes.len()
            && self.definitions.destructors == retained.destructors
            && self.definitions.generic_parameters == retained.generic_parameters
            && self.definitions.parameters.len() == retained.parameters.len()
            && self.definitions.aliases.len() == retained.aliases.len()
            && self.definitions.codes.iter().all(|(id, slot)| {
                retained.codes.get(id).is_some_and(|code| {
                    code.name == slot.name && code.parameters == slot.parameters
                })
            })
            && self.definitions.parameters.iter().all(|(id, slot)| {
                retained.parameters.get(id).is_some_and(|parameter| {
                    parameter.owner == slot.owner && parameter.position == slot.position
                })
            })
            && self
                .definitions
                .nominals
                .has_same_reserved_layout(&retained.nominals)
    }
    /// Whether this reserved definition has been loaded; no typed readiness is implied.
    #[must_use]
    pub fn is_defined(&self, id: impl Into<DeclarationId>) -> Option<bool> {
        match id.into() {
            DeclarationId::Code(id) => Some(self.definitions.codes.get(id)?.definition.is_some()),
            DeclarationId::Nominal(id) => self.definitions.nominals.is_defined(id),
            DeclarationId::Alias(id) => {
                Some(self.definitions.aliases.get(id)?.definition.is_some())
            }
        }
    }
    /// Reserved input slots, without typed parameter contracts.
    #[must_use]
    pub fn parameter_ids(&self, code: AccessCodeId) -> Option<&[AccessParameterId]> {
        self.definitions.parameter_ids(code)
    }
    /// Reserved representation slots, without payload or Copy readiness.
    #[must_use]
    pub fn field_ids(&self, record: NominalTypeId) -> Option<&[nocter_model::AccessFieldId]> {
        self.definitions.field_ids(record)
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
        self.resolve_data_in(super::ScopeId::ROOT, name)
    }
    /// Resolve a leaf in an explicit declaration scope through the same closed name authority.
    /// # Errors
    /// Reports unknown scopes or data names without publishing an incomplete schema.
    pub fn resolve_data_in(
        &self,
        scope: super::ScopeId,
        name: Symbol,
    ) -> Result<super::DataReference, DeclarationRule> {
        super::data::resolve_ready_alias(self.definitions.resolve_data_in(scope, name), |id| {
            self.aliases.get(id)
        })
    }
    /// Load one define-once raw callable, preserving all reserved slots and ready views.
    /// # Errors
    /// Rejects unknown IDs, mismatched arity, generic slots or a second definition without
    /// replacement.
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
    /// Load one define-once enumeration into the fixed variant and payload domain.
    /// # Errors
    /// Rejects foreign IDs, mismatched arity or an already-defined schema.
    pub fn define_enum(
        &mut self,
        id: NominalTypeId,
        copyable: bool,
        variants: impl Into<Box<[super::VariantDefinition]>>,
    ) -> Result<(), crate::DefinitionError> {
        self.definitions.define_enum(id, copyable, variants)
    }
    /// Load one alias target, preserving the original closed name and data domains.
    /// # Errors
    /// Rejects foreign or already-defined aliases.
    pub fn define_alias(
        &mut self,
        id: nocter_model::TypeAliasId,
        target: super::DataReference,
    ) -> Result<(), crate::DefinitionError> {
        let alias = self
            .definitions
            .aliases
            .get_mut(id)
            .ok_or(crate::DefinitionError::UnknownId)?;
        if alias.definition.is_some() {
            return Err(crate::DefinitionError::AlreadyDefined);
        }
        alias.definition = Some(target);
        Ok(())
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
        if let Some(&declaration) = path.iter().find(|id| matches!(id, DeclarationId::Alias(_))) {
            return HeaderFailure::Rejected(vec![DeclarationViolation {
                declaration,
                position: super::HeaderPosition::Result,
                rule: DeclarationRule::RecursiveAlias,
            }]);
        }
        let nominals = path
            .iter()
            .map(|id| match id {
                DeclarationId::Nominal(id) => Some(*id),
                DeclarationId::Code(_) | DeclarationId::Alias(_) => None,
            })
            .collect::<Option<Vec<_>>>();
        nominals
            .and_then(|path| self.definitions.nominals.cycle_violation(&path))
            .map_or(HeaderFailure::Inconsistent, |violation| {
                HeaderFailure::Rejected(vec![violation])
            })
    }
    /// Publish exactly one contract, or report its currently missing nominal contracts.
    /// # Errors
    /// Reports missing/invalid definitions or unknown IDs while preserving established contracts.
    /// # Panics
    /// Panics only if validated internal metadata loses integrity.
    pub fn advance(&mut self, id: impl Into<DeclarationId>) -> Result<HeaderStep, HeaderFailure> {
        match id.into() {
            DeclarationId::Alias(id) => {
                if self.aliases.get(id).is_none() {
                    let alias = self
                        .definitions
                        .aliases
                        .get(id)
                        .ok_or(HeaderFailure::UnknownId)?;
                    let target = alias.publish(id, &self.definitions.nominals, &mut self.types)?;
                    self.aliases
                        .insert(id, target)
                        .map_err(|_| HeaderFailure::Inconsistent)?;
                }
            }
            DeclarationId::Nominal(id) => {
                if self.nominals.get(id).is_some() {
                    return Ok(HeaderStep::Ready);
                }
                self.definitions
                    .nominals
                    .validate(id, &self.definitions.symbols)?;
                let dependencies = self.nominals.dependencies(&self.definitions.nominals, id);
                if !dependencies.is_empty() {
                    return Ok(HeaderStep::Require(
                        dependencies
                            .into_iter()
                            .map(DeclarationId::Nominal)
                            .collect(),
                    ));
                }
                self.nominals
                    .publish_ready(&self.definitions.nominals, id, &mut self.types)
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
                    &self.definitions.nominals,
                    &mut violations,
                );
                if !violations.is_empty() {
                    return Err(HeaderFailure::Rejected(violations));
                }
                if let Some(CodeKind::Destructor(owner)) = reservation
                    .definition
                    .as_ref()
                    .map(|definition| definition.kind)
                {
                    let Some(nominal) = self.nominals.get(owner) else {
                        return Ok(HeaderStep::Require(Box::new([owner.into()])));
                    };
                    if nominal.copyable() {
                        return Err(HeaderFailure::Rejected(vec![DeclarationViolation {
                            declaration: id.into(),
                            position: super::HeaderPosition::Name,
                            rule: DeclarationRule::DestructorOnCopy,
                        }]));
                    }
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
                let mut components = Vec::new();
                let mut type_contracts = Vec::new();
                let (data, contract) = publish_value(
                    &mut types,
                    ValueFunctionOrigin::Parameter(id.into()),
                    &parameter.data,
                    &mut components,
                    nocter_model::ValueContractOwner::Parameter(id),
                    &mut type_contracts,
                );
                (
                    id,
                    Parameter {
                        owner: slot.owner,
                        binding: parameter.binding.clone(),
                        position: slot.position,
                        data,
                        operation: parameter.operation,
                        contract,
                        components: components.into(),
                        type_contracts: type_contracts.into(),
                    },
                )
            })
            .collect();
        let mut result_contracts = Vec::new();
        let mut type_contracts = Vec::new();
        let (result, root_contract) = publish_value(
            &mut types,
            ValueFunctionOrigin::Result(id.into()),
            &definition.result_data,
            &mut result_contracts,
            nocter_model::ValueContractOwner::Result(id),
            &mut type_contracts,
        );
        if let Some(contract) = root_contract {
            assert!(
                result_contracts.is_empty(),
                "a root callable has one result contract"
            );
            result_contracts.push(contract);
        }
        let result = if definition.kind == CodeKind::DeferredFunction {
            types
                .intern(TypeKind::Future(result))
                .expect("validated producer output")
        } else {
            result
        };
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
                    result_contracts: result_contracts.into(),
                    type_contracts: type_contracts.into(),
                },
            )
            .expect("define-once reserved callable");
    }
    /// Read-only evidence for the contracts published so far, never a complete-unit certificate.
    #[must_use]
    pub fn snapshot(&self) -> Program {
        Program {
            closures: BTreeMap::new(),
            closure_parameters: BTreeMap::new(),
            callable_contracts: BTreeMap::new(),
            applications: BTreeMap::new(),
            symbols: self.definitions.symbols.clone(),
            types: self.types.clone(),
            header_prefix: self.types.store().clone(),
            type_scope: super::TypeScope::Headers,
            codes: self.codes.snapshot(),
            destructors: self.definitions.destructors.clone(),
            parameters: self.parameters.snapshot(),
            generic_parameters: self.definitions.generic_parameters.clone(),
            namespace: self.definitions.namespace.clone(),
            nominals: self.nominals.snapshot(),
            aliases: self.aliases.snapshot(),
        }
    }
    /// Publish all remaining contracts in canonical declaration order.
    /// # Errors
    /// Reports the first invalid prerequisite, without publishing its dependents.
    pub fn finish(mut self) -> Result<Program, HeaderFailure> {
        let aliases: Vec<_> = self.definitions.aliases.iter().map(|(id, _)| id).collect();
        for id in aliases {
            self.publish(id)?;
        }
        let nominals: Vec<_> = self
            .definitions
            .nominals
            .names()
            .map(|(id, _, _)| id)
            .collect();
        for id in nominals {
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
    components: &mut Vec<FunctionConstraint>,
    owner: nocter_model::ValueContractOwner,
    type_contracts: &mut Vec<FunctionConstraint>,
) -> (nocter_model::TypeId, Option<FunctionConstraint>) {
    let mut construct = |node| {
        Some(match node {
            super::data::ReferenceNode::Data(kind) => {
                types.intern(kind).expect("validated data reference")
            }
            super::data::ReferenceNode::Function {
                contract,
                in_signature,
            } => {
                let selected = if in_signature {
                    ValueFunctionOrigin::Contract(nocter_model::ValueCallableContract {
                        owner,
                        ordinal: type_contracts.len(),
                    })
                } else {
                    match origin {
                        ValueFunctionOrigin::Parameter(
                            nocter_model::ValueParameter::Declaration(input),
                        ) => ValueFunctionOrigin::Parameter(
                            nocter_model::ValueParameter::Component {
                                input: input.into(),
                                ordinal: components.len(),
                            },
                        ),
                        ValueFunctionOrigin::Result(result) if result.ordinal == 0 => {
                            ValueFunctionOrigin::Result(nocter_model::ValueCallableResult {
                                owner: result.owner,
                                ordinal: components.len(),
                            })
                        }
                        _ => unreachable!("declaration-owned callable component"),
                    }
                };
                let data = functions::intern_views(types, selected, contract.invocation);
                if in_signature {
                    type_contracts.push(contract);
                } else {
                    components.push(contract);
                }
                data
            }
        })
    };
    match definition {
        ValueDefinition::Data(data) => (
            super::data::resolve_reference(data, &mut construct)
                .expect("validated input descriptor"),
            None,
        ),
        ValueDefinition::Function(definition) => {
            let contract = super::data::resolve_function_definition(definition, construct)
                .expect("validated callable signature");
            let selected = functions::intern_views(types, origin, contract.invocation);
            (selected, Some(contract))
        }
    }
}
