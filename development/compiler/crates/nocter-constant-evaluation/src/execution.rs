use std::collections::HashMap;
use std::sync::Arc;

use nocter_model::{
    Arena, BodyId, BodyNodeId, CompilationTarget, ConstantId, ConstantValue, LocalBindingId,
    ParameterId,
};

use crate::scalar::{self, ScalarEvaluationFailure};
use crate::{
    CompileTimeCallTarget, CompileTimeCallablePlan, CompileTimeEvaluationLimits,
    CompileTimeLogicalOperation, CompileTimeOperation, CompileTimePlanTable, CompileTimeValue,
    CompileTimeValueType, ConstantScalarType,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileTimeExecutionRule {
    MissingPlan,
    MissingConstant,
    ArgumentMismatch,
    TypeMismatch,
    ArithmeticFailure,
    StepLimit,
    CallDepthLimit,
    ReachedUnreachable,
    InvalidPlan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileTimeExecutionSubject {
    Call(CompileTimeCallTarget),
    Initializer(BodyId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileTimeExecutionError {
    rule: CompileTimeExecutionRule,
    subject: CompileTimeExecutionSubject,
    node: Option<BodyNodeId>,
}

impl CompileTimeExecutionError {
    #[must_use]
    pub const fn rule(&self) -> CompileTimeExecutionRule {
        self.rule
    }

    #[must_use]
    pub const fn subject(&self) -> &CompileTimeExecutionSubject {
        &self.subject
    }

    #[must_use]
    pub const fn node(&self) -> Option<BodyNodeId> {
        self.node
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct CallKey {
    target: CompileTimeCallTarget,
    arguments: Box<[CompileTimeValue]>,
}

/// Immutable constant lookup used by checked-plan execution.
///
/// Execution depends only on this semantic contract. A complete program can provide its dense
/// value arena, while a dependency query can expose only values that it has already completed.
pub trait CompileTimeConstantResolver {
    #[must_use]
    fn resolve_constant(&self, id: ConstantId) -> Option<&ConstantValue>;
}

impl CompileTimeConstantResolver for Arena<ConstantId, ConstantValue> {
    fn resolve_constant(&self, id: ConstantId) -> Option<&ConstantValue> {
        self.get(id)
    }
}

/// Evaluates closed callable plans with one deterministic budget and call-result cache.
pub struct CompileTimeExecutor<'program> {
    plans: &'program CompileTimePlanTable,
    constants: &'program dyn CompileTimeConstantResolver,
    target: CompilationTarget,
    remaining_steps: u64,
    maximum_call_depth: u32,
    completed_calls: HashMap<CallKey, Arc<CompileTimeValue>>,
}

impl<'program> CompileTimeExecutor<'program> {
    #[must_use]
    pub fn new(
        plans: &'program CompileTimePlanTable,
        constants: &'program dyn CompileTimeConstantResolver,
        target: CompilationTarget,
        limits: CompileTimeEvaluationLimits,
    ) -> Self {
        Self {
            plans,
            constants,
            target,
            remaining_steps: limits.steps().get(),
            maximum_call_depth: limits.call_depth().get(),
            completed_calls: HashMap::new(),
        }
    }

    /// Executes one closed call and memoizes its completed result for later identical calls.
    ///
    /// # Errors
    ///
    /// Returns a source-neutral target and operation identity for deterministic execution,
    /// resource-limit, or plan-integrity failures.
    pub fn evaluate(
        &mut self,
        target: &CompileTimeCallTarget,
        arguments: impl Into<Box<[CompileTimeValue]>>,
    ) -> Result<Arc<CompileTimeValue>, CompileTimeExecutionError> {
        self.evaluate_call(target.clone(), arguments.into(), 1)
    }

    /// Executes one already-closed declaration initializer plan.
    ///
    /// Nested calls use the same callable table, cache, and deterministic budget as ordinary
    /// compile-time calls.
    ///
    /// # Errors
    ///
    /// Returns a source-neutral initializer identity and operation identity for deterministic
    /// execution, resource-limit, or plan-integrity failures.
    pub fn evaluate_initializer(
        &mut self,
        body: BodyId,
        plan: &CompileTimeCallablePlan,
    ) -> Result<Arc<CompileTimeValue>, CompileTimeExecutionError> {
        self.evaluate_plan(CompileTimeExecutionSubject::Initializer(body), plan, &[], 1)
    }

    fn evaluate_call(
        &mut self,
        target: CompileTimeCallTarget,
        arguments: Box<[CompileTimeValue]>,
        depth: u32,
    ) -> Result<Arc<CompileTimeValue>, CompileTimeExecutionError> {
        let key = CallKey {
            target: target.clone(),
            arguments,
        };
        if let Some(value) = self.completed_calls.get(&key) {
            return Ok(Arc::clone(value));
        }
        if depth > self.maximum_call_depth {
            return Err(error(
                CompileTimeExecutionRule::CallDepthLimit,
                CompileTimeExecutionSubject::Call(target),
                None,
            ));
        }
        let plan = self.plans.get(&target).ok_or_else(|| {
            error(
                CompileTimeExecutionRule::MissingPlan,
                CompileTimeExecutionSubject::Call(target.clone()),
                None,
            )
        })?;
        let value = self.evaluate_plan(
            CompileTimeExecutionSubject::Call(target.clone()),
            plan,
            &key.arguments,
            depth,
        )?;
        self.completed_calls.insert(key, Arc::clone(&value));
        Ok(value)
    }

    fn evaluate_plan(
        &mut self,
        subject: CompileTimeExecutionSubject,
        plan: &CompileTimeCallablePlan,
        arguments: &[CompileTimeValue],
        depth: u32,
    ) -> Result<Arc<CompileTimeValue>, CompileTimeExecutionError> {
        let parameters =
            bind_parameters(plan, arguments).map_err(|rule| error(rule, subject.clone(), None))?;
        let mut frame = Frame {
            subject: subject.clone(),
            plan,
            parameters,
            locals: HashMap::new(),
        };
        let value = match self.evaluate_node(&mut frame, plan.root(), depth) {
            Ok(value) | Err(EvaluationInterrupt::Return(value)) => value,
            Err(EvaluationInterrupt::Unreachable) => {
                return Err(error(
                    CompileTimeExecutionRule::ReachedUnreachable,
                    subject,
                    Some(plan.root()),
                ));
            }
            Err(EvaluationInterrupt::Failure(error)) => return Err(error),
        };
        let value = value
            .into_type(plan.result())
            .map_err(|rule| error(rule, subject, Some(plan.root())))?;
        Ok(Arc::new(value))
    }

    // Keeping the exhaustive operation dispatch together ensures that every new checked-plan
    // operation receives an execution decision in the same compiler error site.
    #[allow(clippy::too_many_lines)]
    fn evaluate_node(
        &mut self,
        frame: &mut Frame<'_>,
        node_id: BodyNodeId,
        depth: u32,
    ) -> EvaluationResult<CompileTimeValue> {
        if self.remaining_steps == 0 {
            return Err(frame
                .error(CompileTimeExecutionRule::StepLimit, Some(node_id))
                .into());
        }
        self.remaining_steps -= 1;
        let node = frame
            .plan
            .nodes()
            .get(node_id)
            .ok_or_else(|| frame.error(CompileTimeExecutionRule::InvalidPlan, Some(node_id)))?;
        let value = match node.operation() {
            CompileTimeOperation::Complete => CompileTimeValue::void(),
            CompileTimeOperation::Literal(value) => scalar_value(node.ty(), value.clone())
                .map_err(|rule| frame.error(rule, Some(node_id)))?,
            CompileTimeOperation::DeclaredConstant(id) => {
                let value = self
                    .constants
                    .resolve_constant(*id)
                    .cloned()
                    .ok_or_else(|| {
                        frame.error(CompileTimeExecutionRule::MissingConstant, Some(node_id))
                    })?;
                scalar_value(node.ty(), value).map_err(|rule| frame.error(rule, Some(node_id)))?
            }
            CompileTimeOperation::ReadParameter(parameter) => {
                frame.parameters.get(parameter).cloned().ok_or_else(|| {
                    frame.error(CompileTimeExecutionRule::InvalidPlan, Some(node_id))
                })?
            }
            CompileTimeOperation::ReadLocal(local) => {
                frame.locals.get(local).cloned().ok_or_else(|| {
                    frame.error(CompileTimeExecutionRule::InvalidPlan, Some(node_id))
                })?
            }
            CompileTimeOperation::Unary { operation, operand } => {
                let operand = self.evaluate_node(frame, *operand, depth)?;
                let ty = scalar_type(node.ty()).map_err(|rule| frame.error(rule, Some(node_id)))?;
                let value = scalar::unary(
                    *operation,
                    ty,
                    scalar_representation(&operand)
                        .map_err(|rule| frame.error(rule, Some(node_id)))?,
                    self.target,
                )
                .map_err(|failure| frame.error(map_scalar_failure(failure), Some(node_id)))?;
                CompileTimeValue::scalar(ty, value)
                    .map_err(|rule| frame.error(rule, Some(node_id)))?
            }
            CompileTimeOperation::Binary {
                operation,
                left,
                right,
            } => {
                let left = self.evaluate_node(frame, *left, depth)?;
                let right = self.evaluate_node(frame, *right, depth)?;
                let ty = scalar_type(node.ty()).map_err(|rule| frame.error(rule, Some(node_id)))?;
                let value = scalar::binary(
                    *operation,
                    ty,
                    scalar_representation(&left)
                        .map_err(|rule| frame.error(rule, Some(node_id)))?,
                    scalar_representation(&right)
                        .map_err(|rule| frame.error(rule, Some(node_id)))?,
                    self.target,
                )
                .map_err(|failure| frame.error(map_scalar_failure(failure), Some(node_id)))?;
                CompileTimeValue::scalar(ty, value)
                    .map_err(|rule| frame.error(rule, Some(node_id)))?
            }
            CompileTimeOperation::NumericConversion { operand, target } => {
                let operand = self.evaluate_node(frame, *operand, depth)?;
                let value = scalar::convert(
                    *target,
                    scalar_representation(&operand)
                        .map_err(|rule| frame.error(rule, Some(node_id)))?,
                    self.target,
                )
                .map_err(|failure| frame.error(map_scalar_failure(failure), Some(node_id)))?;
                CompileTimeValue::scalar(*target, value)
                    .map_err(|rule| frame.error(rule, Some(node_id)))?
            }
            CompileTimeOperation::Comparison {
                operation,
                left,
                right,
            } => {
                let left = self.evaluate_node(frame, *left, depth)?;
                let right = self.evaluate_node(frame, *right, depth)?;
                let value = scalar::compare(
                    *operation,
                    scalar_representation(&left)
                        .map_err(|rule| frame.error(rule, Some(node_id)))?,
                    scalar_representation(&right)
                        .map_err(|rule| frame.error(rule, Some(node_id)))?,
                    self.target,
                )
                .map_err(|failure| frame.error(map_scalar_failure(failure), Some(node_id)))?;
                CompileTimeValue::scalar(ConstantScalarType::Bool, value)
                    .map_err(|rule| frame.error(rule, Some(node_id)))?
            }
            CompileTimeOperation::Tuple(elements) => {
                let values = self.values(frame, elements, depth)?;
                CompileTimeValue::tuple(node.ty().clone(), values)
                    .map_err(|rule| frame.error(rule, Some(node_id)))?
            }
            CompileTimeOperation::FixedArray(elements) => {
                let values = self.values(frame, elements, depth)?;
                CompileTimeValue::fixed_array(node.ty().clone(), values)
                    .map_err(|rule| frame.error(rule, Some(node_id)))?
            }
            CompileTimeOperation::Call {
                target,
                receiver,
                arguments,
            } => {
                let mut values =
                    Vec::with_capacity(arguments.len() + usize::from(receiver.is_some()));
                if let Some(receiver) = receiver {
                    values.push(self.evaluate_node(frame, *receiver, depth)?);
                }
                values.extend(self.values(frame, arguments, depth)?);
                self.evaluate_call(target.clone(), values.into_boxed_slice(), depth + 1)
                    .map_err(EvaluationInterrupt::Failure)?
                    .as_ref()
                    .clone()
            }
            CompileTimeOperation::Block { statements, result } => {
                for statement in statements {
                    self.evaluate_node(frame, *statement, depth)?;
                }
                match result {
                    Some(result) => self.evaluate_node(frame, *result, depth)?,
                    None => CompileTimeValue::void(),
                }
            }
            CompileTimeOperation::Bind {
                binding,
                initializer,
            } => {
                let value = self.evaluate_node(frame, *initializer, depth)?;
                if let Some(binding) = binding {
                    let expected = frame.plan.locals().get(*binding).ok_or_else(|| {
                        frame.error(CompileTimeExecutionRule::InvalidPlan, Some(node_id))
                    })?;
                    if !value.matches_type(expected) {
                        return Err(frame
                            .error(CompileTimeExecutionRule::TypeMismatch, Some(node_id))
                            .into());
                    }
                    frame.locals.insert(*binding, value);
                }
                CompileTimeValue::void()
            }
            CompileTimeOperation::Discard(value) => {
                self.evaluate_node(frame, *value, depth)?;
                CompileTimeValue::void()
            }
            CompileTimeOperation::Unreachable => return Err(EvaluationInterrupt::Unreachable),
            CompileTimeOperation::Return(value) => {
                let value = match value {
                    Some(value) => self.evaluate_node(frame, *value, depth)?,
                    None => CompileTimeValue::void(),
                };
                return Err(EvaluationInterrupt::Return(value));
            }
            CompileTimeOperation::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition = self.evaluate_node(frame, *condition, depth)?;
                if bool_representation(&condition)
                    .map_err(|rule| frame.error(rule, Some(node_id)))?
                {
                    self.evaluate_node(frame, *then_branch, depth)?
                } else if let Some(else_branch) = else_branch {
                    self.evaluate_node(frame, *else_branch, depth)?
                } else {
                    CompileTimeValue::void()
                }
            }
            CompileTimeOperation::Logical {
                operation,
                left,
                right,
            } => {
                let left = self.evaluate_node(frame, *left, depth)?;
                let left =
                    bool_representation(&left).map_err(|rule| frame.error(rule, Some(node_id)))?;
                let value = match operation {
                    CompileTimeLogicalOperation::And if !left => false,
                    CompileTimeLogicalOperation::Or if left => true,
                    CompileTimeLogicalOperation::And | CompileTimeLogicalOperation::Or => {
                        let right = self.evaluate_node(frame, *right, depth)?;
                        bool_representation(&right)
                            .map_err(|rule| frame.error(rule, Some(node_id)))?
                    }
                };
                CompileTimeValue::scalar(ConstantScalarType::Bool, ConstantValue::Bool(value))
                    .map_err(|rule| frame.error(rule, Some(node_id)))?
            }
        };
        value
            .into_type(node.ty())
            .map_err(|rule| frame.error(rule, Some(node_id)).into())
    }

    fn values(
        &mut self,
        frame: &mut Frame<'_>,
        nodes: &[BodyNodeId],
        depth: u32,
    ) -> EvaluationResult<Vec<CompileTimeValue>> {
        nodes
            .iter()
            .map(|node| self.evaluate_node(frame, *node, depth))
            .collect()
    }
}

struct Frame<'plan> {
    subject: CompileTimeExecutionSubject,
    plan: &'plan CompileTimeCallablePlan,
    parameters: HashMap<ParameterId, CompileTimeValue>,
    locals: HashMap<LocalBindingId, CompileTimeValue>,
}

impl Frame<'_> {
    fn error(
        &self,
        rule: CompileTimeExecutionRule,
        node: Option<BodyNodeId>,
    ) -> CompileTimeExecutionError {
        error(rule, self.subject.clone(), node)
    }
}

enum EvaluationInterrupt {
    Return(CompileTimeValue),
    Unreachable,
    Failure(CompileTimeExecutionError),
}

impl From<CompileTimeExecutionError> for EvaluationInterrupt {
    fn from(error: CompileTimeExecutionError) -> Self {
        Self::Failure(error)
    }
}

type EvaluationResult<T> = Result<T, EvaluationInterrupt>;

fn bind_parameters(
    plan: &CompileTimeCallablePlan,
    arguments: &[CompileTimeValue],
) -> Result<HashMap<ParameterId, CompileTimeValue>, CompileTimeExecutionRule> {
    if plan.parameters().len() != arguments.len() {
        return Err(CompileTimeExecutionRule::ArgumentMismatch);
    }
    plan.parameters()
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            argument
                .matches_type(parameter.ty())
                .then(|| argument.clone().into_type(parameter.ty()))
                .ok_or(CompileTimeExecutionRule::TypeMismatch)?
                .map(|argument| (parameter.id(), argument))
        })
        .collect()
}

