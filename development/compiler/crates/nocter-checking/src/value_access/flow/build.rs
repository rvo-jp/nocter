use super::{Action, Graph, IntegrityError, Operation, Origin, Point, Terminator};
use crate::value_access::{
    ArgumentAction, Binary, BoundaryAction, ExpressionKind, Statement, TypedBody,
};
use nocter_model::{
    AccessBlockId, AccessExpressionId, AccessFlowPointId, AccessLocalId, AccessLoopId,
    AccessStatementId, ArenaBuilder,
};

type Result<T> = std::result::Result<T, IntegrityError>;
const INVALID: IntegrityError = IntegrityError::InconsistentTypedBody;

#[derive(Debug)]
struct Draft {
    actions: Vec<Action>,
    terminator: Option<Terminator>,
}
#[derive(Clone, Debug)]
enum Frame {
    Evaluation(AccessExpressionId),
    Scope {
        block: AccessBlockId,
        locals: Box<[AccessLocalId]>,
    },
    Call {
        call: AccessExpressionId,
        prepared: usize,
    },
}
#[derive(Clone, Copy, Debug)]
struct Loop {
    id: AccessLoopId,
    header: AccessFlowPointId,
    exit: AccessFlowPointId,
    depth: usize,
}
#[derive(Clone, Copy)]
enum Output {
    Return,
    Forward(AccessExpressionId),
    Discard,
}
struct Builder<'a> {
    body: &'a TypedBody,
    program: &'a nocter_declarations::value_access::Program,
    points: ArenaBuilder<AccessFlowPointId, Draft>,
    frames: Vec<Frame>,
    loops: Vec<Loop>,
}

pub(super) fn build(query: &crate::value_access::Query) -> Result<Graph> {
    let body = query.body();
    let mut builder = Builder {
        body,
        program: query.program(),
        points: ArenaBuilder::new(),
        frames: Vec::new(),
        loops: Vec::new(),
    };
    let entry = builder.point();
    builder.block(body.root(), entry, Output::Return)?;
    let points = builder.points.finish().try_map(|_, draft| {
        Ok(Point {
            actions: draft.actions.clone().into_boxed_slice(),
            terminator: draft.terminator.ok_or(INVALID)?,
        })
    })?;
    for (_, point) in points.iter() {
        for successor in point.terminator.successors() {
            points.get(successor).ok_or(INVALID)?;
        }
    }
    Ok(Graph { entry, points })
}

