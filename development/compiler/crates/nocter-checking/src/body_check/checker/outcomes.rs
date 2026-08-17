use nocter_model::{BodyNodeId, BuiltinType, TypeId, TypeKind};
use nocter_source_index::SyntaxOrigin;
use nocter_syntax::{Keyword, NodeId, NodeKind, Punctuation, SyntaxElement, TokenKind};

use super::{BlockExpectation, BodyChecker};
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::syntax::{direct_child, direct_identifier, direct_nodes};
use crate::{
    CheckedOperation, CheckedOutcome, ExpectedBase, ExpectedEvidence, LocalBindingKind,
    OutcomeLayer, plan_expected_type,
};

impl BodyChecker<'_, '_> {
    pub(super) fn check_outcome_expression(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let (operand, punctuation) = match self.kind(node)? {
            NodeKind::MoveExpression => (
                self.check_move_place(node)?,
                outcome_punctuation(self, node)?,
            ),
            NodeKind::OutcomeExpression => {
                let children = direct_nodes(self.tree(), node);
                let [operand] = children.as_slice() else {
                    return Err(BodyCheckInternalError::InvalidSyntax(node).into());
                };
                (
                    self.check_expression(*operand, None)?,
                    outcome_punctuation(self, node)?,
                )
            }
            _ => return Err(BodyCheckInternalError::InvalidSyntax(node).into()),
        };
        let operand_type = self.node_type(operand)?;
        let Some((layer, payload)) = outcome_layer(self.types, operand_type) else {
            return Err(self.rule(BodyRule::InvalidOutcomeOperation, node)?);
        };
        let operation = match punctuation {
            Punctuation::Question => {
                let outer = if self.closure_result_inference.is_some() {
                    Box::new([])
                } else {
                    let evidence = match layer {
                        OutcomeLayer::Optional => ExpectedEvidence::Absent,
                        OutcomeLayer::Fallible => ExpectedEvidence::Failure,
                    };
                    let Ok(plan) = plan_expected_type(self.types, self.result_type, evidence)
                    else {
                        return Err(self.rule(BodyRule::InvalidOutcomeOperation, node)?);
                    };
                    let (base, outer) = plan.into_parts();
                    if !matches!(
                        (layer, base),
                        (OutcomeLayer::Optional, ExpectedBase::Absent(_))
                            | (OutcomeLayer::Fallible, ExpectedBase::Failure(_))
                    ) {
                        return Err(BodyCheckInternalError::InvalidSyntax(node).into());
                    }
                    outer
                };
                CheckedOutcome::Propagate {
                    operand,
                    layer,
                    outer,
                }
            }
            Punctuation::Bang => CheckedOutcome::Force { operand, layer },
            _ => return Err(BodyCheckInternalError::InvalidSyntax(node).into()),
        };
        let checked = self.add_node(node, payload, CheckedOperation::Outcome(operation))?;
        if punctuation == Punctuation::Question && self.closure_result_inference.is_some() {
            self.record_inferred_closure_propagation(node, checked, layer)?;
        }
        expected.map_or(Ok(checked), |expected| {
            self.apply_expected(node, checked, expected)
        })
    }

    pub(super) fn check_recovery_expression(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let children = direct_nodes(self.tree(), node);
        let [operand_syntax, clause] = children.as_slice() else {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        };
        if self.kind(*clause)? != NodeKind::RecoveryClause {
            return Err(BodyCheckInternalError::InvalidSyntax(*clause).into());
        }
        let operand = self.check_expression(*operand_syntax, None)?;
        let operand_type = self.node_type(operand)?;
        let Some((layer, payload)) = outcome_layer(self.types, operand_type) else {
            return Err(self.rule(BodyRule::InvalidOutcomeOperation, *clause)?);
        };
        let catch = self.tree().children(*clause).iter().any(|element| {
            matches!(
                element,
                SyntaxElement::Token(token)
                    if token.kind() == TokenKind::Keyword(Keyword::Catch)
            )
        });
        if catch != (layer == OutcomeLayer::Fallible) {
            return Err(self.rule(BodyRule::InvalidOutcomeOperation, *clause)?);
        }
        let binding = if catch {
            self.define_catch_binding(*clause)?
        } else {
            None
        };
        let block = direct_child(self.tree(), *clause, NodeKind::Block)
            .ok_or(BodyCheckInternalError::InvalidSyntax(*clause))?;
        let fallback = self.check_block(block, BlockExpectation::Value(Some(payload)))?;
        let recovered = self.add_node(
            node,
            payload,
            CheckedOperation::Outcome(CheckedOutcome::Recover {
                operand,
                layer,
                binding,
                fallback,
            }),
        )?;
        expected.map_or(Ok(recovered), |expected| {
            self.apply_expected(node, recovered, expected)
        })
    }

    fn define_catch_binding(
        &mut self,
        clause: NodeId,
    ) -> Result<Option<nocter_model::LocalBindingId>, BodyCheckError> {
        let token = direct_identifier(self.tree(), clause)
            .ok_or(BodyCheckInternalError::InvalidSyntax(clause))?;
        if self.token_text(token)? == "_" {
            return Ok(None);
        }
        let local = self
            .local_declarations
            .get(&SyntaxOrigin::Token(token))
            .copied()
            .ok_or(BodyCheckInternalError::MissingLocalDeclaration(clause))?;
        if self
            .names
            .locals()
            .get(local)
            .is_none_or(|declaration| declaration.kind() != LocalBindingKind::Catch)
        {
            return Err(BodyCheckInternalError::MissingLocalDeclaration(clause).into());
        }
        self.builder
            .define_local(local, self.types.builtin(BuiltinType::Error))?;
        Ok(Some(local))
    }
}

fn outcome_layer(types: &nocter_model::TypeStore, ty: TypeId) -> Option<(OutcomeLayer, TypeId)> {
    match types.get(ty)? {
        TypeKind::Optional(payload) => Some((OutcomeLayer::Optional, *payload)),
        TypeKind::Fallible(payload) => Some((OutcomeLayer::Fallible, *payload)),
        _ => None,
    }
}

fn outcome_punctuation(
    checker: &BodyChecker<'_, '_>,
    node: NodeId,
) -> Result<Punctuation, BodyCheckInternalError> {
    checker
        .tree()
        .children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) => match token.kind() {
                TokenKind::Punctuation(Punctuation::Question | Punctuation::Bang) => {
                    let TokenKind::Punctuation(punctuation) = token.kind() else {
                        unreachable!()
                    };
                    Some(punctuation)
                }
                _ => None,
            },
            SyntaxElement::Node(_) | SyntaxElement::Missing(_) => None,
        })
        .ok_or(BodyCheckInternalError::InvalidSyntax(node))
}