fn scalar_value(
    ty: &CompileTimeValueType,
    value: ConstantValue,
) -> Result<CompileTimeValue, CompileTimeExecutionRule> {
    CompileTimeValue::scalar(scalar_type(ty)?, value)
}

fn scalar_type(ty: &CompileTimeValueType) -> Result<ConstantScalarType, CompileTimeExecutionRule> {
    match ty {
        CompileTimeValueType::Scalar(ty) => Ok(*ty),
        CompileTimeValueType::ReadonlyBorrow(referent) => scalar_type(referent),
        CompileTimeValueType::Void
        | CompileTimeValueType::Never
        | CompileTimeValueType::Tuple(_)
        | CompileTimeValueType::FixedArray { .. } => Err(CompileTimeExecutionRule::TypeMismatch),
    }
}

fn scalar_representation(
    value: &CompileTimeValue,
) -> Result<&ConstantValue, CompileTimeExecutionRule> {
    value
        .scalar_value()
        .ok_or(CompileTimeExecutionRule::TypeMismatch)
}

fn bool_representation(value: &CompileTimeValue) -> Result<bool, CompileTimeExecutionRule> {
    match value.scalar_value() {
        Some(ConstantValue::Bool(value)) => Ok(*value),
        Some(_) | None => Err(CompileTimeExecutionRule::TypeMismatch),
    }
}