impl Builder<'_> {
    fn point(&mut self) -> AccessFlowPointId {
        self.points.insert(Draft {
            actions: Vec::new(),
            terminator: None,
        })
    }
    fn action(
        &mut self,
        point: AccessFlowPointId,
        origin: Origin,
        operation: Operation,
    ) -> Result<()> {
        let draft = self.points.get_mut(point).ok_or(INVALID)?;
        if draft.terminator.is_some() {
            return Err(INVALID);
        }
        draft.actions.push(Action { origin, operation });
        Ok(())
    }
    fn terminate(&mut self, point: AccessFlowPointId, terminator: Terminator) -> Result<()> {
        let draft = self.points.get_mut(point).ok_or(INVALID)?;
        if draft.terminator.replace(terminator).is_some() {
            return Err(INVALID);
        }
        Ok(())
    }
    fn branch(
        &mut self,
        point: AccessFlowPointId,
        condition: AccessExpressionId,
        yes: AccessFlowPointId,
        no: AccessFlowPointId,
    ) -> Result<()> {
        // Snapshot the predicate before either branch performs effects.
        self.action(
            point,
            Origin::Expression(condition),
            Operation::Condition(condition),
        )?;
        let terminator = match self
            .body
            .expressions()
            .get(condition)
            .ok_or(INVALID)?
            .normal_boolean()
        {
            Some(true) => Terminator::Jump(yes),
            Some(false) => Terminator::Jump(no),
            _ => Terminator::Branch { condition, yes, no },
        };
        self.terminate(point, terminator)
    }
    fn exit_frames(
        &mut self,
        point: AccessFlowPointId,
        depth: usize,
        origin: Origin,
    ) -> Result<()> {
        for frame in self
            .frames
            .get(depth..)
            .ok_or(INVALID)?
            .to_vec()
            .into_iter()
            .rev()
        {
            let operation = match frame {
                Frame::Evaluation(value) => Operation::EndEvaluation(value),
                Frame::Scope { block, locals } => Operation::ExitScope { block, locals },
                Frame::Call { call, prepared } => Operation::CancelCall { call, prepared },
            };
            self.action(point, origin, operation)?;
        }
        Ok(())
    }
    fn scope(&self, block: AccessBlockId) -> Result<Frame> {
        let mut locals = Vec::new();
        if block == self.body.root() {
            locals.extend(self.body.inputs().ordered().map(|input| input.local));
        }
        for &id in &self.body.blocks().get(block).ok_or(INVALID)?.statements {
            if let Statement::Bind { local, .. } = self.body.statements().get(id).ok_or(INVALID)? {
                locals.push(*local);
            }
        }
        locals.reverse();
        Ok(Frame::Scope {
            block,
            locals: locals.into_boxed_slice(),
        })
    }
    fn begin(&mut self, value: AccessExpressionId, point: AccessFlowPointId) -> Result<()> {
        self.action(
            point,
            Origin::Expression(value),
            Operation::BeginEvaluation(value),
        )?;
        self.frames.push(Frame::Evaluation(value));
        Ok(())
    }
    fn end(&mut self, value: AccessExpressionId, point: AccessFlowPointId) -> Result<()> {
        if !matches!(self.frames.pop(), Some(Frame::Evaluation(id)) if id == value) {
            return Err(INVALID);
        }
        self.action(
            point,
            Origin::Expression(value),
            Operation::EndEvaluation(value),
        )
    }
    fn block(
        &mut self,
        block: AccessBlockId,
        entry: AccessFlowPointId,
        output: Output,
    ) -> Result<Option<AccessFlowPointId>> {
        let depth = self.frames.len();
        self.frames.push(self.scope(block)?);
        let body = self.body.blocks().get(block).ok_or(INVALID)?;
        let mut current = Some(entry);
        for &statement in &body.statements {
            let Some(point) = current else {
                break;
            };
            current = self.statement(statement, point)?;
        }
        if let Some(point) = current {
            if let Some(result) = body.result {
                self.begin(result, point)?;
                current = self.expression(result, point)?;
                if let Some(point) = current {
                    let origin = Origin::Block(block);
                    match output {
                        Output::Return => {
                            self.action(
                                point,
                                Origin::Expression(result),
                                Operation::StageReturn(result),
                            )?;
                            self.exit_frames(point, 0, origin)?;
                            self.terminate(point, Terminator::Return)?;
                            current = None;
                        }
                        Output::Forward(destination) => {
                            self.action(
                                point,
                                Origin::Expression(result),
                                Operation::Forward {
                                    source: result,
                                    result: destination,
                                    scope_exit: true,
                                },
                            )?;
                            self.end(result, point)?;
                            self.exit_frames(point, depth, origin)?;
                        }
                        Output::Discard => {
                            self.action(
                                point,
                                Origin::Expression(result),
                                Operation::Discard(result),
                            )?;
                            self.end(result, point)?;
                            self.exit_frames(point, depth, origin)?;
                        }
                    }
                }
            } else {
                // A typed block without a normal result cannot create a continuing edge.
                self.terminate(point, Terminator::Unreachable)?;
                current = None;
            }
        }
        self.frames.truncate(depth);
        Ok(current)
    }
    fn statement(
        &mut self,
        id: AccessStatementId,
        point: AccessFlowPointId,
    ) -> Result<Option<AccessFlowPointId>> {
        let origin = Origin::Statement(id);
        let statement = self.body.statements().get(id).ok_or(INVALID)?;
        let (value, continuation) = match *statement {
            Statement::Bind { local, value } => (value, Operation::Bind { local, value }),
            Statement::Assign { target, value } => (value, Operation::Assign { target, value }),
            Statement::CompoundAssign { target, value, .. } => (
                value,
                Operation::CompoundAssign {
                    statement: id,
                    target,
                    value,
                },
            ),
            Statement::Discard(value) | Statement::Evaluate(value) => {
                (value, Operation::Discard(value))
            }
            Statement::Return(value) => {
                let depth = self.frames.len();
                self.begin(value, point)?;
                if let Some(point) = self.expression(value, point)? {
                    self.action(point, origin, Operation::StageReturn(value))?;
                    self.exit_frames(point, 0, origin)?;
                    self.terminate(point, Terminator::Return)?;
                }
                self.frames.truncate(depth);
                return Ok(None);
            }
            Statement::Drop(local) => {
                self.action(point, origin, Operation::Drop(local))?;
                return Ok(Some(point));
            }
            Statement::Break(id) | Statement::Continue(id) => {
                let target = *self
                    .loops
                    .iter()
                    .rev()
                    .find(|target| target.id == id)
                    .ok_or(INVALID)?;
                self.exit_frames(point, target.depth, origin)?;
                self.terminate(
                    point,
                    Terminator::Jump(if matches!(statement, Statement::Break(_)) {
                        target.exit
                    } else {
                        target.header
                    }),
                )?;
                return Ok(None);
            }
            Statement::Loop { loop_id, body } => return self.loop_(loop_id, None, body, point),
            Statement::While {
                loop_id,
                condition,
                body,
            } => return self.loop_(loop_id, Some(condition), body, point),
        };
        let depth = self.frames.len();
        self.begin(value, point)?;
        let Some(point) = self.expression(value, point)? else {
            self.frames.truncate(depth);
            return Ok(None);
        };
        let target = match &continuation {
            Operation::Assign { target, .. } | Operation::CompoundAssign { target, .. } => {
                Some(*target)
            }
            _ => None,
        };
        let point = if let Some(crate::value_access::Referent::Evaluated(target)) = target {
            let Some(point) = self.expression(target, point)? else {
                self.frames.truncate(depth);
                return Ok(None);
            };
            point
        } else {
            point
        };
        self.action(point, origin, continuation)?;
        self.end(value, point)?;
        Ok(Some(point))
    }
    fn loop_(
        &mut self,
        id: AccessLoopId,
        condition: Option<AccessExpressionId>,
        body: AccessBlockId,
        entry: AccessFlowPointId,
    ) -> Result<Option<AccessFlowPointId>> {
        let header = self.point();
        let exit = self.point();
        self.terminate(entry, Terminator::Jump(header))?;
        // Control inside the condition retains its typed target ID, including an outer loop.
        self.loops.push(Loop {
            id,
            header,
            exit,
            depth: self.frames.len(),
        });
        let body_entry = if let Some(condition) = condition {
            let depth = self.frames.len();
            self.begin(condition, header)?;
            if let Some(end) = self.expression(condition, header)? {
                let body_entry = self.point();
                self.action(
                    end,
                    Origin::Expression(condition),
                    Operation::Condition(condition),
                )?;
                self.end(condition, end)?;
                // branch() would snapshot the condition again after temporary cleanup.
                let terminator = match self.body.expressions().get(condition).ok_or(INVALID)?.kind {
                    ExpressionKind::Boolean(true) => Terminator::Jump(body_entry),
                    ExpressionKind::Boolean(false) => Terminator::Jump(exit),
                    _ => Terminator::Branch {
                        condition,
                        yes: body_entry,
                        no: exit,
                    },
                };
                self.terminate(end, terminator)?;
                Some(body_entry)
            } else {
                self.frames.truncate(depth);
                None
            }
        } else {
            Some(header)
        };
        if let Some(body_entry) = body_entry
            && let Some(end) = self.block(body, body_entry, Output::Discard)?
        {
            self.terminate(end, Terminator::Jump(header))?;
        }
        self.loops.pop();
        Ok(Some(exit))
    }
    fn expression(
        &mut self,
        id: AccessExpressionId,
        entry: AccessFlowPointId,
    ) -> Result<Option<AccessFlowPointId>> {
        let expression = self.body.expressions().get(id).ok_or(INVALID)?;
        let origin = Origin::Expression(id);
        let end = match &expression.kind {
            ExpressionKind::Scoped { source, projected } => {
                let Some(point) = self.expression(*source, entry)? else {
                    return Ok(None);
                };
                let end = self.expression(*projected, point)?;
                if let Some(point) = end {
                    self.action(
                        point,
                        origin,
                        Operation::Forward {
                            source: *projected,
                            result: id,
                            scope_exit: false,
                        },
                    )?;
                }
                return Ok(end);
            }
            ExpressionKind::Block { block } => {
                return self.block(*block, entry, Output::Forward(id));
            }
            ExpressionKind::If {
                condition,
                then_block,
                else_block,
            } => {
                let Some(end) = self.expression(*condition, entry)? else {
                    return Ok(None);
                };
                let yes = self.point();
                let no = self.point();
                let join = self.point();
                self.branch(end, *condition, yes, no)?;
                let left = self.block(*then_block, yes, Output::Forward(id))?;
                let right = self.block(*else_block, no, Output::Forward(id))?;
                for branch in [left, right].into_iter().flatten() {
                    self.terminate(branch, Terminator::Jump(join))?;
                }
                if left.is_none() && right.is_none() {
                    self.terminate(join, Terminator::Unreachable)?;
                    return Ok(None);
                }
                return Ok(Some(join));
            }
            ExpressionKind::Binary {
                operation: Binary::And | Binary::Or,
                ..
            } => return self.logical(id, entry),
            ExpressionKind::Await { .. } => return self.drive_future(id, entry),
            ExpressionKind::Call {
                target, arguments, ..
            } => {
                return self.call(id, entry, *target, arguments);
            }
            ExpressionKind::Aggregate { fields } => self.record(entry, fields)?,
            ExpressionKind::Closure { capture, .. } => self.expression(*capture, entry)?,
            ExpressionKind::Repeat { seed } => self.expression(*seed, entry)?,
            ExpressionKind::RequirePrimary { source }
            | ExpressionKind::AdaptFunction { source } => self.expression(*source, entry)?,
            ExpressionKind::Select { .. } | ExpressionKind::Index { .. } => {
                return self.selection_chain(id, entry);
            }
            ExpressionKind::Boundary { source, .. } => {
                // A selected place boundary uses the address directly. Do not invent an I child
                // before delegation, or perform an early read for a later reserved call input.
                if matches!(
                    self.body.expressions().get(*source).ok_or(INVALID)?.kind,
                    ExpressionKind::Place(_)
                ) {
                    Some(entry)
                } else {
                    self.expression(*source, entry)?
                }
            }
            ExpressionKind::Unary { operand, .. } => self.operand(id, 0, *operand, entry)?,
            ExpressionKind::Binary { left, right, .. } => {
                let Some(point) = self.operand(id, 0, *left, entry)? else {
                    return Ok(None);
                };
                self.operand(id, 1, *right, point)?
            }
            ExpressionKind::Move(source) => match source {
                crate::value_access::Referent::Place(_) => Some(entry),
                crate::value_access::Referent::Evaluated(_) => {
                    return self.selection_chain(id, entry);
                }
            },
            ExpressionKind::Function(_)
            | ExpressionKind::Constant(_)
            | ExpressionKind::Integer(_)
            | ExpressionKind::Boolean(_)
            | ExpressionKind::Unit
            | ExpressionKind::Place(_)
            | ExpressionKind::GuardView { .. }
            | ExpressionKind::PrimaryCondition { .. } => Some(entry),
        };
        end.map(|point| self.complete_expression(id, point))
            .transpose()
    }
    fn complete_expression(
        &mut self,
        id: AccessExpressionId,
        point: AccessFlowPointId,
    ) -> Result<AccessFlowPointId> {
        self.action(point, Origin::Expression(id), Operation::Evaluate(id))?;
        if matches!(
            self.body.expressions().get(id).ok_or(INVALID)?.kind,
            ExpressionKind::RequirePrimary { .. } | ExpressionKind::GuardView { .. }
        ) {
            // The check may terminate without unwinding. Keep its continuation distinct so
            // later domain refinement never removes/reorders the original check or effects.
            let continuation = self.point();
            self.terminate(point, Terminator::Jump(continuation))?;
            Ok(continuation)
        } else {
            Ok(point)
        }
    }
    fn selection_chain(
        &mut self,
        mut id: AccessExpressionId,
        entry: AccessFlowPointId,
    ) -> Result<Option<AccessFlowPointId>> {
        let mut chain = Vec::new();
        loop {
            match self.body.expressions().get(id).ok_or(INVALID)?.kind {
                ExpressionKind::Index { base, index, .. } => {
                    chain.push((id, Some(index)));
                    id = base;
                }
                ExpressionKind::Select { source, .. }
                | ExpressionKind::Move(crate::value_access::Referent::Evaluated(source)) => {
                    chain.push((id, None));
                    id = source;
                }
                _ => break,
            }
        }
        let mut point = self.expression(id, entry)?;
        for (id, index) in chain.into_iter().rev() {
            let Some(current) = point else { break };
            if let Some(index) = index {
                point = self.expression(index, current)?;
            }
            if let Some(current) = point {
                if matches!(
                    self.body.expressions().get(id).ok_or(INVALID)?.kind,
                    ExpressionKind::Index {
                        bounds: nocter_model::IndexBoundsCheck::ProvenTrap,
                        ..
                    }
                ) {
                    self.action(current, Origin::Expression(id), Operation::IndexFailure(id))?;
                    self.terminate(current, Terminator::Unreachable)?;
                    return Ok(None);
                }
                self.action(current, Origin::Expression(id), Operation::Evaluate(id))?;
            }
        }
        Ok(point)
    }
    fn operand(
        &mut self,
        expression: AccessExpressionId,
        operand: usize,
        value: AccessExpressionId,
        entry: AccessFlowPointId,
    ) -> Result<Option<AccessFlowPointId>> {
        let end = self.expression(value, entry)?;
        if let Some(point) = end {
            self.action(
                point,
                Origin::Expression(expression),
                Operation::PrepareOperand {
                    expression,
                    operand,
                    value,
                },
            )?;
        }
        Ok(end)
    }
    fn record(
        &mut self,
        entry: AccessFlowPointId,
        fields: &[crate::value_access::FieldValue],
    ) -> Result<Option<AccessFlowPointId>> {
        let mut current = Some(entry);
        for field in fields {
            let Some(point) = current else {
                break;
            };
            current = self.expression(field.value, point)?;
        }
        Ok(current)
    }
    fn drive_future(
        &mut self,
        id: AccessExpressionId,
        entry: AccessFlowPointId,
    ) -> Result<Option<AccessFlowPointId>> {
        let mut current = id;
        let mut drives = Vec::new();
        while let ExpressionKind::Await { future } =
            self.body.expressions().get(current).ok_or(INVALID)?.kind
        {
            drives.push((current, future));
            current = future;
        }
        let Some(point) = self.expression(current, entry)? else {
            return Ok(None);
        };
        for (result, future) in drives.into_iter().rev() {
            self.action(
                point,
                Origin::Expression(result),
                Operation::DriveFuture { future, result },
            )?;
        }
        Ok(Some(point))
    }
    fn call(
        &mut self,
        id: AccessExpressionId,
        entry: AccessFlowPointId,
        target: nocter_model::ValueFunctionOrigin,
        arguments: &crate::value_access::CallArguments,
    ) -> Result<Option<AccessFlowPointId>> {
        let origin = Origin::Expression(id);
        self.frames.push(Frame::Call {
            call: id,
            prepared: 0,
        });
        let mut current = Some(entry);
        for (index, argument) in arguments.preparation_order() {
            let Some(point) = current else {
                break;
            };
            current = match argument.action {
                ArgumentAction::ReserveRead(source)
                | ArgumentAction::ReserveUpdate(source)
                | ArgumentAction::ReserveTransfer { place: source, .. }
                | ArgumentAction::CopyPlace(source)
                | ArgumentAction::PendingValue(source) => match source {
                    crate::value_access::Referent::Place(_) => Some(point),
                    crate::value_access::Referent::Evaluated(id) => self.expression(id, point)?,
                },
                ArgumentAction::Unproduced | ArgumentAction::ReuseRead { .. } => Some(point),
                _ => self.expression(argument.value, point)?,
            };
            if let Some(point) = current {
                self.action(
                    point,
                    Origin::Expression(argument.value),
                    Operation::PrepareArgument {
                        call: id,
                        argument: index,
                    },
                )?;
                let Some(Frame::Call { prepared, .. }) = self.frames.last_mut() else {
                    return Err(INVALID);
                };
                *prepared += 1;
            }
        }
        self.frames.pop();
        if let Some(point) = current {
            let operation = match self.program.function_execution(target) {
                Some(nocter_declarations::CallableExecution::Deferred { .. }) => {
                    Operation::ConstructFuture(id)
                }
                _ => Operation::CommitCall(id),
            };
            self.action(point, origin, operation)?;
        }
        Ok(current)
    }
    /// Intermediate owned bool values have no observable identity or cleanup. Keep a same-
    /// operator chain in one control region instead of joining false/true prefixes and then
    /// branching on an already decided value again. Leaf evaluation and its frame stay intact.
    fn logical_terms(
        &self,
        id: AccessExpressionId,
        operation: Binary,
    ) -> Result<Vec<AccessExpressionId>> {
        let mut pending = vec![id];
        let mut terms = Vec::new();
        while let Some(original) = pending.pop() {
            let mut current = original;
            while let ExpressionKind::Boundary {
                source,
                action: BoundaryAction::TakeValue,
            } = self.body.expressions().get(current).ok_or(INVALID)?.kind
            {
                current = source;
            }
            match self.body.expressions().get(current).ok_or(INVALID)?.kind {
                ExpressionKind::Binary {
                    operation: actual,
                    left,
                    right,
                } if actual == operation => {
                    pending.push(right);
                    pending.push(left);
                }
                _ => terms.push(original),
            }
        }
        Ok(terms)
    }
    fn logical(
        &mut self,
        id: AccessExpressionId,
        entry: AccessFlowPointId,
    ) -> Result<Option<AccessFlowPointId>> {
        let ExpressionKind::Binary {
            operation: operation @ (Binary::And | Binary::Or),
            ..
        } = self.body.expressions().get(id).ok_or(INVALID)?.kind
        else {
            return Err(INVALID);
        };
        let mut terms = self.logical_terms(id, operation)?.into_iter();
        let mut source = terms.next().ok_or(INVALID)?;
        let Some(end) = self.expression(source, entry)? else {
            return Ok(None);
        };
        let short = self.point();
        let join = self.point();
        let conjunction = operation == Binary::And;
        let mut current = Some(end);
        for term in terms {
            let Some(end) = current else { break };
            let long = self.point();
            self.branch(
                end,
                source,
                if conjunction { long } else { short },
                if conjunction { short } else { long },
            )?;
            source = term;
            current = self.expression(term, long)?;
        }
        self.action(
            short,
            Origin::Expression(id),
            Operation::BooleanConstant {
                result: id,
                value: !conjunction,
            },
        )?;
        self.terminate(short, Terminator::Jump(join))?;
        if let Some(end) = current {
            self.action(
                end,
                Origin::Expression(id),
                Operation::BooleanAnswer { source, result: id },
            )?;
            self.terminate(end, Terminator::Jump(join))?;
        }
        Ok(Some(join))
    }
}
