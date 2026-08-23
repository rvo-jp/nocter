use std::collections::{BTreeMap, BTreeSet};

use crate::validation_call::validate_call;
use crate::validation_closure::validate_closure_aggregate;
use crate::validation_graph::{place_values, successors};
use crate::validation_place::place_facts;
use crate::validation_region::{
    validate_region_creation, validate_region_flow, validate_region_release,
    validate_region_selection,
};
use crate::validation_switch::validate_switch_subject;
use crate::validation_types::{
    is_integer, matches_nominal_member, matches_opaque_projection, matches_opaque_witness,
    nominal_application,
};
use crate::{
    MirAggregate, MirBinaryOperation, MirBody, MirBranchTarget, MirConstant, MirFunction,
    MirLocalKind, MirOperation, MirOperationKind, MirPlace, MirPlaceRoot, MirProjectionKind,
    MirReadMode, MirSwitchSubject, MirTerminator, MirUnaryOperation, MirValueDefinition,
};
use crate::{MirValidationEnvironment, MirValidationError};
use nocter_declarations::{NominalShape, ParameterOwner};
use nocter_model::{
    BorrowCapability, BuiltinField, BuiltinType, ExecutableItemId, FieldId, FieldIdentity,
    MirBlockId, MirDropFlagId, MirLocalId, MirOperationId, MirPlaceId, MirValueId, OpaqueTypeId,
    TypeId, TypeKind, TypeStore,
};

/// Validates every body-local reference, type relation, CFG edge, and SSA use.
///
/// # Errors
///
/// Returns the first invariant violation in deterministic arena order.
pub fn validate_function(
    function: &MirFunction,
    environment: &impl MirValidationEnvironment,
) -> Result<(), MirValidationError> {
    let context = ValidationContext {
        function: function.body(),
        contract: BodyContract::Function {
            item: function.item(),
            result: function.result(),
        },
        environment,
        types: environment.types(),
    };
    context.validate()
}

pub(crate) fn validate_root(
    body: &MirBody,
    environment: &impl MirValidationEnvironment,
) -> Result<(), MirValidationError> {
    let context = ValidationContext {
        function: body,
        contract: BodyContract::Root,
        environment,
        types: environment.types(),
    };
    context.validate()
}

#[derive(Clone, Copy)]
enum BodyContract {
    Function {
        item: ExecutableItemId,
        result: TypeId,
    },
    Root,
}

struct ValidationContext<'a, E: ?Sized> {
    function: &'a MirBody,
    contract: BodyContract,
    environment: &'a E,
    types: &'a TypeStore,
}

