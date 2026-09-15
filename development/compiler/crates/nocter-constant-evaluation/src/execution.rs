use std::collections::HashMap;
use std::sync::Arc;

use nocter_model::{
    BodyNodeId, CompilationTarget, ConstantId, ConstantValue, LocalBindingId, ParameterId,
};

use crate::scalar::{self, ScalarEvaluationFailure};
use crate::{
    CompileTimeCallTarget, CompileTimeCallablePlan, CompileTimeEvaluationLimits,
    CompileTimeLogicalOperation, CompileTimeOperation, CompileTimePlanTable, CompileTimeValueType,
    ConstantScalarType,
};

/// One typed, storage-independent value carried by the compile-time executor.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CompileTimeValue {
    ty: CompileTimeValueType,
    representation: CompileTimeValueRepresentation,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum CompileTimeValueRepresentation {
    Void,
    Scalar(ConstantValue),
    Tuple(Box<[CompileTimeValue]>),
    FixedArray(Box<[CompileTimeValue]>),
}

impl CompileTimeValue {
    /// Constructs a typed scalar after proving that its representation matches the declared
    /// scalar shape.
    ///
    /// # Errors
    ///
    /// Returns `TypeMismatch` instead of admitting an ill-typed evaluator input.
    pub fn scalar(
        ty: ConstantScalarType,
        value: ConstantValue,
    ) -> Result<Self, CompileTimeExecutionRule> {
        let value = Self {
            ty: CompileTimeValueType::Scalar(ty),
            representation: CompileTimeValueRepresentation::Scalar(value),
        };
        value
            .matches_type(&value.ty)
            .then_some(value)
            .ok_or(CompileTimeExecutionRule::TypeMismatch)
    }

    #[must_use]
    pub fn ty(&self) -> &CompileTimeValueType {
        &self.ty
    }

    #[must_use]
    pub fn scalar_value(&self) -> Option<&ConstantValue> {
        match &self.representation {
            CompileTimeValueRepresentation::Scalar(value) => Some(value),
            CompileTimeValueRepresentation::Void
            | CompileTimeValueRepresentation::Tuple(_)
            | CompileTimeValueRepresentation::FixedArray(_) => None,
        }
    }

    fn void() -> Self {
        Self {
            ty: CompileTimeValueType::Void,
            representation: CompileTimeValueRepresentation::Void,
        }
    }

    fn tuple(
        ty: CompileTimeValueType,
        values: Vec<Self>,
    ) -> Result<Self, CompileTimeExecutionRule> {
        Self::aggregate(
            ty,
            CompileTimeValueRepresentation::Tuple(values.into_boxed_slice()),
        )
    }

    fn fixed_array(
        ty: CompileTimeValueType,
        values: Vec<Self>,
    ) -> Result<Self, CompileTimeExecutionRule> {
        Self::aggregate(
            ty,
            CompileTimeValueRepresentation::FixedArray(values.into_boxed_slice()),
        )
    }

    fn aggregate(
        ty: CompileTimeValueType,
        representation: CompileTimeValueRepresentation,
    ) -> Result<Self, CompileTimeExecutionRule> {
        let value = Self { ty, representation };
        value
            .matches_type(&value.ty)
            .then_some(value)
            .ok_or(CompileTimeExecutionRule::TypeMismatch)
    }

    fn matches_type(&self, expected: &CompileTimeValueType) -> bool {
        match (expected, &self.representation) {
            (CompileTimeValueType::Void, CompileTimeValueRepresentation::Void) => true,
            (CompileTimeValueType::Scalar(ty), CompileTimeValueRepresentation::Scalar(value)) => {
                scalar_matches(*ty, value)
            }
            (CompileTimeValueType::ReadonlyBorrow(referent), _) => self.matches_type(referent),
            (CompileTimeValueType::Tuple(types), CompileTimeValueRepresentation::Tuple(values)) => {
                types.len() == values.len()
                    && types
                        .iter()
                        .zip(values)
                        .all(|(ty, value)| value.matches_type(ty))
            }
            (
                CompileTimeValueType::FixedArray { element, length },
                CompileTimeValueRepresentation::FixedArray(values),
            ) => {
                usize::try_from(*length).ok() == Some(values.len())
                    && values.iter().all(|value| value.matches_type(element))
            }
            (
                CompileTimeValueType::Never
                | CompileTimeValueType::Void
                | CompileTimeValueType::Scalar(_)
                | CompileTimeValueType::Tuple(_)
                | CompileTimeValueType::FixedArray { .. },
                _,
            ) => false,
        }
    }

