use nocter_model::{BodyNodeId, BorrowCapability, CallableCapability, TypeId, TypeKind};
use nocter_source_index::{SemanticEntity, SourceOrigin};
use nocter_syntax::{NodeId, NodeKind};

use super::call_planning::DeclaredCallGenerics;
use super::{BodyChecker, ResolvedPlace};
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::copyability::Copyability;
use crate::instance_operations::{MethodCandidate, receiver_supports};
use crate::syntax::{direct_identifier, direct_nodes, is_transparent_expression};
use crate::{
    CallTarget, CheckedCall, CheckedOperation, CheckedReceiver, CheckedReceiverCoercion,
    PlaceAccess, ReceiverPreparation, StaticSelection, TypedBodyInterruption,
    TypedBodyInterruptionKind,
};

enum ReceiverDraft {
    Place {
        syntax: NodeId,
        place: ResolvedPlace,
    },
    Value {
        value: BodyNodeId,
        ty: TypeId,
    },
}

impl ReceiverDraft {
    const fn ty(&self) -> TypeId {
        match self {
            Self::Place { place, .. } => place.ty,
            Self::Value { ty, .. } => *ty,
        }
    }

    fn is_owned_source(&self, types: &nocter_model::TypeStore) -> bool {
        !matches!(types.get(self.ty()), Some(TypeKind::Borrow { .. }))
            && match self {
                Self::Place { place, .. } => place.access == PlaceAccess::Owned,
                Self::Value { .. } => true,
            }
    }
}

impl BodyChecker<'_, '_> {
    pub(super) fn check_method_call(
        &mut self,
        node: NodeId,
        owner: NodeId,
        member: NodeId,
        call_suffix: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        if let Some((parameter, _)) = self.literal_pack_parameter(owner)? {
            return self.check_literal_pack_method(node, parameter, member, call_suffix, expected);
        }
        let receiver = self.method_receiver_draft(owner)?;
        let receiver_owner = receiver_owner(self.types, receiver.ty())?;
        let available = self.receiver_borrow_capability(&receiver)?;
        let consumable = receiver.is_owned_source(self.types);
        let Some(member_token) = direct_identifier(self.tree(), member) else {
            let origin = SourceOrigin::from_node(self.tree(), member)
                .map_err(|_| BodyCheckInternalError::InvalidSyntax(member))?;
            self.record_member_interruption_origin(origin, receiver_owner, available, consumable);
            return Err(BodyCheckInternalError::InvalidSyntax(member).into());
        };
        let member_name = self
            .graph
            .symbols()
            .get(self.token_text(member_token)?)
            .ok_or(BodyCheckInternalError::InvalidSyntax(member))?;
        let mut candidates = {
            let mut selector = self.instance_selector();
            selector
                .select_method_candidates(receiver_owner, member_name)
                .map_err(BodyCheckInternalError::from)?
        };
        candidates.retain(|candidate| {
            receiver_supports(available, consumable, candidate.receiver_capability())
        });
        if candidates.is_empty() {
            let mut selector = self.instance_selector();
            candidates = selector
                .select_coerced_method_candidates(receiver_owner, member_name, available)
                .map_err(BodyCheckInternalError::from)?;
        }
        let mut candidates = candidates.drain(..);
        let Some(selected) = candidates.next() else {
            self.record_member_interruption(member_token, receiver_owner, available, consumable)?;
            return Err(self.token_rule(BodyRule::InvalidCall, member_token)?);
        };
        if candidates.next().is_some() {
            self.record_member_interruption(member_token, receiver_owner, available, consumable)?;
            return Err(self.token_rule(BodyRule::InvalidCall, member_token)?);
        }
        self.finish_method_call(
            node,
            receiver,
            member_token,
            call_suffix,
            &selected,
            expected,
        )
    }

    fn finish_method_call(
        &mut self,
        node: NodeId,
        receiver: ReceiverDraft,
        member_token: nocter_syntax::SyntaxToken,
        call_suffix: NodeId,
        selected: &MethodCandidate,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let callable_id = selected.callable();
        let callable = self
            .graph
            .declarations()
            .callables()
            .get(callable_id)
            .cloned()
            .ok_or(BodyCheckInternalError::MissingCallable(callable_id))?;
        let fixed_arguments = selected.generic_arguments().as_slice().to_vec();
        let (preparation_capability, coercion) =
            if let Some(coercion) = selected.receiver_coercion() {
                (
                    callable_capability(coercion.source_capability()),
                    Some(CheckedReceiverCoercion::new(
                        coercion.selection().clone(),
                        coercion.result_preparation(),
                    )),
                )
            } else {
                (selected.receiver_capability(), None)
            };
        let receiver =
            self.materialize_method_receiver(receiver, preparation_capability, coercion)?;
        let plan = self.plan_declared_call(
            node,
            call_suffix,
            callable_id,
            &callable,
            DeclaredCallGenerics::specialized(
                callable.generic_parameters(),
                &fixed_arguments,
                selected.substitution(),
            ),
            expected,
        )?;
        self.project_method_member(member_token, selected.surface())?;
        let call = self.add_node(
            node,
            plan.result,
            CheckedOperation::Call(CheckedCall::new(
                CallTarget::Static(StaticSelection::new(
                    selected.dispatch(),
                    plan.generic_arguments,
                )),
                Some(receiver),
                plan.arguments,
            )),
        )?;
        expected.map_or(Ok(call), |expected| {
            self.apply_expected(node, call, expected)
        })
    }

