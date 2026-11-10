//! Incremental publication of complete contracts into one persistent header lineage.
use super::{
    Callable, DeclarationId, FunctionConstraint, FunctionInput, Parameter, ParameterSlot, Program,
    Reservation, ValueDefinition, data::resolved_data, functions, records,
};
use nocter_model::{
    AccessCallableId, AccessParameterId, Arena, SparseArenaBuilder, Symbol, SymbolTable,
    TypeAuthority, TypeKind, ValueFunctionOrigin, ValueInvocation,
};
use std::{collections::BTreeMap, sync::Arc};

#[cfg(test)]
mod tests;

/// One atomic header publication step; required records grant no provisional contract.
#[derive(Debug, Eq, PartialEq)]
pub enum HeaderStep {
    Ready,
    Require(Box<[DeclarationId]>),
}

/// Single writer for a validated source header domain. Views contain only complete contracts.
/// Source references and finite record containment are validated before publication starts.
#[derive(Debug)]
pub struct HeaderPublication {
    reserved: Arena<AccessCallableId, Reservation>,
    parameter_slots: Arena<AccessParameterId, ParameterSlot>,
    callables: SparseArenaBuilder<AccessCallableId, Callable>,
    parameters: SparseArenaBuilder<AccessParameterId, Parameter>,
    types: TypeAuthority,
    symbols: Arc<SymbolTable>,
    namespace: Arc<BTreeMap<Symbol, DeclarationId>>,
    records: records::Publication,
}
impl HeaderPublication {
    /// All reserved code identities; missing contracts grant no body-checking admission.
    #[must_use]
    pub fn callable_ids(&self) -> impl ExactSizeIterator<Item = AccessCallableId> + '_ {
        self.reserved.iter().map(|(id, _)| id)
    }
    /// The closed declaration domain, independently of publication readiness.
    #[must_use]
    pub fn declaration_ids(&self) -> impl ExactSizeIterator<Item = DeclarationId> + '_ {
        self.namespace.values().copied()
    }
    pub(super) fn new(
        symbols: SymbolTable,
        reserved: Arena<AccessCallableId, Reservation>,
        parameter_slots: Arena<AccessParameterId, ParameterSlot>,
        namespace: BTreeMap<Symbol, DeclarationId>,
        records: records::Validated,
    ) -> Self {
        let mut types = TypeAuthority::new();
        let records = records::Publication::new(records, &mut types);
        let types = functions::publish_witnesses(
            &types,
            reserved.iter().filter_map(|(id, r)| {
                matches!(
                    r.definition.as_ref()?.kind,
                    super::CallableKind::Function(_)
                )
                .then_some(id)
            }),
        );
        Self {
            callables: SparseArenaBuilder::new(reserved.len()),
            parameters: SparseArenaBuilder::new(parameter_slots.len()),
            reserved,
            parameter_slots,
            types,
            symbols: symbols.into(),
            namespace: namespace.into(),
            records,
        }
    }
    /// Normalize one contract and its record dependencies, retaining all older ready views.
    /// # Errors
    /// Rejects an identity outside this validated declaration domain.
    /// # Panics
    /// Panics only if validated internal metadata or its exact type transaction loses integrity.
    pub fn publish(&mut self, id: impl Into<DeclarationId>) -> Result<(), crate::DefinitionError> {
        let mut pending = vec![id.into()];
        while let Some(&id) = pending.last() {
            match self.advance(id)? {
                HeaderStep::Ready => {
                    pending.pop();
                }
                HeaderStep::Require(dependencies) => {
                    pending.extend(dependencies.into_vec().into_iter().rev());
                }
            }
        }
        Ok(())
    }
    /// Publish exactly one contract, or report all of its currently missing record contracts.
    /// # Errors
    /// Rejects an identity outside this closed domain without changing any ready contract.
    /// # Panics
    /// Panics only if validated internal metadata loses integrity.
    pub fn advance(
        &mut self,
        id: impl Into<DeclarationId>,
    ) -> Result<HeaderStep, crate::DefinitionError> {
        match id.into() {
            DeclarationId::Record(id) => {
                let dependencies = self.records.dependencies(id)?;
                if !dependencies.is_empty() {
                    return Ok(HeaderStep::Require(
                        dependencies
                            .into_iter()
                            .map(DeclarationId::Record)
                            .collect(),
                    ));
                }
                self.records.publish_ready(id, &mut self.types);
            }
            DeclarationId::Callable(id) => {
                if self.callables.get(id).is_some() {
                    return Ok(HeaderStep::Ready);
                }
                let reservation = self
                    .reserved
                    .get(id)
                    .ok_or(crate::DefinitionError::UnknownId)?;
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
                    parameter.data.visit_data(&mut require_record);
                }
                definition.result_data.visit_data(require_record);
                if !records.is_empty() {
                    return Ok(HeaderStep::Require(records.into()));
                }
                self.publish_callable(id);
            }
        }
        Ok(HeaderStep::Ready)
    }
    fn publish_callable(&mut self, id: AccessCallableId) {
        let reservation = self.reserved.get(id).expect("reserved callable");
        let definition = reservation
            .definition
            .as_ref()
            .expect("validated definition");
        let mut types = self.types.transaction();
        let parameters: Vec<_> = reservation
            .parameters
            .iter()
            .map(|&id| {
                let slot = self.parameter_slots.get(id).expect("reserved parameter");
                let parameter = &definition.parameters[slot.position];
                let (data, contract) = publish_value(
                    &mut types,
                    &self.records,
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
            &self.records,
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
        self.callables
            .insert(
                id,
                Callable {
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
            symbols: self.symbols.clone(),
            types: self.types.clone(),
            header_prefix: self.types.store().clone(),
            type_scope: super::TypeScope::Headers,
            callables: self.callables.snapshot(),
            parameters: self.parameters.snapshot(),
            namespace: self.namespace.clone(),
            records: self.records.snapshot(),
        }
    }
    /// Publish all remaining contracts in canonical declaration order.
    /// # Panics
    /// Panics only if a reserved contract cannot be published from this validated domain.
    #[must_use]
    pub fn finish(mut self) -> Program {
        let records: Vec<_> = self.records.ids().collect();
        for id in records {
            self.publish(id).expect("reserved record");
        }
        let ids: Vec<_> = self.reserved.iter().map(|(id, _)| id).collect();
        for id in ids {
            self.publish(id).expect("reserved callable");
        }
        self.snapshot()
    }
}

fn publish_value(
    types: &mut nocter_model::TypeTransaction,
    records: &records::Publication,
    origin: ValueFunctionOrigin,
    definition: &ValueDefinition,
) -> (nocter_model::TypeId, Option<FunctionConstraint>) {
    let definition = match definition {
        ValueDefinition::Data(data) => {
            return (
                resolved_data(
                    types,
                    |id| records.get(id).expect("ready record").data(),
                    data,
                ),
                None,
            );
        }
        ValueDefinition::Function(definition) => definition,
    };
    let contract = FunctionConstraint {
        invocation: definition.invocation,
        inputs: definition
            .inputs
            .iter()
            .map(|(operation, data)| FunctionInput {
                data: resolved_data(
                    types,
                    |id| records.get(id).expect("ready record").data(),
                    data,
                ),
                operation: *operation,
            })
            .collect(),
        result: resolved_data(
            types,
            |id| records.get(id).expect("ready record").data(),
            &definition.result,
        ),
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