const fn map_scalar_failure(failure: ScalarEvaluationFailure) -> CompileTimeExecutionRule {
    match failure {
        ScalarEvaluationFailure::Arithmetic => CompileTimeExecutionRule::ArithmeticFailure,
        ScalarEvaluationFailure::InvalidType => CompileTimeExecutionRule::TypeMismatch,
    }
}

const fn error(
    rule: CompileTimeExecutionRule,
    subject: CompileTimeExecutionSubject,
    node: Option<BodyNodeId>,
) -> CompileTimeExecutionError {
    CompileTimeExecutionError {
        rule,
        subject,
        node,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::num::{NonZeroU32, NonZeroU64};
    use std::sync::Arc;

    use nocter_model::{
        Arena, ArenaBuilder, BodyId, BuiltinType, CompilationTarget, ConstantValue, FrozenValue,
    };

    use super::{
        CompileTimeExecutionRule, CompileTimeExecutionSubject, CompileTimeExecutor,
        CompileTimeValue,
    };
    use crate::{
        CompileTimeBinaryOperation, CompileTimeCallTarget, CompileTimeCallablePlan,
        CompileTimeEvaluationLimits, CompileTimeNode, CompileTimeOperation, CompileTimeParameter,
        CompileTimePlanTable, CompileTimeValueType, ConstantScalarType,
    };

    fn i32_type() -> CompileTimeValueType {
        CompileTimeValueType::Scalar(ConstantScalarType::Integer(BuiltinType::I32))
    }

    fn integer(value: i128) -> CompileTimeValue {
        CompileTimeValue::scalar(
            ConstantScalarType::Integer(BuiltinType::I32),
            ConstantValue::Integer(value),
        )
        .unwrap()
    }

    #[test]
    fn a_closed_call_executes_selected_nested_plans() {
        let mut callable_ids = ArenaBuilder::new();
        let increment = callable_ids.insert(());
        let answer = callable_ids.insert(());
        let mut parameter_ids = ArenaBuilder::new();
        let value = parameter_ids.insert(());

        let mut increment_nodes = ArenaBuilder::new();
        let read = increment_nodes.insert(CompileTimeNode::new(
            i32_type(),
            CompileTimeOperation::ReadParameter(value),
        ));
        let one = increment_nodes.insert(CompileTimeNode::new(
            i32_type(),
            CompileTimeOperation::Literal(ConstantValue::Integer(1)),
        ));
        let add = increment_nodes.insert(CompileTimeNode::new(
            i32_type(),
            CompileTimeOperation::Binary {
                operation: CompileTimeBinaryOperation::Add,
                left: read,
                right: one,
            },
        ));
        let increment_plan = CompileTimeCallablePlan::new(
            [CompileTimeParameter::new(value, i32_type())],
            i32_type(),
            Arena::default(),
            increment_nodes.finish(),
            add,
        )
        .unwrap();
        let increment_target = CompileTimeCallTarget::new(increment, []).unwrap();

        let mut answer_nodes = ArenaBuilder::new();
        let forty_one = answer_nodes.insert(CompileTimeNode::new(
            i32_type(),
            CompileTimeOperation::Literal(ConstantValue::Integer(41)),
        ));
        let call = answer_nodes.insert(CompileTimeNode::new(
            i32_type(),
            CompileTimeOperation::Call {
                target: increment_target.clone(),
                receiver: None,
                arguments: Box::new([forty_one]),
            },
        ));
        let answer_plan = CompileTimeCallablePlan::new(
            Vec::<CompileTimeParameter<CompileTimeValueType>>::new(),
            i32_type(),
            Arena::default(),
            answer_nodes.finish(),
            call,
        )
        .unwrap();
        let answer_target = CompileTimeCallTarget::new(answer, []).unwrap();
        let plans = CompileTimePlanTable::new(HashMap::from([
            (increment_target, Arc::new(increment_plan)),
            (answer_target.clone(), Arc::new(answer_plan)),
        ]))
        .unwrap();
        let constants = Arena::default();
        let mut executor = CompileTimeExecutor::new(
            &plans,
            &constants,
            CompilationTarget::Arm64Darwin,
            CompileTimeEvaluationLimits::default(),
        );

        let result = executor.evaluate(&answer_target, []).unwrap();

        assert_eq!(result.scalar_value(), Some(&ConstantValue::Integer(42)));
    }

    #[test]
    fn argument_types_are_validated_by_the_plan() {
        let mut callable_ids = ArenaBuilder::new();
        let identity = callable_ids.insert(());
        let mut parameter_ids = ArenaBuilder::new();
        let value = parameter_ids.insert(());
        let mut nodes = ArenaBuilder::new();
        let read = nodes.insert(CompileTimeNode::new(
            i32_type(),
            CompileTimeOperation::ReadParameter(value),
        ));
        let plan = CompileTimeCallablePlan::new(
            [CompileTimeParameter::new(value, i32_type())],
            i32_type(),
            Arena::default(),
            nodes.finish(),
            read,
        )
        .unwrap();
        let target = CompileTimeCallTarget::new(identity, []).unwrap();
        let plans =
            CompileTimePlanTable::new(HashMap::from([(target.clone(), Arc::new(plan))])).unwrap();
        let constants = Arena::default();
        let mut executor = CompileTimeExecutor::new(
            &plans,
            &constants,
            CompilationTarget::Arm64Darwin,
            CompileTimeEvaluationLimits::default(),
        );
        let boolean =
            CompileTimeValue::scalar(ConstantScalarType::Bool, ConstantValue::Bool(true)).unwrap();

        let error = executor.evaluate(&target, [boolean]).unwrap_err();

        assert_eq!(error.rule(), CompileTimeExecutionRule::TypeMismatch);
        assert_eq!(error.node(), None);
    }

    #[test]
    fn an_initializer_uses_the_same_closed_execution_model() {
        let mut body_ids = ArenaBuilder::<BodyId, ()>::new();
        let body = body_ids.insert(());
        let mut nodes = ArenaBuilder::new();
        let forty = nodes.insert(CompileTimeNode::new(
            i32_type(),
            CompileTimeOperation::Literal(ConstantValue::Integer(40)),
        ));
        let two = nodes.insert(CompileTimeNode::new(
            i32_type(),
            CompileTimeOperation::Literal(ConstantValue::Integer(2)),
        ));
        let sum = nodes.insert(CompileTimeNode::new(
            i32_type(),
            CompileTimeOperation::Binary {
                operation: CompileTimeBinaryOperation::Add,
                left: forty,
                right: two,
            },
        ));
        let plan = CompileTimeCallablePlan::new(
            Vec::<CompileTimeParameter<CompileTimeValueType>>::new(),
            i32_type(),
            Arena::default(),
            nodes.finish(),
            sum,
        )
        .unwrap();
        let plans = CompileTimePlanTable::new(HashMap::new()).unwrap();
        let constants = Arena::default();
        let mut executor = CompileTimeExecutor::new(
            &plans,
            &constants,
            CompilationTarget::Arm64Darwin,
            CompileTimeEvaluationLimits::default(),
        );

        let result = executor.evaluate_initializer(body, &plan).unwrap();

        assert_eq!(result.scalar_value(), Some(&ConstantValue::Integer(42)));
    }

    #[test]
    fn recursive_calls_stop_at_the_deterministic_depth_limit() {
        let mut callable_ids = ArenaBuilder::new();
        let recurse = callable_ids.insert(());
        let target = CompileTimeCallTarget::new(recurse, []).unwrap();
        let mut nodes = ArenaBuilder::new();
        let call = nodes.insert(CompileTimeNode::new(
            i32_type(),
            CompileTimeOperation::Call {
                target: target.clone(),
                receiver: None,
                arguments: Box::new([]),
            },
        ));
        let plan = CompileTimeCallablePlan::new(
            Vec::<CompileTimeParameter<CompileTimeValueType>>::new(),
            i32_type(),
            Arena::default(),
            nodes.finish(),
            call,
        )
        .unwrap();
        let plans =
            CompileTimePlanTable::new(HashMap::from([(target.clone(), Arc::new(plan))])).unwrap();
        let limits = CompileTimeEvaluationLimits::new(
            NonZeroU64::new(100).unwrap(),
            NonZeroU32::new(3).unwrap(),
        );
        let constants = Arena::default();
        let mut executor =
            CompileTimeExecutor::new(&plans, &constants, CompilationTarget::Arm64Darwin, limits);

        let error = executor.evaluate(&target, []).unwrap_err();

        assert_eq!(error.rule(), CompileTimeExecutionRule::CallDepthLimit);
        assert_eq!(error.subject(), &CompileTimeExecutionSubject::Call(target));
    }

    #[test]
    fn integer_inputs_cannot_exceed_their_closed_width() {
        let error = CompileTimeValue::scalar(
            ConstantScalarType::Integer(BuiltinType::I8),
            ConstantValue::Integer(128),
        )
        .unwrap_err();

        assert_eq!(error, CompileTimeExecutionRule::TypeMismatch);
        assert_eq!(integer(1).scalar_value(), Some(&ConstantValue::Integer(1)));
    }

    #[test]
    fn aggregate_results_preserve_frozen_tuple_and_array_structure() {
        let array_type = CompileTimeValueType::FixedArray {
            element: Box::new(i32_type()),
            length: 2,
        };
        let array = CompileTimeValue::fixed_array(array_type.clone(), vec![integer(1), integer(2)])
            .unwrap();
        let tuple = CompileTimeValue::tuple(
            CompileTimeValueType::Tuple(Box::new([i32_type(), array_type])),
            vec![integer(3), array],
        )
        .unwrap();

        assert_eq!(
            tuple.into_frozen().unwrap(),
            FrozenValue::Tuple(Box::new([
                FrozenValue::Scalar(ConstantValue::Integer(3)),
                FrozenValue::FixedArray(Box::new([
                    FrozenValue::Scalar(ConstantValue::Integer(1)),
                    FrozenValue::Scalar(ConstantValue::Integer(2)),
                ])),
            ]))
        );
    }
}