    fn into_type(
        mut self,
        expected: &CompileTimeValueType,
    ) -> Result<Self, CompileTimeExecutionRule> {
        if !self.matches_type(expected) {
            return Err(CompileTimeExecutionRule::TypeMismatch);
        }
        self.ty = expected.clone();
        Ok(self)
    }
}

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
pub struct CompileTimeExecutionError {
    rule: CompileTimeExecutionRule,
    target: CompileTimeCallTarget,
    node: Option<BodyNodeId>,
}

impl CompileTimeExecutionError {
    #[must_use]
    pub const fn rule(&self) -> CompileTimeExecutionRule {
        self.rule
    }

    #[must_use]
    pub const fn target(&self) -> &CompileTimeCallTarget {
        &self.target
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

/// Evaluates closed callable plans with one deterministic budget and call-result cache.
pub struct CompileTimeExecutor<'program, L> {
    plans: &'program CompileTimePlanTable,
    target: CompilationTarget,
    lookup_constant: L,
    remaining_steps: u64,
    maximum_call_depth: u32,
    completed_calls: HashMap<CallKey, Arc<CompileTimeValue>>,
}

impl<'program, L> CompileTimeExecutor<'program, L>
where
    L: FnMut(ConstantId) -> Option<ConstantValue>,
{
    #[must_use]
    pub fn new(
        plans: &'program CompileTimePlanTable,
        target: CompilationTarget,
        limits: CompileTimeEvaluationLimits,
        lookup_constant: L,
    ) -> Self {
        Self {
            plans,
            target,
            lookup_constant,
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
                target,
                None,
            ));
        }
        let plan = self
            .plans
            .get(&target)
            .ok_or_else(|| error(CompileTimeExecutionRule::MissingPlan, target.clone(), None))?;
        let parameters = bind_parameters(plan, &key.arguments)
            .map_err(|rule| error(rule, target.clone(), None))?;
        let mut frame = Frame {
            target: target.clone(),
            plan,
            parameters,
            locals: HashMap::new(),
        };
        let outcome = self.evaluate_node(&mut frame, plan.root(), depth)?;
        let value = match outcome {
            Flow::Value(value) | Flow::Return(value) => value,
            Flow::Unreachable => {
                return Err(error(
                    CompileTimeExecutionRule::ReachedUnreachable,
                    target,
                    Some(plan.root()),
                ));
            }
        };
        let value = value
            .into_type(plan.result())
            .map_err(|rule| error(rule, target.clone(), Some(plan.root())))?;
        let value = Arc::new(value);
        self.completed_calls.insert(key, Arc::clone(&value));
        Ok(value)
    }

    // Keeping the exhaustive operation dispatch together ensures that every new checked-plan
    // operation receives an execution decision in the same compiler error site.
    #[allow(clippy::too_many_lines)]
    fn evaluate_node(
        &mut self,
        frame: &mut Frame<'_>,
        node_id: BodyNodeId,
        depth: u32,
    ) -> Result<Flow, CompileTimeExecutionError> {
        if self.remaining_steps == 0 {
            return Err(frame.error(CompileTimeExecutionRule::StepLimit, Some(node_id)));
        }
        self.remaining_steps -= 1;
        let node = frame
            .plan
            .nodes()
            .get(node_id)
            .ok_or_else(|| frame.error(CompileTimeExecutionRule::InvalidPlan, Some(node_id)))?;
        let flow = match node.operation() {
            CompileTimeOperation::Complete => Flow::Value(CompileTimeValue::void()),
            CompileTimeOperation::Literal(value) => Flow::Value(
                scalar_value(node.ty(), value.clone())
                    .map_err(|rule| frame.error(rule, Some(node_id)))?,
            ),
            CompileTimeOperation::DeclaredConstant(id) => {
                let value = (self.lookup_constant)(*id).ok_or_else(|| {
                    frame.error(CompileTimeExecutionRule::MissingConstant, Some(node_id))
                })?;
                Flow::Value(
                    scalar_value(node.ty(), value)
                        .map_err(|rule| frame.error(rule, Some(node_id)))?,
                )
            }
            CompileTimeOperation::ReadParameter(parameter) => {
                Flow::Value(frame.parameters.get(parameter).cloned().ok_or_else(|| {
                    frame.error(CompileTimeExecutionRule::InvalidPlan, Some(node_id))
                })?)
            }
            CompileTimeOperation::ReadLocal(local) => {
                Flow::Value(frame.locals.get(local).cloned().ok_or_else(|| {
                    frame.error(CompileTimeExecutionRule::InvalidPlan, Some(node_id))
                })?)
            }
            CompileTimeOperation::Unary { operation, operand } => {
                let operand = self.value(frame, *operand, depth)?;
                let ty = scalar_type(node.ty()).map_err(|rule| frame.error(rule, Some(node_id)))?;
                let value = scalar::unary(
                    *operation,
                    ty,
                    scalar_representation(&operand)
                        .map_err(|rule| frame.error(rule, Some(node_id)))?,
                    self.target,
                )
                .map_err(|failure| frame.error(map_scalar_failure(failure), Some(node_id)))?;
                Flow::Value(
                    CompileTimeValue::scalar(ty, value)
                        .map_err(|rule| frame.error(rule, Some(node_id)))?,
                )
            }
            CompileTimeOperation::Binary {
                operation,
                left,
                right,
            } => {
                let left = self.value(frame, *left, depth)?;
                let right = self.value(frame, *right, depth)?;
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
                Flow::Value(
                    CompileTimeValue::scalar(ty, value)
                        .map_err(|rule| frame.error(rule, Some(node_id)))?,
                )
            }
            CompileTimeOperation::NumericConversion { operand, target } => {
                let operand = self.value(frame, *operand, depth)?;
                let value = scalar::convert(
                    *target,
                    scalar_representation(&operand)
                        .map_err(|rule| frame.error(rule, Some(node_id)))?,
                    self.target,
                )
                .map_err(|failure| frame.error(map_scalar_failure(failure), Some(node_id)))?;
                Flow::Value(
                    CompileTimeValue::scalar(*target, value)
                        .map_err(|rule| frame.error(rule, Some(node_id)))?,
                )
            }
            CompileTimeOperation::Comparison {
                operation,
                left,
                right,
            } => {
                let left = self.value(frame, *left, depth)?;
                let right = self.value(frame, *right, depth)?;
                let value = scalar::compare(
                    *operation,
                    scalar_representation(&left)
                        .map_err(|rule| frame.error(rule, Some(node_id)))?,
                    scalar_representation(&right)
                        .map_err(|rule| frame.error(rule, Some(node_id)))?,
                    self.target,
                )
                .map_err(|failure| frame.error(map_scalar_failure(failure), Some(node_id)))?;
                Flow::Value(
                    CompileTimeValue::scalar(ConstantScalarType::Bool, value)
                        .map_err(|rule| frame.error(rule, Some(node_id)))?,
                )
            }
            CompileTimeOperation::Tuple(elements) => {
                let values = self.values(frame, elements, depth)?;
                Flow::Value(
                    CompileTimeValue::tuple(node.ty().clone(), values)
                        .map_err(|rule| frame.error(rule, Some(node_id)))?,
                )
            }
            CompileTimeOperation::FixedArray(elements) => {
                let values = self.values(frame, elements, depth)?;
                Flow::Value(
                    CompileTimeValue::fixed_array(node.ty().clone(), values)
                        .map_err(|rule| frame.error(rule, Some(node_id)))?,
                )
            }
            CompileTimeOperation::Call {
                target,
                receiver,
                arguments,
            } => {
                let mut values =
                    Vec::with_capacity(arguments.len() + usize::from(receiver.is_some()));
                if let Some(receiver) = receiver {
                    values.push(self.value(frame, *receiver, depth)?);
                }
                values.extend(self.values(frame, arguments, depth)?);
                Flow::Value(
                    self.evaluate_call(target.clone(), values.into_boxed_slice(), depth + 1)?
                        .as_ref()
                        .clone(),
                )
            }
            CompileTimeOperation::Block { statements, result } => {
                for statement in statements {
                    match self.evaluate_node(frame, *statement, depth)? {
                        Flow::Value(_) => {}
                        flow @ (Flow::Return(_) | Flow::Unreachable) => return Ok(flow),
                    }
                }
                match result {
                    Some(result) => self.evaluate_node(frame, *result, depth)?,
                    None => Flow::Value(CompileTimeValue::void()),
                }
            }
            CompileTimeOperation::Bind {
                binding,
                initializer,
            } => {
                let value = self.value(frame, *initializer, depth)?;
                if let Some(binding) = binding {
                    let expected = frame.plan.locals().get(*binding).ok_or_else(|| {
                        frame.error(CompileTimeExecutionRule::InvalidPlan, Some(node_id))
                    })?;
                    if !value.matches_type(expected) {
                        return Err(
                            frame.error(CompileTimeExecutionRule::TypeMismatch, Some(node_id))
                        );
                    }
                    frame.locals.insert(*binding, value);
                }
                Flow::Value(CompileTimeValue::void())
            }
            CompileTimeOperation::Discard(value) => {
                self.value(frame, *value, depth)?;
                Flow::Value(CompileTimeValue::void())
            }
            CompileTimeOperation::Unreachable => Flow::Unreachable,
            CompileTimeOperation::Return(value) => Flow::Return(match value {
                Some(value) => self.value(frame, *value, depth)?,
                None => CompileTimeValue::void(),
            }),
            CompileTimeOperation::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition = self.value(frame, *condition, depth)?;
                if bool_representation(&condition)
                    .map_err(|rule| frame.error(rule, Some(node_id)))?
                {
                    self.evaluate_node(frame, *then_branch, depth)?
                } else if let Some(else_branch) = else_branch {
                    self.evaluate_node(frame, *else_branch, depth)?
                } else {
                    Flow::Value(CompileTimeValue::void())
                }
            }
            CompileTimeOperation::Logical {
                operation,
                left,
                right,
            } => {
                let left = self.value(frame, *left, depth)?;
                let left =
                    bool_representation(&left).map_err(|rule| frame.error(rule, Some(node_id)))?;
                let value = match operation {
                    CompileTimeLogicalOperation::And if !left => false,
                    CompileTimeLogicalOperation::Or if left => true,
                    CompileTimeLogicalOperation::And | CompileTimeLogicalOperation::Or => {
                        let right = self.value(frame, *right, depth)?;
                        bool_representation(&right)
                            .map_err(|rule| frame.error(rule, Some(node_id)))?
                    }
                };
                Flow::Value(
                    CompileTimeValue::scalar(ConstantScalarType::Bool, ConstantValue::Bool(value))
                        .map_err(|rule| frame.error(rule, Some(node_id)))?,
                )
            }
        };
        match flow {
            Flow::Value(value) => value
                .into_type(node.ty())
                .map(Flow::Value)
                .map_err(|rule| frame.error(rule, Some(node_id))),
            Flow::Return(value) => Ok(Flow::Return(value)),
            Flow::Unreachable => Ok(Flow::Unreachable),
        }
    }

    fn value(
        &mut self,
        frame: &mut Frame<'_>,
        node: BodyNodeId,
        depth: u32,
    ) -> Result<CompileTimeValue, CompileTimeExecutionError> {
        match self.evaluate_node(frame, node, depth)? {
            Flow::Value(value) => Ok(value),
            Flow::Return(_) | Flow::Unreachable => {
                Err(frame.error(CompileTimeExecutionRule::InvalidPlan, Some(node)))
            }
        }
    }

    fn values(
        &mut self,
        frame: &mut Frame<'_>,
        nodes: &[BodyNodeId],
        depth: u32,
    ) -> Result<Vec<CompileTimeValue>, CompileTimeExecutionError> {
        nodes
            .iter()
            .map(|node| self.value(frame, *node, depth))
            .collect()
    }
}

