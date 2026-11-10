//! Incremental publication of validated callable contracts into one persistent header lineage.
use super::{
    Callable, DeclarationId, FunctionConstraint, FunctionInput, Parameter, ParameterSlot, Program,
    Reservation, ValueDefinition, data::resolved_data, functions, records,
};
use nocter_model::{
    AccessCallableId, AccessParameterId, Arena, SparseArenaBuilder, Symbol, SymbolTable,
    TypeAuthority, TypeKind, ValueFunctionOrigin, ValueInvocation,
};
use std::{collections::BTreeMap, sync::Arc};

/// Single writer for a validated source header domain. Views contain only complete contracts.
/// Record schemas are closed together before this construction state is opened.
#[derive(Debug)]
pub struct HeaderPublication {
    reserved: Arena<AccessCallableId, Reservation>,
    parameter_slots: Arena<AccessParameterId, ParameterSlot>,
    callables: SparseArenaBuilder<AccessCallableId, Callable>,
    parameters: SparseArenaBuilder<AccessParameterId, Parameter>,
    types: TypeAuthority,
    symbols: Arc<SymbolTable>,
    namespace: Arc<BTreeMap<Symbol, DeclarationId>>,
    records: Arc<records::Schemas>,
}
impl HeaderPublication {
    pub(super) fn new(
        symbols: SymbolTable,
        reserved: Arena<AccessCallableId, Reservation>,
        parameter_slots: Arena<AccessParameterId, ParameterSlot>,
        namespace: BTreeMap<Symbol, DeclarationId>,
        records: records::Validated,
    ) -> Self {
        let (types, records) = records.publish();
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
            records: records.into(),
        }
    }
    /// Normalize one callable once, keeping its reserved input identities and all older views.
    /// # Errors
    /// Rejects an identity outside this validated declaration domain.
    /// # Panics
    /// Panics only if validated internal metadata or its exact type transaction loses integrity.
    pub fn publish(&mut self, id: AccessCallableId) -> Result<(), crate::DefinitionError> {
        if self.callables.get(id).is_some() {
            return Ok(());
        }
        let reservation = self
            .reserved
            .get(id)
            .ok_or(crate::DefinitionError::UnknownId)?;
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
        Ok(())
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
            records: self.records.clone(),
        }
    }
    /// Publish all remaining contracts in canonical declaration order.
    /// # Panics
    /// Panics only if a reserved callable cannot be published from this validated domain.
    #[must_use]
    pub fn finish(mut self) -> Program {
        let ids: Vec<_> = self.reserved.iter().map(|(id, _)| id).collect();
        for id in ids {
            self.publish(id).expect("reserved callable");
        }
        self.snapshot()
    }
}

fn publish_value(
    types: &mut nocter_model::TypeTransaction,
    records: &records::Schemas,
    origin: ValueFunctionOrigin,
    definition: &ValueDefinition,
) -> (nocter_model::TypeId, Option<FunctionConstraint>) {
    let definition = match definition {
        ValueDefinition::Data(data) => {
            return (
                resolved_data(
                    types,
                    |id| records.records.get(id).expect("validated record").data(),
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
                    |id| records.records.get(id).expect("validated record").data(),
                    data,
                ),
                operation: *operation,
            })
            .collect(),
        result: resolved_data(
            types,
            |id| records.records.get(id).expect("validated record").data(),
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