impl<E: MirValidationEnvironment + ?Sized> ValidationContext<'_, E> {
    fn validate(&self) -> Result<(), MirValidationError> {
        if let BodyContract::Function { item, result } = self.contract {
            self.require_item(item)?;
            self.require_type(result)?;
        } else if !self.function.parameters().is_empty() || self.function.pack().is_some() {
            return Err(MirValidationError::InvalidRootSignature);
        }
        self.validate_pack_input()?;
        self.validate_parameters()?;
        for (id, local) in self.function.locals().iter() {
            self.require_type(local.ty())?;
            if is_non_storable(self.types, local.ty()) {
                return Err(MirValidationError::NonStorableLocal(id));
            }
        }
        for (place, value) in self.function.places().iter() {
            self.validate_place(place, value)?;
        }
        for (flag, value) in self.function.drop_flags().iter() {
            let place = self.require_place(value.place())?;
            if place_values(place).next().is_some() {
                return Err(MirValidationError::DynamicDropFlag { flag });
            }
        }
        self.validate_value_definitions()?;
        let operation_locations = self.validate_operation_membership()?;
        self.validate_pack_exits()?;
        let predecessors = self.validate_edges_and_reachability()?;
        let dominators = self.compute_dominators(&predecessors);
        self.validate_operations(&operation_locations, &dominators)?;
        self.validate_terminators(&operation_locations, &dominators)?;
        validate_region_flow(self.function)?;
        Ok(())
    }

    fn validate_parameters(&self) -> Result<(), MirValidationError> {
        let mut seen = BTreeSet::new();
        for (position, parameter) in self.function.parameters().iter().copied().enumerate() {
            if !seen.insert(parameter) {
                return Err(MirValidationError::DuplicateParameter(parameter));
            }
            let local = self.require_local(parameter)?;
            if local.kind() != (MirLocalKind::Parameter { position }) {
                return Err(MirValidationError::InvalidParameterKind {
                    parameter,
                    position,
                });
            }
        }
        for (local, value) in self.function.locals().iter() {
            if let MirLocalKind::Parameter { position } = value.kind()
                && self.function.parameters().get(position) != Some(&local)
            {
                return Err(MirValidationError::OrphanParameter(local));
            }
        }
        Ok(())
    }

    fn validate_pack_input(&self) -> Result<(), MirValidationError> {
        let (item, expected) = match self.contract {
            BodyContract::Function { item, .. } => {
                (Some(item), self.environment.item_pack_input(item))
            }
            BodyContract::Root => (None, None),
        };
        let actual = self.function.pack().map(|pack| {
            self.require_type(pack.element())?;
            self.require_type(pack.next())?;
            if !matches!(
                self.types.get(pack.next()),
                Some(TypeKind::Optional(payload)) if *payload == pack.element()
            ) {
                return Err(item.map_or(
                    MirValidationError::InvalidRootSignature,
                    MirValidationError::InvalidPackInput,
                ));
            }
            Ok((pack.element(), pack.next()))
        });
        if actual.transpose()? != expected {
            return Err(item.map_or(
                MirValidationError::InvalidRootSignature,
                MirValidationError::InvalidPackInput,
            ));
        }
        Ok(())
    }

    fn validate_pack_exits(&self) -> Result<(), MirValidationError> {
        for (block, body) in self.function.blocks().iter() {
            let destroys = body
                .operations()
                .iter()
                .copied()
                .filter(|operation| {
                    self.function
                        .operations()
                        .get(*operation)
                        .is_some_and(|operation| {
                            matches!(operation.kind(), MirOperationKind::DestroyPack)
                        })
                })
                .collect::<Vec<_>>();
            if !destroys.is_empty()
                && (!matches!(body.terminator(), MirTerminator::Return(_))
                    || destroys.len() != 1
                    || body.operations().last() != destroys.first())
            {
                return Err(MirValidationError::InvalidPackExit(block));
            }
            if self.function.pack().is_some()
                && matches!(body.terminator(), MirTerminator::Return(_))
                && destroys.len() != 1
            {
                return Err(MirValidationError::InvalidPackExit(block));
            }
        }
        Ok(())
    }

    fn validate_place(&self, id: MirPlaceId, place: &MirPlace) -> Result<(), MirValidationError> {
        self.require_type(place.ty())?;
        if is_non_storable(self.types, place.ty()) {
            return Err(MirValidationError::NonStorablePlace(id));
        }
        let mut current = match place.root() {
            MirPlaceRoot::Local(local) => {
                let local = self.require_local(local)?;
                local.ty()
            }
            MirPlaceRoot::Dereference { value, capability } => {
                let source = self.value_type(value)?;
                match self.types.get(source) {
                    Some(TypeKind::Borrow {
                        capability: actual,
                        referent,
                    }) if *actual == capability => *referent,
                    _ => return Err(MirValidationError::InvalidPlaceRoot { place: id }),
                }
            }
        };
        for projection in place.projections().iter().copied() {
            self.require_type(projection.ty())?;
            self.validate_projection(id, current, projection.kind(), projection.ty())?;
            current = projection.ty();
        }
        if current != place.ty() {
            return Err(MirValidationError::PlaceTypeMismatch { place: id });
        }
        Ok(())
    }

    fn validate_projection(
        &self,
        place: MirPlaceId,
        source: TypeId,
        projection: MirProjectionKind,
        result: TypeId,
    ) -> Result<(), MirValidationError> {
        match projection {
            MirProjectionKind::Field(FieldIdentity::Declared(field)) => {
                self.validate_declared_field_projection(place, source, field, result)?;
            }
            MirProjectionKind::Field(FieldIdentity::Builtin(field)) => {
                if !builtin_field_projection_matches(self.types, source, field, result) {
                    return Err(MirValidationError::InvalidProjection { place });
                }
            }
            MirProjectionKind::ClosureCapture(capture) => {
                if self.environment.closure_capture_type(source, capture) != Some(result) {
                    return Err(MirValidationError::InvalidProjection { place });
                }
            }
            MirProjectionKind::VariantPayload {
                variant: variant_id,
                parameter: parameter_id,
            } => {
                let (definition, arguments) = nominal_application(self.types, source, place)?;
                let variant = self
                    .environment
                    .variant(variant_id)
                    .ok_or(MirValidationError::UnknownVariant(variant_id))?;
                let parameter = self
                    .environment
                    .parameter(parameter_id)
                    .ok_or(MirValidationError::UnknownParameter(parameter_id))?;
                if variant.owner() != definition
                    || parameter.owner() != ParameterOwner::Variant(variant_id)
                    || !variant.payload().contains(&parameter_id)
                    || !matches_nominal_member(
                        self.environment,
                        self.types,
                        definition,
                        arguments,
                        parameter.ty(),
                        result,
                    )
                {
                    return Err(MirValidationError::InvalidProjection { place });
                }
            }
            MirProjectionKind::BorrowDereference(capability) => match self.types.get(source) {
                Some(TypeKind::Borrow {
                    capability: actual,
                    referent,
                }) if *actual == capability && *referent == result => {}
                _ => return Err(MirValidationError::InvalidProjection { place }),
            },
            MirProjectionKind::FixedIndex(index) => match self.types.get(source) {
                Some(TypeKind::FixedArray { element, length })
                    if index < *length && *element == result => {}
                Some(TypeKind::Slice(element)) if *element == result => {}
                _ => return Err(MirValidationError::InvalidProjection { place }),
            },
            MirProjectionKind::DynamicIndex(index) => {
                if self.value_type(index)? != self.types.builtin(BuiltinType::Usize) {
                    return Err(MirValidationError::InvalidProjection { place });
                }
                match self.types.get(source) {
                    Some(TypeKind::FixedArray { element, .. } | TypeKind::Slice(element))
                        if *element == result => {}
                    Some(TypeKind::Builtin(BuiltinType::Str))
                        if result == self.types.builtin(BuiltinType::U8) => {}
                    _ => return Err(MirValidationError::InvalidProjection { place }),
                }
            }
            MirProjectionKind::OptionalPayload => match self.types.get(source) {
                Some(TypeKind::Optional(payload)) if *payload == result => {}
                _ => return Err(MirValidationError::InvalidProjection { place }),
            },
            MirProjectionKind::FallibleSuccess => match self.types.get(source) {
                Some(TypeKind::Fallible(payload)) if *payload == result => {}
                _ => return Err(MirValidationError::InvalidProjection { place }),
            },
            MirProjectionKind::FallibleFailure => match self.types.get(source) {
                Some(TypeKind::Fallible(_)) if result == self.types.builtin(BuiltinType::Error) => {
                }
                _ => return Err(MirValidationError::InvalidProjection { place }),
            },
            MirProjectionKind::OpaqueWitness(definition)
                if self.opaque_projection_matches(source, definition, result) => {}
            MirProjectionKind::OpaqueWitness(_) => {
                return Err(MirValidationError::InvalidProjection { place });
            }
        }
        Ok(())
    }

    fn validate_declared_field_projection(
        &self,
        place: MirPlaceId,
        source: TypeId,
        field: FieldId,
        result: TypeId,
    ) -> Result<(), MirValidationError> {
        let (definition, arguments) = nominal_application(self.types, source, place)?;
        let declaration = self
            .environment
            .field(field)
            .ok_or(MirValidationError::UnknownField(field))?;
        if declaration.owner() == definition
            && matches_nominal_member(
                self.environment,
                self.types,
                definition,
                arguments,
                declaration.ty(),
                result,
            )
        {
            Ok(())
        } else {
            Err(MirValidationError::InvalidProjection { place })
        }
    }

    fn opaque_projection_matches(
        &self,
        source: TypeId,
        definition: OpaqueTypeId,
        result: TypeId,
    ) -> bool {
        matches_opaque_projection(self.environment, self.types, source, definition, result)
    }

    fn validate_value_definitions(&self) -> Result<(), MirValidationError> {
        for (value, definition) in self.function.values().iter() {
            self.require_type(definition.ty())?;
            match definition.definition() {
                MirValueDefinition::BlockParameter { block, position } => {
                    let block_value = self
                        .require_block(block)?
                        .parameters()
                        .get(position)
                        .copied();
                    if block_value != Some(value) {
                        return Err(MirValidationError::InvalidValueDefinition(value));
                    }
                }
                MirValueDefinition::Operation(operation) => {
                    if self.require_operation(operation)?.result() != Some(value) {
                        return Err(MirValidationError::InvalidValueDefinition(value));
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_operation_membership(
        &self,
    ) -> Result<BTreeMap<MirOperationId, (MirBlockId, usize)>, MirValidationError> {
        let mut locations = BTreeMap::new();
        for (block, body) in self.function.blocks().iter() {
            for (position, operation) in body.operations().iter().copied().enumerate() {
                self.require_operation(operation)?;
                if locations.insert(operation, (block, position)).is_some() {
                    return Err(MirValidationError::DuplicateOperation(operation));
                }
            }
        }
        for (operation, _) in self.function.operations().iter() {
            if !locations.contains_key(&operation) {
                return Err(MirValidationError::OrphanOperation(operation));
            }
        }
        Ok(locations)
    }

    fn validate_edges_and_reachability(
        &self,
    ) -> Result<BTreeMap<MirBlockId, BTreeSet<MirBlockId>>, MirValidationError> {
        let entry = self.require_block(self.function.entry())?;
        if !entry.parameters().is_empty() {
            return Err(MirValidationError::EntryHasParameters);
        }
        let mut predecessors = self
            .function
            .blocks()
            .iter()
            .map(|(block, _)| (block, BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        for (source, block) in self.function.blocks().iter() {
            for target in successors(block.terminator()) {
                self.validate_edge(target)?;
                predecessors
                    .get_mut(&target.block())
                    .expect("validated successor block must exist")
                    .insert(source);
            }
        }
        let mut reachable = BTreeSet::new();
        let mut pending = vec![self.function.entry()];
        while let Some(block) = pending.pop() {
            if !reachable.insert(block) {
                continue;
            }
            pending.extend(
                successors(self.require_block(block)?.terminator()).map(MirBranchTarget::block),
            );
        }
        if let Some((block, _)) = self
            .function
            .blocks()
            .iter()
            .find(|(block, _)| !reachable.contains(block))
        {
            return Err(MirValidationError::UnreachableBlock(block));
        }
        Ok(predecessors)
    }

    fn validate_edge(&self, edge: &MirBranchTarget) -> Result<(), MirValidationError> {
        let destination = self.require_block(edge.block())?;
        if edge.arguments().len() != destination.parameters().len() {
            return Err(MirValidationError::EdgeArity {
                block: edge.block(),
                expected: destination.parameters().len(),
                actual: edge.arguments().len(),
            });
        }
        for (position, (argument, parameter)) in edge
            .arguments()
            .iter()
            .copied()
            .zip(destination.parameters().iter().copied())
            .enumerate()
        {
            if self.value_type(argument)? != self.value_type(parameter)? {
                return Err(MirValidationError::EdgeType {
                    block: edge.block(),
                    position,
                });
            }
        }
        Ok(())
    }

    fn compute_dominators(
        &self,
        predecessors: &BTreeMap<MirBlockId, BTreeSet<MirBlockId>>,
    ) -> BTreeMap<MirBlockId, BTreeSet<MirBlockId>> {
        let all = predecessors.keys().copied().collect::<BTreeSet<_>>();
        let mut dominators = predecessors
            .keys()
            .copied()
            .map(|block| {
                let initial = if block == self.function.entry() {
                    BTreeSet::from([block])
                } else {
                    all.clone()
                };
                (block, initial)
            })
            .collect::<BTreeMap<_, _>>();
        loop {
            let mut changed = false;
            for block in predecessors.keys().copied() {
                if block == self.function.entry() {
                    continue;
                }
                let mut incoming = predecessors[&block].iter();
                let first = *incoming
                    .next()
                    .expect("every reachable non-entry block has a predecessor");
                let mut next = dominators[&first].clone();
                for predecessor in incoming {
                    next.retain(|candidate| dominators[predecessor].contains(candidate));
                }
                next.insert(block);
                if next != dominators[&block] {
                    dominators.insert(block, next);
                    changed = true;
                }
            }
            if !changed {
                return dominators;
            }
        }
    }

    fn validate_operations(
        &self,
        locations: &BTreeMap<MirOperationId, (MirBlockId, usize)>,
        dominators: &BTreeMap<MirBlockId, BTreeSet<MirBlockId>>,
    ) -> Result<(), MirValidationError> {
        for (operation, value) in self.function.operations().iter() {
            let (block, position) = locations[&operation];
            if value.kind().produces_value() != value.result().is_some() {
                return Err(MirValidationError::InvalidOperationResult(operation));
            }
            let result = value
                .result()
                .map(|result| self.value_type(result))
                .transpose()?;
            self.validate_operation(operation, value, result)?;
            for used in self.operation_values(value)? {
                self.validate_use(used, block, position, locations, dominators)?;
            }
        }
        Ok(())
    }

    // Keeping the closed instruction set in one exhaustive match makes newly added operations a
    // compile-time validation obligation instead of silently accepting them.
    #[allow(clippy::too_many_lines)]
    fn validate_operation(
        &self,
        id: MirOperationId,
        operation: &MirOperation,
        result: Option<TypeId>,
    ) -> Result<(), MirValidationError> {
        let mismatch = || MirValidationError::OperationType(id);
        match operation.kind() {
            MirOperationKind::Constant(constant) => {
                let result = result.ok_or_else(mismatch)?;
                let valid = match constant {
                    MirConstant::Bool(_) => result == self.types.builtin(BuiltinType::Bool),
                    MirConstant::Integer(_) => is_integer(self.types, result),
                    MirConstant::Text(_) => matches!(
                        self.types.get(result),
                        Some(TypeKind::Borrow {
                            capability: BorrowCapability::Readonly,
                            referent,
                        }) if *referent == self.types.builtin(BuiltinType::Str)
                    ),
                };
                if !valid {
                    return Err(mismatch());
                }
            }
            MirOperationKind::Read { place, mode } => {
                let place_value = self.require_place(*place)?;
                let facts = place_facts(self.function, place_value)?;
                if result != Some(place_value.ty())
                    || (*mode == MirReadMode::Move && !facts.movable)
                {
                    return Err(mismatch());
                }
            }
            MirOperationKind::Borrow { place, capability } => {
                let place_value = self.require_place(*place)?;
                let facts = place_facts(self.function, place_value)?;
                if (*capability == BorrowCapability::ReadWrite && !facts.writable)
                    || !matches!(
                        result.and_then(|result| self.types.get(result)),
                        Some(TypeKind::Borrow { capability: actual, referent })
                            if actual == capability && *referent == place_value.ty()
                    )
                {
                    return Err(mismatch());
                }
            }
            MirOperationKind::Store { destination, value } => {
                let destination = self.require_place(*destination)?;
                if result.is_some()
                    || !place_facts(self.function, destination)?.writable
                    || destination.ty() != self.value_type(*value)?
                {
                    return Err(mismatch());
                }
            }
            MirOperationKind::Initialize { destination, value } => {
                let destination = self.require_place(*destination)?;
                if result.is_some() || destination.ty() != self.value_type(*value)? {
                    return Err(mismatch());
                }
            }
            MirOperationKind::SetDropFlag { flag, .. } => {
                self.require_drop_flag(*flag)?;
                if result.is_some() {
                    return Err(mismatch());
                }
            }
            MirOperationKind::Unary { operation, operand } => {
                let operand = self.value_type(*operand)?;
                let valid = match operation {
                    MirUnaryOperation::LogicalNot => {
                        operand == self.types.builtin(BuiltinType::Bool) && result == Some(operand)
                    }
                    MirUnaryOperation::Negate => {
                        is_integer(self.types, operand) && result == Some(operand)
                    }
                };
                if !valid {
                    return Err(mismatch());
                }
            }
            MirOperationKind::Binary {
                operation,
                left,
                right,
            } => {
                let left = self.value_type(*left)?;
                let right = self.value_type(*right)?;
                let valid = if matches!(
                    operation,
                    MirBinaryOperation::Equal | MirBinaryOperation::Less
                ) {
                    left == right && result == Some(self.types.builtin(BuiltinType::Bool))
                } else {
                    left == right && is_integer(self.types, left) && result == Some(left)
                };
                if !valid {
                    return Err(mismatch());
                }
            }
            MirOperationKind::IntegerConversion { operand } => {
                if !is_integer(self.types, self.value_type(*operand)?)
                    || !result.is_some_and(|result| is_integer(self.types, result))
                {
                    return Err(mismatch());
                }
            }
            MirOperationKind::Aggregate(aggregate) => {
                self.validate_aggregate(id, aggregate, result.ok_or_else(mismatch)?)?;
            }
            MirOperationKind::Call(call) => {
                validate_call(
                    self.environment,
                    self.function,
                    id,
                    call,
                    result.ok_or_else(mismatch)?,
                )?;
            }
            MirOperationKind::PackLength => {
                if self.function.pack().is_none()
                    || result != Some(self.types.builtin(BuiltinType::Usize))
                {
                    return Err(mismatch());
                }
            }
            MirOperationKind::PackNext => {
                if self.function.pack().map(crate::MirPackInput::next) != result {
                    return Err(mismatch());
                }
            }
            MirOperationKind::DestroyPack => {
                if self.function.pack().is_none() || result.is_some() {
                    return Err(mismatch());
                }
            }
            MirOperationKind::InvokeDrop {
                body,
                place,
                allocation,
            } => {
                self.require_item(*body)?;
                self.require_place(*place)?;
                match allocation {
                    crate::MirCallAllocation::Inherit => {}
                    crate::MirCallAllocation::Region(region) => {
                        validate_region_selection(self.environment, self.function, id, *region)?;
                    }
                    crate::MirCallAllocation::Explicit(_) => return Err(mismatch()),
                }
                if result.is_some() {
                    return Err(mismatch());
                }
            }
            MirOperationKind::ReportError { error } => {
                if !matches!(self.contract, BodyContract::Root)
                    || result.is_some()
                    || self.value_type(*error)? != self.types.builtin(BuiltinType::Error)
                {
                    return Err(mismatch());
                }
            }
            MirOperationKind::CreateRegion { parent, region } => validate_region_creation(
                self.environment,
                self.function,
                id,
                *parent,
                *region,
                result,
            )?,
            MirOperationKind::ReleaseRegion { region } => {
                validate_region_release(self.environment, self.function, id, *region, result)?;
            }
        }
        Ok(())
    }

    // Aggregate layouts are likewise checked exhaustively against their concrete result shape.
    #[allow(clippy::too_many_lines)]
    fn validate_aggregate(
        &self,
        operation: MirOperationId,
        aggregate: &MirAggregate,
        result: TypeId,
    ) -> Result<(), MirValidationError> {
        let invalid = || MirValidationError::OperationType(operation);
        match aggregate {
            MirAggregate::Struct { definition, fields } => {
                let Some(TypeKind::Nominal {
                    definition: actual,
                    arguments,
                }) = self.types.get(result)
                else {
                    return Err(invalid());
                };
                let nominal = self
                    .environment
                    .nominal_type(*definition)
                    .ok_or_else(invalid)?;
                let NominalShape::Struct {
                    fields: declared, ..
                } = nominal.shape()
                else {
                    return Err(invalid());
                };
                if actual != definition || declared.len() != fields.len() {
                    return Err(invalid());
                }
                for ((field, value), expected) in fields.iter().zip(declared) {
                    let declaration = self.environment.field(*field).ok_or_else(invalid)?;
                    if field != expected
                        || !matches_nominal_member(
                            self.environment,
                            self.types,
                            *definition,
                            arguments,
                            declaration.ty(),
                            self.value_type(*value)?,
                        )
                    {
                        return Err(invalid());
                    }
                }
            }
            MirAggregate::Enum { variant, payload } => {
                let Some(TypeKind::Nominal {
                    definition,
                    arguments,
                }) = self.types.get(result)
                else {
                    return Err(invalid());
                };
                let variant = self.environment.variant(*variant).ok_or_else(invalid)?;
                if variant.owner() != *definition || variant.payload().len() != payload.len() {
                    return Err(invalid());
                }
                for (parameter, value) in variant
                    .payload()
                    .iter()
                    .copied()
                    .zip(payload.iter().copied())
                {
                    let parameter = self.environment.parameter(parameter).ok_or_else(invalid)?;
                    if !matches_nominal_member(
                        self.environment,
                        self.types,
                        *definition,
                        arguments,
                        parameter.ty(),
                        self.value_type(value)?,
                    ) {
                        return Err(invalid());
                    }
                }
            }
            MirAggregate::FixedArray(values) => {
                let Some(TypeKind::FixedArray { element, length }) = self.types.get(result) else {
                    return Err(invalid());
                };
                if usize::try_from(*length).ok() != Some(values.len())
                    || values
                        .iter()
                        .copied()
                        .any(|value| self.value_type(value) != Ok(*element))
                {
                    return Err(invalid());
                }
            }
            MirAggregate::Optional(value) => {
                let Some(TypeKind::Optional(payload)) = self.types.get(result) else {
                    return Err(invalid());
                };
                if value.is_some_and(|value| self.value_type(value) != Ok(*payload)) {
                    return Err(invalid());
                }
            }
            MirAggregate::FallibleSuccess(value) => {
                let Some(TypeKind::Fallible(payload)) = self.types.get(result) else {
                    return Err(invalid());
                };
                let valid = match value {
                    Some(value) => self.value_type(*value)? == *payload,
                    None => matches!(
                        self.types.get(*payload),
                        Some(TypeKind::Builtin(BuiltinType::Void))
                    ),
                };
                if !valid {
                    return Err(invalid());
                }
            }
            MirAggregate::FallibleFailure(value) => {
                if !matches!(self.types.get(result), Some(TypeKind::Fallible(_)))
                    || self.value_type(*value)? != self.types.builtin(BuiltinType::Error)
                {
                    return Err(invalid());
                }
            }
            MirAggregate::Closure { body, captures } => {
                let capture_types = captures
                    .iter()
                    .map(|capture| self.value_type(capture.value()))
                    .collect::<Result<Vec<_>, _>>()?;
                validate_closure_aggregate(
                    self.environment,
                    operation,
                    result,
                    *body,
                    captures,
                    &capture_types,
                )?;
            }
            MirAggregate::Opaque { witness } => {
                if !matches_opaque_witness(
                    self.environment,
                    self.types,
                    result,
                    self.value_type(*witness)?,
                ) {
                    return Err(invalid());
                }
            }
        }
        Ok(())
    }

    fn validate_terminators(
        &self,
        locations: &BTreeMap<MirOperationId, (MirBlockId, usize)>,
        dominators: &BTreeMap<MirBlockId, BTreeSet<MirBlockId>>,
    ) -> Result<(), MirValidationError> {
        for (block, body) in self.function.blocks().iter() {
            let position = body.operations().len();
            let mut values = Vec::new();
            match body.terminator() {
                MirTerminator::Goto(target) => values.extend_from_slice(target.arguments()),
                MirTerminator::Branch {
                    condition,
                    then_target,
                    else_target,
                } => {
                    if self.value_type(*condition)? != self.types.builtin(BuiltinType::Bool) {
                        return Err(MirValidationError::NonBooleanBranch(block));
                    }
                    values.push(*condition);
                    values.extend_from_slice(then_target.arguments());
                    values.extend_from_slice(else_target.arguments());
                }
                MirTerminator::BranchDropFlag {
                    flag,
                    initialized,
                    uninitialized,
                } => {
                    self.require_drop_flag(*flag)?;
                    values.extend_from_slice(initialized.arguments());
                    values.extend_from_slice(uninitialized.arguments());
                }
                MirTerminator::Switch {
                    subject,
                    cases,
                    fallback,
                } => {
                    validate_switch_subject(
                        self.environment,
                        self.function,
                        block,
                        *subject,
                        cases,
                    )?;
                    match subject {
                        MirSwitchSubject::Value(value) => values.push(*value),
                        MirSwitchSubject::Place(place) => {
                            values.extend(place_values(self.require_place(*place)?));
                        }
                    }
                    let mut seen = BTreeSet::new();
                    for case in cases {
                        if !seen.insert(case.value()) {
                            return Err(MirValidationError::DuplicateSwitchCase(block));
                        }
                        values.extend_from_slice(case.target().arguments());
                    }
                    values.extend_from_slice(fallback.arguments());
                }
                MirTerminator::Return(value) => {
                    let BodyContract::Function { result, .. } = self.contract else {
                        return Err(MirValidationError::InvalidRootTerminator(block));
                    };
                    match (value, self.types.get(result)) {
                        (None, Some(TypeKind::Builtin(BuiltinType::Void))) => {}
                        (Some(value), _) if self.value_type(*value)? == result => {
                            values.push(*value);
                        }
                        _ => return Err(MirValidationError::InvalidReturn(block)),
                    }
                }
                MirTerminator::Exit(status) => {
                    if !matches!(self.contract, BodyContract::Root) {
                        return Err(MirValidationError::InvalidReturn(block));
                    }
                    if let Some(status) = status {
                        let status_type = self.value_type(*status)?;
                        if status_type != self.types.builtin(BuiltinType::I32)
                            && status_type != self.types.builtin(BuiltinType::Usize)
                        {
                            return Err(MirValidationError::InvalidRootTerminator(block));
                        }
                        values.push(*status);
                    }
                }
                MirTerminator::Trap | MirTerminator::Unreachable => {}
            }
            for value in values {
                self.validate_use(value, block, position, locations, dominators)?;
            }
        }
        Ok(())
    }

    fn validate_use(
        &self,
        value: MirValueId,
        use_block: MirBlockId,
        use_position: usize,
        locations: &BTreeMap<MirOperationId, (MirBlockId, usize)>,
        dominators: &BTreeMap<MirBlockId, BTreeSet<MirBlockId>>,
    ) -> Result<(), MirValidationError> {
        let value_data = self.require_value(value)?;
        let (definition_block, definition_position) = match value_data.definition() {
            MirValueDefinition::BlockParameter { block, .. } => (block, None),
            MirValueDefinition::Operation(operation) => {
                let (block, position) = locations
                    .get(&operation)
                    .copied()
                    .ok_or(MirValidationError::InvalidValueDefinition(value))?;
                (block, Some(position))
            }
        };
        let valid = if definition_block == use_block {
            definition_position.is_none_or(|position| position < use_position)
        } else {
            dominators[&use_block].contains(&definition_block)
        };
        if !valid {
            return Err(MirValidationError::ValueDoesNotDominate {
                value,
                block: use_block,
            });
        }
        Ok(())
    }

    fn operation_values(
        &self,
        operation: &MirOperation,
    ) -> Result<Vec<MirValueId>, MirValidationError> {
        let mut values = Vec::new();
        match operation.kind() {
            MirOperationKind::Constant(_)
            | MirOperationKind::SetDropFlag { .. }
            | MirOperationKind::PackLength
            | MirOperationKind::PackNext
            | MirOperationKind::DestroyPack
            | MirOperationKind::ReleaseRegion { .. } => {}
            MirOperationKind::CreateRegion { parent, .. } => values.push(*parent),
            MirOperationKind::ReportError { error } => values.push(*error),
            MirOperationKind::Read { place, .. }
            | MirOperationKind::Borrow { place, .. }
            | MirOperationKind::InvokeDrop { place, .. } => {
                values.extend(place_values(self.require_place(*place)?));
            }
            MirOperationKind::Store { destination, value }
            | MirOperationKind::Initialize { destination, value } => {
                values.extend(place_values(self.require_place(*destination)?));
                values.push(*value);
            }
            MirOperationKind::Unary { operand, .. }
            | MirOperationKind::IntegerConversion { operand } => values.push(*operand),
            MirOperationKind::Binary { left, right, .. } => {
                values.extend([*left, *right]);
            }
            MirOperationKind::Aggregate(aggregate) => match aggregate {
                MirAggregate::Struct { fields, .. } => {
                    values.extend(fields.iter().map(|(_, value)| *value));
                }
                MirAggregate::Enum { payload, .. } | MirAggregate::FixedArray(payload) => {
                    values.extend(payload.iter().copied());
                }
                MirAggregate::Closure { captures, .. } => {
                    values.extend(captures.iter().map(|capture| capture.value()));
                }
                MirAggregate::Optional(value) | MirAggregate::FallibleSuccess(value) => {
                    values.extend(*value);
                }
                MirAggregate::FallibleFailure(value) | MirAggregate::Opaque { witness: value } => {
                    values.push(*value);
                }
            },
            MirOperationKind::Call(call) => {
                values.extend(call.arguments().iter().copied());
                if let crate::MirCallAllocation::Explicit(place) = call.allocation() {
                    values.extend(place_values(self.require_place(place)?));
                }
                if let Some(pack) = call.pack().and_then(crate::MirCallPack::prepared) {
                    values.push(pack.length());
                    for segment in pack.segments() {
                        match segment {
                            crate::MirPackSegment::Value { value, .. } => values.push(*value),
                            crate::MirPackSegment::Spread(spread) => {
                                values.extend([spread.remaining(), spread.receiver()]);
                                values.extend(place_values(self.require_place(spread.iterator())?));
                            }
                        }
                    }
                }
            }
        }
        Ok(values)
    }

    fn require_type(&self, ty: TypeId) -> Result<(), MirValidationError> {
        self.types
            .get(ty)
            .map(|_| ())
            .ok_or(MirValidationError::UnknownType(ty))
    }

    fn require_item(&self, item: ExecutableItemId) -> Result<(), MirValidationError> {
        self.environment
            .contains_item(item)
            .then_some(())
            .ok_or(MirValidationError::UnknownItem(item))
    }

    fn require_local(&self, local: MirLocalId) -> Result<crate::MirLocal, MirValidationError> {
        self.function
            .locals()
            .get(local)
            .copied()
            .ok_or(MirValidationError::UnknownLocal(local))
    }

    fn require_drop_flag(
        &self,
        flag: MirDropFlagId,
    ) -> Result<crate::MirDropFlag, MirValidationError> {
        self.function
            .drop_flags()
            .get(flag)
            .copied()
            .ok_or(MirValidationError::UnknownDropFlag(flag))
    }

    fn require_place(&self, place: MirPlaceId) -> Result<&MirPlace, MirValidationError> {
        self.function
            .places()
            .get(place)
            .ok_or(MirValidationError::UnknownPlace(place))
    }

    fn require_value(&self, value: MirValueId) -> Result<crate::MirValue, MirValidationError> {
        self.function
            .values()
            .get(value)
            .copied()
            .ok_or(MirValidationError::UnknownValue(value))
    }

    fn value_type(&self, value: MirValueId) -> Result<TypeId, MirValidationError> {
        self.require_value(value).map(crate::MirValue::ty)
    }

    fn require_operation(
        &self,
        operation: MirOperationId,
    ) -> Result<&MirOperation, MirValidationError> {
        self.function
            .operations()
            .get(operation)
            .ok_or(MirValidationError::UnknownOperation(operation))
    }

    fn require_block(&self, block: MirBlockId) -> Result<&crate::MirBlock, MirValidationError> {
        self.function
            .blocks()
            .get(block)
            .ok_or(MirValidationError::UnknownBlock(block))
    }
}

fn is_non_storable(types: &TypeStore, ty: TypeId) -> bool {
    matches!(
        types.get(ty),
        Some(TypeKind::Builtin(BuiltinType::Void | BuiltinType::Never))
    )
}

fn builtin_field_projection_matches(
    types: &TypeStore,
    source: TypeId,
    field: BuiltinField,
    result: TypeId,
) -> bool {
    let text = types.builtin(BuiltinType::Str);
    types.get(source) == Some(&TypeKind::Builtin(BuiltinType::Error))
        && matches!(field, BuiltinField::ErrorCode | BuiltinField::ErrorMessage)
        && matches!(
            types.get(result),
            Some(TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent,
            }) if *referent == text
        )
}
