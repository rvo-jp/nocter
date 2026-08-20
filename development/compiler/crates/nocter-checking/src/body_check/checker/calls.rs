use nocter_declarations::{CallableKind, CallableOwner, ExportedEntity};
use nocter_model::{BodyNodeId, TypeId};
use nocter_source_index::SyntaxOrigin;
use nocter_syntax::{NodeId, NodeKind, SyntaxToken};

use super::BodyChecker;
use super::call_planning::DeclaredCallGenerics;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::syntax::{direct_identifier, direct_nodes, is_transparent_expression};
use crate::{
    CallTarget, CheckedCall, CheckedOperation, NameTarget, StaticDispatch, StaticSelection,
};

impl BodyChecker<'_, '_> {
    pub(super) fn check_call(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let syntax = call_syntax(self, node)?;
        let (reference, suffix) = match syntax {
            CallSyntax::Direct { reference, suffix } => (reference, suffix),
            CallSyntax::Member {
                callee,
                owner,
                member,
                suffix,
            } => {
                if let Some((parameter, _)) = self.literal_pack_parameter(owner)? {
                    return self
                        .check_literal_pack_method(node, parameter, member, suffix, expected);
                }
                return if member_owner_is_value(self, owner)? {
                    if owner_is_direct_call_result(self, owner)? {
                        return self.check_method_call(node, owner, member, suffix, expected);
                    }
                    match self.postfix_place(callee, nocter_model::BorrowCapability::Readonly) {
                        Ok(place) => {
                            self.check_callable_place_call(node, callee, &place, suffix, expected)
                        }
                        Err(error)
                            if matches!(
                                error.rule(),
                                Some(BodyRule::UnknownField | BodyRule::InaccessibleField)
                            ) || matches!(
                                error,
                                BodyCheckError::Internal(
                                    BodyCheckInternalError::UnsupportedSyntax(_, _)
                                )
                            ) =>
                        {
                            self.check_method_call(node, owner, member, suffix, expected)
                        }
                        Err(error) => Err(error),
                    }
                } else {
                    self.check_construction_function_call(node, owner, member, suffix, expected)
                };
            }
            CallSyntax::GenericOwner { owner, suffix } => {
                return self
                    .check_explicit_construction_function_call(node, owner, suffix, expected);
            }
        };
        let target = call_name_target(self, reference)?;
        let callable_id = match target {
            NameTarget::Exported(ExportedEntity::Callable(callable)) => {
                self.consumed_uses.insert(call_origin(self, reference)?);
                callable
            }
            NameTarget::Parameter(_) | NameTarget::Local(_) | NameTarget::Capture(_) => {
                return self.check_callable_value_call(node, reference, suffix, expected);
            }
            NameTarget::Exported(_) | NameTarget::Builtin(_) => {
                self.consumed_uses.insert(call_origin(self, reference)?);
                return Err(self.rule(BodyRule::InvalidCall, reference)?);
            }
        };
        let callable = self
            .graph
            .declarations()
            .callables()
            .get(callable_id)
            .cloned()
            .ok_or(BodyCheckInternalError::MissingCallable(callable_id))?;
        if !matches!(callable.owner(), CallableOwner::Module(_))
            || !matches!(
                callable.kind(),
                CallableKind::Function | CallableKind::Primitive
            )
            || callable.receiver().is_some()
        {
            return Err(
                BodyCheckInternalError::UnsupportedSyntax(suffix, NodeKind::CallSuffix).into(),
            );
        }

        let plan = self.plan_declared_call(
            node,
            suffix,
            callable_id,
            &callable,
            DeclaredCallGenerics::inferred(callable.generic_parameters()),
            expected,
        )?;
        let call = self.add_node(
            node,
            plan.result,
            CheckedOperation::Call(CheckedCall::new(
                CallTarget::Static(StaticSelection::new(
                    StaticDispatch::Direct(callable_id),
                    plan.generic_arguments,
                )),
                None,
                plan.arguments,
            )),
        )?;
        expected.map_or(Ok(call), |expected| {
            self.apply_expected(node, call, expected)
        })
    }
}

fn owner_is_direct_call_result(
    checker: &BodyChecker<'_, '_>,
    mut node: NodeId,
) -> Result<bool, BodyCheckInternalError> {
    while checker.kind(node).is_ok_and(is_transparent_expression) {
        let children = direct_nodes(checker.tree(), node);
        let [child] = children.as_slice() else {
            return Err(BodyCheckInternalError::InvalidSyntax(node));
        };
        node = *child;
    }
    Ok(checker.kind(node)? == NodeKind::PostfixExpression
        && crate::syntax::direct_child(checker.tree(), node, NodeKind::CallSuffix).is_some())
}

