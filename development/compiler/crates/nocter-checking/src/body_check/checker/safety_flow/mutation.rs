use std::collections::BTreeSet;

use nocter_model::{
    BodyNodeId, BorrowCapability, CallableCapability, LocalBindingId, PlaceId, TypeId, TypeKind,
};

use super::{BodyChecker, OrderRelations, SafetyFlowState, SafetySymbol, SafetyValue};
use crate::{CheckedCall, CheckedOperation, ReceiverPreparation};

impl SafetyFlowState {
    pub(super) fn bind_copy(&mut self, local: LocalBindingId, source: Option<SafetyValue>) {
        let target = SafetyValue::local(local);
        self.invalidate(&target);
        let Some(source) = source else {
            return;
        };
        if let Some(range) = self.ranges.get(&source).copied() {
            self.ranges.insert(target.clone(), range);
        }
        self.order
            .equate(SafetySymbol::Value(target), SafetySymbol::Value(source));
    }

    pub(super) fn bind_alias(&mut self, local: LocalBindingId, target: SafetyValue) {
        self.aliases.insert(SafetyValue::local(local), target);
    }

    pub(super) fn invalidate(&mut self, target: &SafetyValue) {
        self.ranges.retain(|value, _| !value.overlaps(target));
        self.order.invalidate(target);
    }

    pub(super) fn clear_facts(&mut self) {
        self.ranges.clear();
        self.order = OrderRelations::default();
    }

    pub(super) fn canonicalize(&self, mut value: SafetyValue) -> SafetyValue {
        let mut visited = BTreeSet::new();
        loop {
            let alias = SafetyValue::new(value.root, []);
            if !visited.insert(alias.clone()) {
                return value;
            }
            let Some(target) = self.aliases.get(&alias) else {
                return value;
            };
            let mut projections = target.projections.to_vec();
            projections.extend_from_slice(&value.projections);
            value = SafetyValue::new(target.root, projections);
        }
    }
}

impl BodyChecker<'_, '_> {
    pub(in crate::body_check::checker) fn bind_safety_local(
        &mut self,
        local: LocalBindingId,
        value: BodyNodeId,
    ) {
        if matches!(
            self.builder
                .node_type(value)
                .and_then(|ty| self.types.get(ty)),
            Some(TypeKind::Borrow { .. })
        ) && let Some(place) = self.value_place(value)
            && let Some(target) = self
                .builder
                .place(place)
                .and_then(|place| self.safety_place(place.root(), place.projections()))
        {
            self.safety_flow.bind_alias(local, target);
            return;
        }
        let source = self.safety_value(value);
        self.safety_flow.bind_copy(local, source);
    }

    pub(in crate::body_check::checker) fn invalidate_place(&mut self, place: PlaceId) {
        let target = self
            .builder
            .place(place)
            .and_then(|place| self.safety_place(place.root(), place.projections()));
        if let Some(target) = target {
            self.safety_flow.invalidate(&target);
        } else {
            self.safety_flow.clear_facts();
        }
    }

    pub(in crate::body_check::checker) fn invalidate_call_mutations(&mut self, call: &CheckedCall) {
        if call
            .target()
            .callable_operand()
            .is_some_and(|(_, capability)| capability != CallableCapability::Readonly)
        {
            self.safety_flow.clear_facts();
            return;
        }
        if let Some(receiver) = call.receiver()
            && matches!(
                receiver.preparation(),
                ReceiverPreparation::BorrowPlace(BorrowCapability::ReadWrite)
                    | ReceiverPreparation::BorrowTemporary(BorrowCapability::ReadWrite)
                    | ReceiverPreparation::PreserveBorrow(BorrowCapability::ReadWrite)
            )
        {
            self.invalidate_mutable_value(receiver.value());
        } else if let Some(receiver) = call.receiver()
            && matches!(receiver.preparation(), ReceiverPreparation::Owned)
            && self.type_may_carry_mutable_alias(
                self.builder.node_type(receiver.value()),
                &mut BTreeSet::new(),
            )
        {
            self.safety_flow.clear_facts();
            return;
        }
        for argument in call.arguments() {
            let ty = self.builder.node_type(*argument);
            if matches!(
                ty.and_then(|ty| self.types.get(ty)),
                Some(TypeKind::Borrow {
                    capability: BorrowCapability::ReadWrite,
                    ..
                })
            ) {
                self.invalidate_mutable_value(*argument);
            } else if self.type_may_carry_mutable_alias(ty, &mut BTreeSet::new()) {
                self.safety_flow.clear_facts();
                return;
            }
        }
    }

    pub(super) fn type_may_carry_mutable_alias(
        &self,
        ty: Option<TypeId>,
        visiting: &mut BTreeSet<TypeId>,
    ) -> bool {
        let Some(ty) = ty else {
            return true;
        };
        if !visiting.insert(ty) {
            return false;
        }
        let result = match self.types.get(ty) {
            Some(
                TypeKind::Builtin(_)
                | TypeKind::ValueFunction {
                    origin: nocter_model::ValueFunctionOrigin::Declaration(_),
                    ..
                }
                | TypeKind::Borrow {
                    capability: BorrowCapability::Readonly,
                    ..
                },
            ) => false,
            Some(
                TypeKind::Borrow {
                    capability: BorrowCapability::ReadWrite,
                    ..
                }
                | TypeKind::GenericParameter(_)
                | TypeKind::InterfaceSelf(_)
                | TypeKind::Nominal { .. }
                | TypeKind::AssociatedProjection { .. }
                | TypeKind::ValueFunction {
                    origin:
                        nocter_model::ValueFunctionOrigin::Closure(_)
                        | nocter_model::ValueFunctionOrigin::Parameter(_)
                        | nocter_model::ValueFunctionOrigin::Result(_)
                        | nocter_model::ValueFunctionOrigin::Application(_),
                    ..
                }
                | TypeKind::Opaque { .. }
                | TypeKind::Pointer(_)
                | TypeKind::Future(_)
                | TypeKind::PackEntry { .. }
                | TypeKind::Closure { .. }
                | TypeKind::Callable(_),
            )
            | None => true,
            Some(
                TypeKind::Slice(element)
                | TypeKind::Optional(element)
                | TypeKind::Fallible(element),
            ) => self.type_may_carry_mutable_alias(Some(*element), visiting),
            Some(TypeKind::FixedArray { element, .. }) => {
                self.type_may_carry_mutable_alias(Some(*element), visiting)
            }
            Some(TypeKind::Tuple(elements)) => elements
                .iter()
                .any(|element| self.type_may_carry_mutable_alias(Some(element), visiting)),
        };
        visiting.remove(&ty);
        result
    }

    fn invalidate_mutable_value(&mut self, node: BodyNodeId) {
        if let Some(place) = self.value_place(node) {
            self.invalidate_place(place);
        } else {
            self.safety_flow.clear_facts();
        }
    }

    fn value_place(&self, node: BodyNodeId) -> Option<PlaceId> {
        match self.builder.node(node)?.operation() {
            CheckedOperation::Place(place)
            | CheckedOperation::Copy(place)
            | CheckedOperation::Move(place)
            | CheckedOperation::Borrow { place, .. } => Some(*place),
            CheckedOperation::BorrowConversion(conversion) => self.value_place(conversion.value()),
            _ => None,
        }
    }
}
