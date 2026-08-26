use nocter_model::{BodyNodeId, BuiltinType, LoopId};
use nocter_syntax::SyntaxOrigin;
use nocter_syntax::{NodeId, NodeKind};

use super::{BlockExpectation, BodyChecker};
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::body_check::literal::is_integer_type;
use crate::syntax::{direct_children, direct_identifier};
use crate::{CheckedControl, CheckedLoop, CheckedOperation, LoopKind};

pub(super) struct LoopConstruction {
    pub(super) id: LoopId,
    pub(super) has_break: bool,
}

impl BodyChecker<'_, '_> {
    pub(super) fn check_loop(&mut self, statement: NodeId) -> Result<BodyNodeId, BodyCheckError> {
        let loop_ = self.builder.reserve_loop();
        self.loops.push(LoopConstruction {
            id: loop_,
            has_break: false,
        });
        let kind = match self.kind(statement)? {
            NodeKind::WhileStatement => {
                let condition = self.required_child(statement, NodeKind::Expression)?;
                let condition =
                    self.check_expression(condition, Some(self.types.builtin(BuiltinType::Bool)))?;
                LoopKind::While { condition }
            }
            NodeKind::LoopStatement => LoopKind::Infinite,
            NodeKind::ForStatement => self.check_for_loop(statement)?,
            kind => return Err(BodyCheckInternalError::UnsupportedSyntax(statement, kind).into()),
        };
        let block = self.required_child(statement, NodeKind::Block)?;
        let body_scope = self
            .names
            .block_scope(block)
            .ok_or(BodyCheckInternalError::MissingBlockScope(block))?;
        let body = self.check_block(
            block,
            BlockExpectation::Value(Some(self.types.builtin(BuiltinType::Void))),
        )?;
        let frame = self.loops.pop().ok_or(BodyCheckInternalError::LoopStack)?;
        if frame.id != loop_ {
            return Err(BodyCheckInternalError::LoopStack.into());
        }
        let ty = match &kind {
            LoopKind::Infinite if !frame.has_break => self.types.builtin(BuiltinType::Never),
            LoopKind::Infinite
            | LoopKind::While { .. }
            | LoopKind::Range { .. }
            | LoopKind::For { .. }
            | LoopKind::ArgumentPack { .. } => self.types.builtin(BuiltinType::Void),
        };
        self.builder
            .define_loop(loop_, CheckedLoop::new(kind, body, body_scope))?;
        self.add_node(
            statement,
            ty,
            CheckedOperation::Control(CheckedControl::Loop(loop_)),
        )
    }

    fn check_for_loop(&mut self, statement: NodeId) -> Result<LoopKind, BodyCheckError> {
        let source = self.required_child(statement, NodeKind::ForSource)?;
        let expressions = direct_children(self.tree(), source, NodeKind::Expression);
        match expressions.as_slice() {
            [collection] => self.check_collection_loop(statement, *collection),
            [start, end] => self.check_range_loop(statement, *start, *end),
            _ => Err(BodyCheckInternalError::UnsupportedSyntax(source, NodeKind::ForSource).into()),
        }
    }

    fn check_collection_loop(
        &mut self,
        statement: NodeId,
        source: NodeId,
    ) -> Result<LoopKind, BodyCheckError> {
        if let Some((parameter, item)) = self.argument_pack_parameter(source)? {
            self.register_argument_pack_iteration(parameter, source)?;
            let binding = self.loop_binding(statement)?;
            self.builder.define_local(binding, item)?;
            return Ok(LoopKind::ArgumentPack {
                binding,
                parameter,
                item,
            });
        }
        let iteration = self.check_collection_iteration(statement, source)?;
        let binding = self.loop_binding(statement)?;
        self.builder.define_local(binding, iteration.item())?;
        Ok(LoopKind::For { binding, iteration })
    }

    fn check_range_loop(
        &mut self,
        statement: NodeId,
        start: NodeId,
        end: NodeId,
    ) -> Result<LoopKind, BodyCheckError> {
        let start_syntax = start;
        let start = self.check_expression(start_syntax, None)?;
        let ty = self.node_type(start)?;
        if !is_integer_type(self.types, ty) {
            return Err(self.rule(BodyRule::TypeMismatch, start_syntax)?);
        }
        let end = self.check_expression(end, Some(ty))?;
        let binding = self.loop_binding(statement)?;
        self.builder.define_local(binding, ty)?;
        Ok(LoopKind::Range {
            binding,
            start,
            end,
        })
    }

    fn loop_binding(
        &self,
        statement: NodeId,
    ) -> Result<nocter_model::LocalBindingId, BodyCheckInternalError> {
        let token = direct_identifier(self.tree(), statement)
            .ok_or(BodyCheckInternalError::InvalidSyntax(statement))?;
        self.local_declarations
            .get(&SyntaxOrigin::Token(token))
            .copied()
            .ok_or(BodyCheckInternalError::MissingLocalDeclaration(statement))
    }

    pub(super) fn check_loop_control(
        &mut self,
        statement: NodeId,
        is_break: bool,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let Some(frame) = self.loops.last_mut() else {
            return Err(self.rule(BodyRule::InvalidLoopControl, statement)?);
        };
        if is_break && self.flow_reachable {
            frame.has_break = true;
        }
        let control = if is_break {
            CheckedControl::Break(frame.id)
        } else {
            CheckedControl::Continue(frame.id)
        };
        self.add_node(
            statement,
            self.types.builtin(BuiltinType::Never),
            CheckedOperation::Control(control),
        )
    }
}