pub(super) fn member_owner_is_value(
    checker: &BodyChecker<'_, '_>,
    mut node: NodeId,
) -> Result<bool, BodyCheckInternalError> {
    loop {
        while checker.kind(node).is_ok_and(is_transparent_expression) {
            let children = direct_nodes(checker.tree(), node);
            let [child] = children.as_slice() else {
                return Err(BodyCheckInternalError::InvalidSyntax(node));
            };
            node = *child;
        }
        match checker.kind(node)? {
            NodeKind::ReferenceExpression => {
                return Ok(matches!(
                    call_name_target(checker, node)?,
                    NameTarget::Parameter(_) | NameTarget::Local(_) | NameTarget::Capture(_)
                ));
            }
            NodeKind::PostfixExpression => {
                let children = direct_nodes(checker.tree(), node);
                let [base, suffix] = children.as_slice() else {
                    return Err(BodyCheckInternalError::InvalidSyntax(node));
                };
                match checker.kind(*suffix)? {
                    NodeKind::MemberSuffix => node = *base,
                    NodeKind::CallSuffix | NodeKind::IndexSuffix => return Ok(true),
                    _ => return Err(BodyCheckInternalError::InvalidSyntax(*suffix)),
                }
            }
            NodeKind::GenericOwnerMember => return Ok(false),
            _ => return Ok(true),
        }
    }
}

pub(super) fn construction_member_syntax(
    checker: &BodyChecker<'_, '_>,
    node: NodeId,
) -> Result<Option<(NodeId, NodeId)>, BodyCheckInternalError> {
    let children = direct_nodes(checker.tree(), node);
    let [owner, member] = children.as_slice() else {
        return Ok(None);
    };
    if checker.kind(*member)? != NodeKind::MemberSuffix || member_owner_is_value(checker, *owner)? {
        return Ok(None);
    }
    Ok(Some((*owner, *member)))
}

#[derive(Clone, Copy)]
enum CallSyntax {
    Direct {
        reference: NodeId,
        suffix: NodeId,
    },
    Member {
        callee: NodeId,
        owner: NodeId,
        member: NodeId,
        suffix: NodeId,
    },
    GenericOwner {
        owner: NodeId,
        suffix: NodeId,
    },
}

fn call_syntax(
    checker: &BodyChecker<'_, '_>,
    node: NodeId,
) -> Result<CallSyntax, BodyCheckInternalError> {
    let children = direct_nodes(checker.tree(), node);
    let [callee, suffix] = children.as_slice() else {
        return Err(BodyCheckInternalError::InvalidSyntax(node));
    };
    if checker.kind(*suffix)? != NodeKind::CallSuffix {
        return Err(BodyCheckInternalError::InvalidSyntax(*suffix));
    }
    let mut callee = *callee;
    while checker.kind(callee).is_ok_and(is_transparent_expression) {
        let children = direct_nodes(checker.tree(), callee);
        let [child] = children.as_slice() else {
            break;
        };
        callee = *child;
    }
    if checker.kind(callee)? == NodeKind::ReferenceExpression {
        return Ok(CallSyntax::Direct {
            reference: callee,
            suffix: *suffix,
        });
    }
    if checker.kind(callee)? == NodeKind::GenericOwnerMember {
        return Ok(CallSyntax::GenericOwner {
            owner: callee,
            suffix: *suffix,
        });
    }
    if checker.kind(callee)? == NodeKind::PostfixExpression {
        let member_children = direct_nodes(checker.tree(), callee);
        if let [owner, member] = member_children.as_slice()
            && checker.kind(*member)? == NodeKind::MemberSuffix
        {
            return Ok(CallSyntax::Member {
                callee,
                owner: *owner,
                member: *member,
                suffix: *suffix,
            });
        }
    }
    Err(BodyCheckInternalError::UnsupportedSyntax(
        *suffix,
        NodeKind::CallSuffix,
    ))
}

pub(super) fn call_origin(
    checker: &BodyChecker<'_, '_>,
    reference: NodeId,
) -> Result<SyntaxOrigin, BodyCheckInternalError> {
    direct_identifier(checker.tree(), reference)
        .or_else(|| identifier(checker, reference))
        .map(SyntaxOrigin::Token)
        .ok_or(BodyCheckInternalError::InvalidSyntax(reference))
}

pub(super) fn call_name_target(
    checker: &BodyChecker<'_, '_>,
    reference: NodeId,
) -> Result<NameTarget, BodyCheckInternalError> {
    let origin = call_origin(checker, reference)?;
    checker
        .uses
        .get(&origin)
        .copied()
        .ok_or(BodyCheckInternalError::MissingNameUse(reference))
}

fn identifier(checker: &BodyChecker<'_, '_>, node: NodeId) -> Option<SyntaxToken> {
    let mut found = crate::syntax::identifier_tokens(checker.tree(), node).into_iter();
    let token = found.next()?;
    found.next().is_none().then_some(token)
}