    fn method_receiver_draft(&mut self, root: NodeId) -> Result<ReceiverDraft, BodyCheckError> {
        let mut syntax = root;
        while self.kind(syntax).is_ok_and(is_transparent_expression) {
            let children = direct_nodes(self.tree(), syntax);
            let [child] = children.as_slice() else {
                break;
            };
            syntax = *child;
        }
        let place = match self.kind(syntax)? {
            NodeKind::ReferenceExpression => Some(self.named_place(syntax)?),
            NodeKind::PostfixExpression
                if crate::syntax::direct_child(self.tree(), syntax, NodeKind::CallSuffix)
                    .is_none() =>
            {
                match self.postfix_place(syntax, BorrowCapability::Readonly) {
                    Ok(place) => Some(place),
                    Err(BodyCheckError::Internal(BodyCheckInternalError::UnsupportedSyntax(
                        _,
                        _,
                    ))) => None,
                    Err(error) => return Err(error),
                }
            }
            _ => None,
        };
        if let Some(place) = place {
            return Ok(ReceiverDraft::Place { syntax, place });
        }
        let value = self.check_expression(root, None)?;
        Ok(ReceiverDraft::Value {
            value,
            ty: self.node_type(value)?,
        })
    }

    fn materialize_method_receiver(
        &mut self,
        receiver: ReceiverDraft,
        capability: CallableCapability,
        coercion: Option<CheckedReceiverCoercion>,
    ) -> Result<CheckedReceiver, BodyCheckError> {
        match receiver {
            ReceiverDraft::Place { syntax, place } => {
                self.materialize_place_receiver(syntax, &place, capability, coercion)
            }
            ReceiverDraft::Value { value, ty } => {
                let preparation = match (capability, self.types.get(ty)) {
                    (CallableCapability::Owned, Some(TypeKind::Borrow { .. })) => {
                        return Err(self.rule(BodyRule::InvalidCall, self.source.block())?);
                    }
                    (CallableCapability::Owned, Some(_)) => ReceiverPreparation::Owned,
                    (
                        CallableCapability::Readonly,
                        Some(TypeKind::Borrow {
                            capability: BorrowCapability::Readonly,
                            ..
                        }),
                    ) => ReceiverPreparation::PreserveBorrow(BorrowCapability::Readonly),
                    (
                        CallableCapability::Readonly,
                        Some(TypeKind::Borrow {
                            capability: BorrowCapability::ReadWrite,
                            ..
                        }),
                    ) => ReceiverPreparation::WeakenReadwriteBorrow,
                    (
                        CallableCapability::ReadWrite,
                        Some(TypeKind::Borrow {
                            capability: BorrowCapability::ReadWrite,
                            ..
                        }),
                    ) => ReceiverPreparation::PreserveBorrow(BorrowCapability::ReadWrite),
                    (CallableCapability::ReadWrite, Some(TypeKind::Borrow { .. })) => {
                        return Err(self.rule(BodyRule::InvalidCall, self.source.block())?);
                    }
                    (CallableCapability::Readonly, Some(_)) => {
                        ReceiverPreparation::BorrowTemporary(BorrowCapability::Readonly)
                    }
                    (CallableCapability::ReadWrite, Some(_)) => {
                        ReceiverPreparation::BorrowTemporary(BorrowCapability::ReadWrite)
                    }
                    (_, None) => return Err(BodyCheckInternalError::UnknownType(ty).into()),
                };
                Ok(CheckedReceiver::new(value, preparation, coercion))
            }
        }
    }