struct Frame<'plan> {
    target: CompileTimeCallTarget,
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
        error(rule, self.target.clone(), node)
    }
}

enum Flow {
    Value(CompileTimeValue),
    Return(CompileTimeValue),
    Unreachable,
}

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

fn scalar_matches(ty: ConstantScalarType, value: &ConstantValue) -> bool {
    match (ty, value) {
        (ConstantScalarType::Bool, ConstantValue::Bool(_))
        | (ConstantScalarType::Character, ConstantValue::Character(_))
        | (ConstantScalarType::Float(crate::FloatFormat::Binary32), ConstantValue::Float32(_))
        | (ConstantScalarType::Float(crate::FloatFormat::Binary64), ConstantValue::Float64(_))
        | (ConstantScalarType::Text, ConstantValue::Text(_)) => true,
        (ConstantScalarType::Integer(builtin), ConstantValue::Integer(value)) => {
            crate::support::integer_spec(builtin).is_some_and(|spec| spec.contains(*value))
        }
        _ => false,
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
    target: CompileTimeCallTarget,
    node: Option<BodyNodeId>,
) -> CompileTimeExecutionError {
    CompileTimeExecutionError { rule, target, node }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::num::{NonZeroU32, NonZeroU64};
    use std::sync::Arc;

    use nocter_model::{Arena, ArenaBuilder, BuiltinType, CompilationTarget, ConstantValue};

    use super::{CompileTimeExecutionRule, CompileTimeExecutor, CompileTimeValue};
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
        let mut executor = CompileTimeExecutor::new(
            &plans,
            CompilationTarget::Arm64Darwin,
            CompileTimeEvaluationLimits::default(),
            |_| None,
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
        let mut executor = CompileTimeExecutor::new(
            &plans,
            CompilationTarget::Arm64Darwin,
            CompileTimeEvaluationLimits::default(),
            |_| None,
        );
        let boolean =
            CompileTimeValue::scalar(ConstantScalarType::Bool, ConstantValue::Bool(true)).unwrap();

        let error = executor.evaluate(&target, [boolean]).unwrap_err();

        assert_eq!(error.rule(), CompileTimeExecutionRule::TypeMismatch);
        assert_eq!(error.node(), None);
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
        let mut executor =
            CompileTimeExecutor::new(&plans, CompilationTarget::Arm64Darwin, limits, |_| None);

        let error = executor.evaluate(&target, []).unwrap_err();

        assert_eq!(error.rule(), CompileTimeExecutionRule::CallDepthLimit);
        assert_eq!(error.target(), &target);
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
}