    fn materialize_place_receiver(
        &mut self,
        syntax: NodeId,
        place: &ResolvedPlace,
        capability: CallableCapability,
        coercion: Option<CheckedReceiverCoercion>,
    ) -> Result<CheckedReceiver, BodyCheckError> {
        let borrowed = match self.types.get(place.ty) {
            Some(TypeKind::Borrow {
                capability,
                referent: _,
            }) => Some(*capability),
            Some(_) => None,
            None => return Err(BodyCheckInternalError::UnknownType(place.ty).into()),
        };
        let (operation, preparation) = match (capability, borrowed) {
            (CallableCapability::Readonly, None) => (
                CheckedOperation::Place(place.id),
                ReceiverPreparation::BorrowPlace(BorrowCapability::Readonly),
            ),
            (CallableCapability::ReadWrite, None) => {
                if !self.is_writable_place(place.id)? {
                    return Err(self.rule(BodyRule::InvalidReadWriteBorrow, syntax)?);
                }
                (
                    CheckedOperation::Place(place.id),
                    ReceiverPreparation::BorrowPlace(BorrowCapability::ReadWrite),
                )
            }
            (CallableCapability::Readonly, Some(BorrowCapability::Readonly)) => (
                CheckedOperation::Place(place.id),
                ReceiverPreparation::PreserveBorrow(BorrowCapability::Readonly),
            ),
            (CallableCapability::Readonly, Some(BorrowCapability::ReadWrite)) => (
                CheckedOperation::Place(place.id),
                ReceiverPreparation::WeakenReadwriteBorrow,
            ),
            (CallableCapability::ReadWrite, Some(BorrowCapability::ReadWrite)) => (
                CheckedOperation::Place(place.id),
                ReceiverPreparation::PreserveBorrow(BorrowCapability::ReadWrite),
            ),
            (CallableCapability::ReadWrite, Some(BorrowCapability::Readonly))
            | (CallableCapability::Owned, Some(_)) => {
                return Err(self.rule(BodyRule::InvalidCall, syntax)?);
            }
            (CallableCapability::Owned, None) => {
                if place.access != PlaceAccess::Owned {
                    return Err(self.rule(BodyRule::InvalidCall, syntax)?);
                }
                let operation = match self.classify_copyability(place.ty)? {
                    Copyability::Copy => CheckedOperation::Copy(place.id),
                    Copyability::MoveOnly => {
                        for parent in place.partial_parents.iter().rev() {
                            if let Some(drop) = self.drops.get(*parent) {
                                return Err(self.partial_move_drop(syntax, drop)?);
                            }
                        }
                        CheckedOperation::Move(place.id)
                    }
                };
                (operation, ReceiverPreparation::Owned)
            }
        };
        let value = self.add_node(syntax, place.ty, operation)?;
        Ok(CheckedReceiver::new(value, preparation, coercion))
    }

    fn receiver_borrow_capability(
        &self,
        receiver: &ReceiverDraft,
    ) -> Result<BorrowCapability, BodyCheckInternalError> {
        match self.types.get(receiver.ty()) {
            Some(TypeKind::Borrow { capability, .. }) => Ok(*capability),
            Some(_) => match receiver {
                ReceiverDraft::Place { place, .. } => Ok(if self.is_writable_place(place.id)? {
                    BorrowCapability::ReadWrite
                } else {
                    BorrowCapability::Readonly
                }),
                ReceiverDraft::Value { .. } => Ok(BorrowCapability::ReadWrite),
            },
            None => Err(BodyCheckInternalError::UnknownType(receiver.ty())),
        }
    }

    fn project_method_member(
        &mut self,
        token: nocter_syntax::SyntaxToken,
        callable: nocter_model::CallableId,
    ) -> Result<(), BodyCheckInternalError> {
        let origin = SourceOrigin::from_token(self.tree(), token)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(self.source.block()))?;
        self.projections.push(super::NodeProjection::new(
            SemanticEntity::Callable(callable),
            origin,
        ));
        Ok(())
    }

    fn record_member_interruption(
        &mut self,
        token: nocter_syntax::SyntaxToken,
        receiver: TypeId,
        available: BorrowCapability,
        owned: bool,
    ) -> Result<(), BodyCheckInternalError> {
        let origin = SourceOrigin::from_token(self.tree(), token)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(self.source.block()))?;
        self.record_member_interruption_origin(origin, receiver, available, owned);
        Ok(())
    }

    pub(super) fn record_member_interruption_origin(
        &mut self,
        origin: SourceOrigin,
        receiver: TypeId,
        available: BorrowCapability,
        owned: bool,
    ) {
        self.interruption = Some(TypedBodyInterruption::new(
            self.source.body(),
            origin,
            TypedBodyInterruptionKind::MemberSelection {
                receiver,
                available,
                owned,
            },
        ));
    }
}

fn receiver_owner(
    types: &nocter_model::TypeStore,
    ty: TypeId,
) -> Result<TypeId, BodyCheckInternalError> {
    match types.get(ty) {
        Some(TypeKind::Borrow { referent, .. }) => Ok(*referent),
        Some(_) => Ok(ty),
        None => Err(BodyCheckInternalError::UnknownType(ty)),
    }
}

fn callable_capability(capability: BorrowCapability) -> CallableCapability {
    match capability {
        BorrowCapability::Readonly => CallableCapability::Readonly,
        BorrowCapability::ReadWrite => CallableCapability::ReadWrite,
    }
}
