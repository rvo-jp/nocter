mod query;

use std::collections::HashMap;

use nocter_constant_evaluation::{
    CompileTimeBinaryOperation, CompileTimeCallTarget, CompileTimeCallablePlan,
    CompileTimeCallableRecipe, CompileTimeComparisonOperation, CompileTimeGenericArgument,
    CompileTimeLogicalOperation, CompileTimeNode, CompileTimeOperation, CompileTimeParameter,
    CompileTimeRecipeCallTarget, CompileTimeType, CompileTimeUnaryOperation, CompileTimeValueType,
    ConstantScalarType, FloatFormat, InvalidCompileTimeCallable,
};
use nocter_declarations::DeclarationGraph;
use nocter_model::{
    BorrowCapability, BuiltinType, CallableId, CompileTimeGuarantee, TypeId, TypeKind, TypeStore,
};

use crate::{
    AggregateConstruction, CallTarget, CheckedBindingPattern, CheckedBody, CheckedCallExecution,
    CheckedComparisonPlan, CheckedControl, CheckedOperation, ComparisonImplementation,
    ComparisonOperation, LogicalOperation, PlaceRoot, PrimitiveBinary, PrimitiveOperation,
    PrimitiveUnary, ReceiverPreparation, StaticDispatch,
};

/// Why an ordinary checked operation cannot enter the initial compile-time plan domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileTimeProjectionRule {
    UnsupportedValueType,
    UnsupportedOperation,
    MutableReceiver,
    DynamicDispatch,
    RuntimeOnlyCall,
    UnavailableCallTarget,
    DeferredCall,
    ArgumentPack,
    ProjectedPlace,
    InvalidPlan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompileTimeProjectionError {
    callable: CallableId,
    body: Option<nocter_model::BodyId>,
    node: Option<nocter_model::BodyNodeId>,
    rule: CompileTimeProjectionRule,
}

impl CompileTimeProjectionError {
    #[must_use]
    pub const fn callable(self) -> CallableId {
        self.callable
    }

    #[must_use]
    pub const fn body(self) -> Option<nocter_model::BodyId> {
        self.body
    }

    #[must_use]
    pub const fn node(self) -> Option<nocter_model::BodyNodeId> {
        self.node
    }

    #[must_use]
    pub const fn rule(self) -> CompileTimeProjectionRule {
        self.rule
    }
}

struct Projector<'a> {
    graph: &'a DeclarationGraph,
    types: &'a TypeStore,
    callable: CallableId,
    body_id: nocter_model::BodyId,
    body: &'a CheckedBody,
}

pub(crate) use query::build_compile_time_plan_table;

fn project_compile_time_callable_recipe(
    graph: &DeclarationGraph,
    types: &TypeStore,
    callable: CallableId,
    body_id: nocter_model::BodyId,
    body: &CheckedBody,
) -> Result<CompileTimeCallableRecipe, CompileTimeProjectionError> {
    let declaration =
        graph
            .declarations()
            .callables()
            .get(callable)
            .ok_or(CompileTimeProjectionError {
                callable,
                body: None,
                node: None,
                rule: CompileTimeProjectionRule::InvalidPlan,
            })?;
    let projector = Projector {
        graph,
        types,
        callable,
        body_id,
        body,
    };
    let locals = body.locals().try_map(|_, local| {
        projector.require_recipe_type(local.ty(), None)?;
        Ok(local.ty())
    })?;
    let nodes = body.nodes().try_map(|node, checked| {
        projector.require_recipe_type(checked.ty(), Some(node))?;
        let operation = projector.operation(node, checked.operation())?;
        Ok(CompileTimeNode::new(checked.ty(), operation))
    })?;
    let parameters = declaration
        .receiver()
        .into_iter()
        .chain(declaration.parameters().iter().copied())
        .map(|parameter| {
            graph
                .declarations()
                .parameters()
                .get(parameter)
                .copied()
                .map(|declaration| CompileTimeParameter::new(parameter, declaration.ty()))
                .ok_or_else(|| projector.error(None, CompileTimeProjectionRule::InvalidPlan))
        })
        .collect::<Result<Vec<_>, _>>()?;
    projector.require_recipe_type(declaration.body_result(), None)?;
    CompileTimeCallableRecipe::new(
        parameters,
        declaration.body_result(),
        locals,
        nodes,
        body.root(),
    )
    .map_err(|error| {
        let node = match error {
            InvalidCompileTimeCallable::MissingNode(node) => Some(node),
            InvalidCompileTimeCallable::MissingParameter(_)
            | InvalidCompileTimeCallable::DuplicateParameter(_)
            | InvalidCompileTimeCallable::MissingLocal(_) => None,
        };
        projector.error(node, CompileTimeProjectionRule::InvalidPlan)
    })
}

fn specialize_compile_time_callable_recipe(
    graph: &DeclarationGraph,
    types: &TypeStore,
    target: &CompileTimeCallTarget,
    recipe: &CompileTimeCallableRecipe,
) -> Result<CompileTimeCallablePlan, CompileTimeProjectionError> {
    let callable = target.callable();
    let declaration =
        graph
            .declarations()
            .callables()
            .get(callable)
            .ok_or(CompileTimeProjectionError {
                callable,
                body: None,
                node: None,
                rule: CompileTimeProjectionRule::InvalidPlan,
            })?;
    let domain = graph
        .declarations()
        .callable_generic_domain(callable)
        .ok_or(CompileTimeProjectionError {
            callable,
            body: declaration.body(),
            node: None,
            rule: CompileTimeProjectionRule::InvalidPlan,
        })?;
    if domain.len() != target.generic_arguments().len()
        || domain.iter().copied().ne(target
            .generic_arguments()
            .iter()
            .map(CompileTimeGenericArgument::parameter))
    {
        return Err(CompileTimeProjectionError {
            callable,
            body: declaration.body(),
            node: None,
            rule: CompileTimeProjectionRule::InvalidPlan,
        });
    }
    let specializer = Specializer {
        types,
        callable,
        body: declaration.body(),
        substitution: target
            .generic_arguments()
            .iter()
            .map(|argument| (argument.parameter(), argument.ty().clone()))
            .collect(),
    };
    let locals = recipe.locals().try_map(|_, ty| {
        specializer
            .value_type(*ty)
            .ok_or_else(|| specializer.error(None, CompileTimeProjectionRule::UnsupportedValueType))
    })?;
    let nodes = recipe.nodes().try_map(|node, recipe_node| {
        let ty = specializer.value_type(*recipe_node.ty()).ok_or_else(|| {
            specializer.error(Some(node), CompileTimeProjectionRule::UnsupportedValueType)
        })?;
        let operation = recipe_node
            .operation()
            .clone()
            .try_map_call_target(|target| specializer.call_target(&target, node))?;
        Ok(CompileTimeNode::new(ty, operation))
    })?;
    let parameters = recipe
        .parameters()
        .iter()
        .map(|parameter| {
            specializer
                .value_type(*parameter.ty())
                .map(|ty| CompileTimeParameter::new(parameter.id(), ty))
                .ok_or_else(|| {
                    specializer.error(None, CompileTimeProjectionRule::UnsupportedValueType)
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result = specializer
        .value_type(*recipe.result())
        .ok_or_else(|| specializer.error(None, CompileTimeProjectionRule::UnsupportedValueType))?;
    CompileTimeCallablePlan::new(parameters, result, locals, nodes, recipe.root()).map_err(
        |error| {
            let node = match error {
                InvalidCompileTimeCallable::MissingNode(node) => Some(node),
                InvalidCompileTimeCallable::MissingParameter(_)
                | InvalidCompileTimeCallable::DuplicateParameter(_)
                | InvalidCompileTimeCallable::MissingLocal(_) => None,
            };
            specializer.error(node, CompileTimeProjectionRule::InvalidPlan)
        },
    )
}

struct Specializer<'a> {
    types: &'a TypeStore,
    callable: CallableId,
    body: Option<nocter_model::BodyId>,
    substitution: HashMap<nocter_model::GenericParameterId, CompileTimeType>,
}

impl Projector<'_> {
    fn operation(
        &self,
        node: nocter_model::BodyNodeId,
        operation: &CheckedOperation,
    ) -> Result<CompileTimeOperation<CompileTimeRecipeCallTarget>, CompileTimeProjectionError> {
        match operation {
            CheckedOperation::Complete => Ok(CompileTimeOperation::Complete),
            CheckedOperation::Literal(value) => Ok(CompileTimeOperation::Literal(value.clone())),
            CheckedOperation::DeclaredConstant(id) => {
                Ok(CompileTimeOperation::DeclaredConstant(*id))
            }
            CheckedOperation::Place(place)
            | CheckedOperation::Copy(place)
            | CheckedOperation::Move(place) => self.read_place(node, *place),
            CheckedOperation::Borrow { capability, place }
                if *capability == BorrowCapability::Readonly =>
            {
                self.read_place(node, *place)
            }
            CheckedOperation::Primitive(operation) => self.primitive(node, operation),
            CheckedOperation::Aggregate(AggregateConstruction::Tuple(elements)) => {
                Ok(CompileTimeOperation::Tuple(elements.clone()))
            }
            CheckedOperation::Aggregate(AggregateConstruction::FixedArray(elements)) => {
                Ok(CompileTimeOperation::FixedArray(elements.clone()))
            }
            CheckedOperation::Call(call) => self.call(node, call),
            CheckedOperation::Comparison(comparison) => self.comparison(node, comparison),
            CheckedOperation::Control(control) => self.control(node, control),
            CheckedOperation::Borrow { .. }
            | CheckedOperation::Await(_)
            | CheckedOperation::BorrowConversion(_)
            | CheckedOperation::CallableGuaranteeErasure(_)
            | CheckedOperation::Aggregate(_)
            | CheckedOperation::Outcome(_)
            | CheckedOperation::OpaqueWitness(_)
            | CheckedOperation::Closure(_)
            | CheckedOperation::ArgumentPackLength(_)
            | CheckedOperation::IteratorAcquisition(_)
            | CheckedOperation::PackLiteral(_)
            | CheckedOperation::StringLiteral { .. }
            | CheckedOperation::Interpolation(_) => {
                Err(self.error(Some(node), CompileTimeProjectionRule::UnsupportedOperation))
            }
        }
    }

    fn comparison(
        &self,
        node: nocter_model::BodyNodeId,
        comparison: &crate::CheckedComparison,
    ) -> Result<CompileTimeOperation<CompileTimeRecipeCallTarget>, CompileTimeProjectionError> {
        if comparison.left().coercion().is_some() || comparison.right().coercion().is_some() {
            return Err(self.error(Some(node), CompileTimeProjectionRule::UnsupportedOperation));
        }
        let operation = match comparison.plan() {
            CheckedComparisonPlan::Direct { step, negate }
                if matches!(step.implementation(), ComparisonImplementation::Primitive) =>
            {
                match (step.operation(), step.reverse(), *negate) {
                    (ComparisonOperation::Equal, _, false) => CompileTimeComparisonOperation::Equal,
                    (ComparisonOperation::Equal, _, true) => {
                        CompileTimeComparisonOperation::NotEqual
                    }
                    (ComparisonOperation::Less, false, false) => {
                        CompileTimeComparisonOperation::Less
                    }
                    (ComparisonOperation::Less, false, true) => {
                        CompileTimeComparisonOperation::GreaterEqual
                    }
                    (ComparisonOperation::Less, true, false) => {
                        CompileTimeComparisonOperation::Greater
                    }
                    (ComparisonOperation::Less, true, true) => {
                        CompileTimeComparisonOperation::LessEqual
                    }
                }
            }
            CheckedComparisonPlan::Inclusive { strict, equal }
                if matches!(strict.implementation(), ComparisonImplementation::Primitive)
                    && matches!(equal.implementation(), ComparisonImplementation::Primitive)
                    && strict.operation() == ComparisonOperation::Less
                    && equal.operation() == ComparisonOperation::Equal
                    && strict.reverse() == equal.reverse() =>
            {
                if strict.reverse() {
                    CompileTimeComparisonOperation::GreaterEqual
                } else {
                    CompileTimeComparisonOperation::LessEqual
                }
            }
            CheckedComparisonPlan::Direct { .. }
            | CheckedComparisonPlan::Inclusive { .. }
            | CheckedComparisonPlan::Unreachable => {
                return Err(self.error(Some(node), CompileTimeProjectionRule::UnsupportedOperation));
            }
        };
        Ok(CompileTimeOperation::Comparison {
            operation,
            left: comparison.left().value(),
            right: comparison.right().value(),
        })
    }

    fn read_place(
        &self,
        node: nocter_model::BodyNodeId,
        place: nocter_model::PlaceId,
    ) -> Result<CompileTimeOperation<CompileTimeRecipeCallTarget>, CompileTimeProjectionError> {
        let place = self
            .body
            .places()
            .get(place)
            .ok_or_else(|| self.error(Some(node), CompileTimeProjectionRule::InvalidPlan))?;
        if !place.projections().is_empty() {
            return Err(self.error(Some(node), CompileTimeProjectionRule::ProjectedPlace));
        }
        match place.root() {
            PlaceRoot::Parameter(parameter) => Ok(CompileTimeOperation::ReadParameter(parameter)),
            PlaceRoot::Local(local) => Ok(CompileTimeOperation::ReadLocal(local)),
            PlaceRoot::Capture(_) | PlaceRoot::Static(_) | PlaceRoot::Value(_) => {
                Err(self.error(Some(node), CompileTimeProjectionRule::UnsupportedOperation))
            }
        }
    }

    fn primitive(
        &self,
        node: nocter_model::BodyNodeId,
        operation: &PrimitiveOperation,
    ) -> Result<CompileTimeOperation<CompileTimeRecipeCallTarget>, CompileTimeProjectionError> {
        match operation {
            PrimitiveOperation::Unary { operation, operand } => Ok(CompileTimeOperation::Unary {
                operation: match operation {
                    PrimitiveUnary::LogicalNot => CompileTimeUnaryOperation::LogicalNot,
                    PrimitiveUnary::Negate => CompileTimeUnaryOperation::Negate,
                },
                operand: *operand,
            }),
            PrimitiveOperation::Binary {
                operation,
                left,
                right,
            } => Ok(CompileTimeOperation::Binary {
                operation: match operation {
                    PrimitiveBinary::Add => CompileTimeBinaryOperation::Add,
                    PrimitiveBinary::Subtract => CompileTimeBinaryOperation::Subtract,
                    PrimitiveBinary::Multiply => CompileTimeBinaryOperation::Multiply,
                    PrimitiveBinary::Divide => CompileTimeBinaryOperation::Divide,
                    PrimitiveBinary::Remainder => CompileTimeBinaryOperation::Remainder,
                    PrimitiveBinary::ShiftLeft => CompileTimeBinaryOperation::ShiftLeft,
                    PrimitiveBinary::ShiftRightSigned => {
                        CompileTimeBinaryOperation::ShiftRightSigned
                    }
                    PrimitiveBinary::ShiftRightUnsigned => {
                        CompileTimeBinaryOperation::ShiftRightUnsigned
                    }
                },
                left: *left,
                right: *right,
            }),
            PrimitiveOperation::NumericConversion { operand, target } => {
                let Some(target) = constant_scalar_type(self.types, *target) else {
                    return Err(
                        self.error(Some(node), CompileTimeProjectionRule::UnsupportedValueType)
                    );
                };
                Ok(CompileTimeOperation::NumericConversion {
                    operand: *operand,
                    target,
                })
            }
        }
    }

    fn call(
        &self,
        node: nocter_model::BodyNodeId,
        call: &crate::CheckedCall,
    ) -> Result<CompileTimeOperation<CompileTimeRecipeCallTarget>, CompileTimeProjectionError> {
        if call.pack().is_some() {
            return Err(self.error(Some(node), CompileTimeProjectionRule::ArgumentPack));
        }
        if matches!(call.execution(), CheckedCallExecution::Deferred { .. }) {
            return Err(self.error(Some(node), CompileTimeProjectionRule::DeferredCall));
        }
        let CallTarget::Static(selection) = call.target() else {
            return Err(self.error(Some(node), CompileTimeProjectionRule::DynamicDispatch));
        };
        let StaticDispatch::Direct(callee) = selection.dispatch() else {
            return Err(self.error(Some(node), CompileTimeProjectionRule::DynamicDispatch));
        };
        let declaration = self
            .graph
            .declarations()
            .callables()
            .get(callee)
            .ok_or_else(|| self.error(Some(node), CompileTimeProjectionRule::InvalidPlan))?;
        if declaration.guarantees().compile_time() != CompileTimeGuarantee::Evaluatable {
            return Err(self.error(Some(node), CompileTimeProjectionRule::RuntimeOnlyCall));
        }
        let receiver = call.receiver().map(|receiver| {
            if receiver.coercion().is_some()
                || matches!(
                    receiver.preparation(),
                    ReceiverPreparation::BorrowPlace(BorrowCapability::ReadWrite)
                        | ReceiverPreparation::BorrowTemporary(BorrowCapability::ReadWrite)
                        | ReceiverPreparation::PreserveBorrow(BorrowCapability::ReadWrite)
                )
            {
                Err(self.error(Some(node), CompileTimeProjectionRule::MutableReceiver))
            } else {
                Ok(receiver.value())
            }
        });
        let receiver = receiver.transpose()?;
        let generic_arguments = selection
            .generic_arguments()
            .as_slice()
            .iter()
            .map(|argument| CompileTimeGenericArgument::new(argument.parameter(), argument.ty()))
            .collect::<Vec<_>>();
        let target = CompileTimeRecipeCallTarget::new(callee, generic_arguments)
            .map_err(|_| self.error(Some(node), CompileTimeProjectionRule::InvalidPlan))?;
        Ok(CompileTimeOperation::Call {
            target,
            receiver,
            arguments: call.arguments().into(),
        })
    }

    fn control(
        &self,
        node: nocter_model::BodyNodeId,
        control: &CheckedControl,
    ) -> Result<CompileTimeOperation<CompileTimeRecipeCallTarget>, CompileTimeProjectionError> {
        match control {
            CheckedControl::Block {
                statements, result, ..
            } => Ok(CompileTimeOperation::Block {
                statements: statements.clone(),
                result: *result,
            }),
            CheckedControl::Bind {
                pattern,
                initializer,
            } => {
                let binding = match pattern {
                    CheckedBindingPattern::Local { binding, .. } => Some(*binding),
                    CheckedBindingPattern::Discard { .. } => None,
                    CheckedBindingPattern::Tuple { .. } => {
                        return Err(
                            self.error(Some(node), CompileTimeProjectionRule::UnsupportedOperation)
                        );
                    }
                };
                Ok(CompileTimeOperation::Bind {
                    binding,
                    initializer: *initializer,
                })
            }
            CheckedControl::Discard(value) => Ok(CompileTimeOperation::Discard(*value)),
            CheckedControl::Unreachable(_) => Ok(CompileTimeOperation::Unreachable),
            CheckedControl::Return(value) => Ok(CompileTimeOperation::Return(*value)),
            CheckedControl::If {
                condition,
                then_branch,
                else_branch,
            } => Ok(CompileTimeOperation::If {
                condition: *condition,
                then_branch: *then_branch,
                else_branch: *else_branch,
            }),
            CheckedControl::Logical {
                operation,
                left,
                right,
            } => Ok(CompileTimeOperation::Logical {
                operation: match operation {
                    LogicalOperation::And => CompileTimeLogicalOperation::And,
                    LogicalOperation::Or => CompileTimeLogicalOperation::Or,
                },
                left: *left,
                right: *right,
            }),
            CheckedControl::Assign { .. }
            | CheckedControl::CompoundAssign { .. }
            | CheckedControl::Break(_)
            | CheckedControl::Continue(_)
            | CheckedControl::Drop(_)
            | CheckedControl::Pattern { .. }
            | CheckedControl::Loop(_)
            | CheckedControl::Region { .. } => {
                Err(self.error(Some(node), CompileTimeProjectionRule::UnsupportedOperation))
            }
        }
    }

    fn require_recipe_type(
        &self,
        ty: TypeId,
        node: Option<nocter_model::BodyNodeId>,
    ) -> Result<(), CompileTimeProjectionError> {
        if self.recipe_type_supported(ty) {
            Ok(())
        } else {
            Err(self.error(node, CompileTimeProjectionRule::UnsupportedValueType))
        }
    }

    fn recipe_type_supported(&self, ty: TypeId) -> bool {
        match self.types.get(ty) {
            Some(TypeKind::Builtin(builtin)) => supported_builtin_value(*builtin),
            Some(TypeKind::GenericParameter(_)) => true,
            Some(TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent,
            }) if matches!(
                self.types.get(*referent),
                Some(TypeKind::Builtin(BuiltinType::Str))
            ) =>
            {
                true
            }
            Some(TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent,
            }) => self.recipe_type_supported(*referent),
            Some(TypeKind::Tuple(elements)) => elements
                .iter()
                .all(|element| self.recipe_type_supported(element)),
            Some(TypeKind::FixedArray { element, .. }) => self.recipe_type_supported(*element),
            Some(_) | None => false,
        }
    }
}

impl Specializer<'_> {
    fn call_target(
        &self,
        target: &CompileTimeRecipeCallTarget,
        node: nocter_model::BodyNodeId,
    ) -> Result<CompileTimeCallTarget, CompileTimeProjectionError> {
        let arguments = target
            .generic_arguments()
            .iter()
            .map(|argument| {
                self.specialization_type(*argument.ty())
                    .map(|ty| CompileTimeGenericArgument::new(argument.parameter(), ty))
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                self.error(Some(node), CompileTimeProjectionRule::UnsupportedValueType)
            })?;
        CompileTimeCallTarget::new(target.callable(), arguments)
            .map_err(|_| self.error(Some(node), CompileTimeProjectionRule::InvalidPlan))
    }

    fn value_type(&self, ty: TypeId) -> Option<CompileTimeValueType> {
        value_type(&self.specialization_type(ty)?)
    }

    fn specialization_type(&self, ty: TypeId) -> Option<CompileTimeType> {
        match self.types.get(ty)? {
            TypeKind::Builtin(builtin) => Some(CompileTimeType::Builtin(*builtin)),
            TypeKind::GenericParameter(parameter) => self.substitution.get(parameter).cloned(),
            TypeKind::Borrow {
                capability,
                referent,
            } => Some(CompileTimeType::Borrow {
                capability: *capability,
                referent: Box::new(self.specialization_type(*referent)?),
            }),
            TypeKind::Tuple(elements) => elements
                .iter()
                .map(|element| self.specialization_type(element))
                .collect::<Option<Vec<_>>>()
                .map(|elements| CompileTimeType::Tuple(elements.into_boxed_slice())),
            TypeKind::FixedArray { element, length } => Some(CompileTimeType::FixedArray {
                element: Box::new(self.specialization_type(*element)?),
                length: *length,
            }),
            _ => None,
        }
    }

    const fn error(
        &self,
        node: Option<nocter_model::BodyNodeId>,
        rule: CompileTimeProjectionRule,
    ) -> CompileTimeProjectionError {
        CompileTimeProjectionError {
            callable: self.callable,
            body: self.body,
            node,
            rule,
        }
    }
}

fn value_type(ty: &CompileTimeType) -> Option<CompileTimeValueType> {
    match ty {
        CompileTimeType::Builtin(BuiltinType::Void) => Some(CompileTimeValueType::Void),
        CompileTimeType::Builtin(BuiltinType::Never) => Some(CompileTimeValueType::Never),
        CompileTimeType::Builtin(BuiltinType::Bool) => {
            Some(CompileTimeValueType::Scalar(ConstantScalarType::Bool))
        }
        CompileTimeType::Builtin(BuiltinType::Char) => {
            Some(CompileTimeValueType::Scalar(ConstantScalarType::Character))
        }
        CompileTimeType::Builtin(BuiltinType::F32) => Some(CompileTimeValueType::Scalar(
            ConstantScalarType::Float(FloatFormat::Binary32),
        )),
        CompileTimeType::Builtin(BuiltinType::F64) => Some(CompileTimeValueType::Scalar(
            ConstantScalarType::Float(FloatFormat::Binary64),
        )),
        CompileTimeType::Builtin(builtin) if integer_builtin(*builtin) => Some(
            CompileTimeValueType::Scalar(ConstantScalarType::Integer(*builtin)),
        ),
        CompileTimeType::Borrow {
            capability: BorrowCapability::Readonly,
            referent,
        } if matches!(
            referent.as_ref(),
            CompileTimeType::Builtin(BuiltinType::Str)
        ) =>
        {
            Some(CompileTimeValueType::Scalar(ConstantScalarType::Text))
        }
        CompileTimeType::Borrow {
            capability: BorrowCapability::Readonly,
            referent,
        } => value_type(referent)
            .map(Box::new)
            .map(CompileTimeValueType::ReadonlyBorrow),
        CompileTimeType::Tuple(elements) => elements
            .iter()
            .map(value_type)
            .collect::<Option<Vec<_>>>()
            .map(|elements| CompileTimeValueType::Tuple(elements.into_boxed_slice())),
        CompileTimeType::FixedArray { element, length } => {
            value_type(element).map(|element| CompileTimeValueType::FixedArray {
                element: Box::new(element),
                length: *length,
            })
        }
        CompileTimeType::Builtin(_) | CompileTimeType::Borrow { .. } => None,
    }
}

impl Projector<'_> {
    const fn error(
        &self,
        node: Option<nocter_model::BodyNodeId>,
        rule: CompileTimeProjectionRule,
    ) -> CompileTimeProjectionError {
        CompileTimeProjectionError {
            callable: self.callable,
            body: Some(self.body_id),
            node,
            rule,
        }
    }
}

fn constant_scalar_type(types: &TypeStore, ty: TypeId) -> Option<ConstantScalarType> {
    match types.get(ty)? {
        TypeKind::Builtin(BuiltinType::Bool) => Some(ConstantScalarType::Bool),
        TypeKind::Builtin(BuiltinType::Char) => Some(ConstantScalarType::Character),
        TypeKind::Builtin(BuiltinType::F32) => {
            Some(ConstantScalarType::Float(FloatFormat::Binary32))
        }
        TypeKind::Builtin(BuiltinType::F64) => {
            Some(ConstantScalarType::Float(FloatFormat::Binary64))
        }
        TypeKind::Builtin(builtin) if integer_builtin(*builtin) => {
            Some(ConstantScalarType::Integer(*builtin))
        }
        _ => None,
    }
}

const fn supported_builtin_value(builtin: BuiltinType) -> bool {
    matches!(
        builtin,
        BuiltinType::Void
            | BuiltinType::Never
            | BuiltinType::Bool
            | BuiltinType::Char
            | BuiltinType::F32
            | BuiltinType::F64
    ) || integer_builtin(builtin)
}

const fn integer_builtin(builtin: BuiltinType) -> bool {
    matches!(
        builtin,
        BuiltinType::I8
            | BuiltinType::I16
            | BuiltinType::I32
            | BuiltinType::I64
            | BuiltinType::Isize
            | BuiltinType::U8
            | BuiltinType::U16
            | BuiltinType::U32
            | BuiltinType::U64
            | BuiltinType::Usize
    )
}

#[cfg(test)]
mod tests {
    use nocter_declaration_lowering::lower_compile_unit_declarations;
    use nocter_model::BuiltinType;

    use nocter_constant_evaluation::{
        CompileTimeCallTarget, CompileTimeGenericArgument, CompileTimeOperation, CompileTimeType,
        CompileTimeValueType, ConstantScalarType,
    };

    use crate::test_support::Fixture;
    use crate::{check_prepared_program, prepare_program_checking};

    fn check(source: &str) -> crate::CheckedProgramOutput {
        let fixture = Fixture::new(source);
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
        let prepared =
            prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
        check_prepared_program(&input, prepared).unwrap()
    }

    fn callable(
        program: &crate::CheckedProgram,
        name: &str,
    ) -> (nocter_model::CallableId, nocter_model::BodyId) {
        let symbol = program.graph().symbols().get(name).unwrap();
        program
            .graph()
            .declarations()
            .callables()
            .iter()
            .find_map(|(id, declaration)| {
                (declaration.name() == Some(symbol)).then(|| (id, declaration.body().unwrap()))
            })
            .unwrap()
    }

    #[test]
    fn ordinary_checked_scalar_body_projects_without_rechecking_source() {
        let output = check(
            "const func increment(value: i32): i32 {\n\
                 if value < 0 { return 0 }\n\
                 let next = value + 1\n\
                 return next\n\
             }\n",
        );
        let program = output.program();
        let (callable, body) = callable(program, "increment");
        assert!(program.bodies().get(body).is_some());
        let target = CompileTimeCallTarget::new(callable, []).unwrap();
        let plan = program.compile_time_plans().get(&target).unwrap();

        assert!(
            plan.nodes()
                .iter()
                .any(|(_, node)| matches!(node.operation(), CompileTimeOperation::Binary { .. }))
        );
        assert!(
            plan.nodes().iter().any(|(_, node)| matches!(
                node.operation(),
                CompileTimeOperation::Comparison { .. }
            ))
        );
        assert_eq!(plan.parameters().len(), 1);
        assert_eq!(plan.locals().len(), 1);
    }

    #[test]
    fn program_finalization_rejects_a_call_without_authored_compile_time_capability() {
        let fixture = Fixture::new(
            "func runtime(value: i32): i32 { return value }\n\
             const func invalid(value: i32): i32 { return runtime(value) }\n",
        );
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
        let prepared =
            prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
        let error = check_prepared_program(&input, prepared).unwrap_err();

        assert_eq!(
            error.rule(),
            Some(crate::BodyRule::InvalidCompileTimeCallable)
        );
        assert_eq!(
            error
                .source_diagnostic()
                .expect("authored compile-time failure")
                .code(),
            "E0421"
        );
    }

    #[test]
    fn runtime_only_callable_has_no_compile_time_plan_slot_value() {
        let output = check("func runtime(value: i32): i32 { return value }\n");
        let program = output.program();
        let (callable, _) = callable(program, "runtime");

        let target = CompileTimeCallTarget::new(callable, []).unwrap();
        assert!(program.compile_time_plans().get(&target).is_none());
        assert!(program.compile_time_plans().is_empty());
    }

    #[test]
    fn a_reachable_generic_callable_projects_once_for_its_closed_specialization() {
        let output = check(
            "const func identity<T>(value: T): T where copy T { return value }\n\
             const func answer(): i32 { return identity(42) }\n",
        );
        let program = output.program();
        let (identity, _) = callable(program, "identity");
        let parameter = program
            .graph()
            .declarations()
            .callable_generic_domain(identity)
            .unwrap()[0];
        let target = CompileTimeCallTarget::new(
            identity,
            [CompileTimeGenericArgument::new(
                parameter,
                CompileTimeType::Builtin(BuiltinType::I32),
            )],
        )
        .unwrap();

        let plan = program.compile_time_plans().get(&target).unwrap();

        assert_eq!(plan.parameters().len(), 1);
        assert_eq!(
            plan.parameters()[0].ty(),
            &CompileTimeValueType::Scalar(ConstantScalarType::Integer(BuiltinType::I32))
        );
        assert_eq!(program.compile_time_plans().len(), 2);
    }

    #[test]
    fn recursive_generic_plan_edges_do_not_become_construction_cycles() {
        let output = check(
            "const func recurse<T>(value: T): T where copy T { return recurse(value) }\n\
             const func start(): i32 { return recurse(1) }\n",
        );

        assert_eq!(output.program().compile_time_plans().len(), 2);
    }

    #[test]
    fn one_generic_recipe_can_produce_multiple_closed_plans() {
        let output = check(
            "const func identity<T>(value: T): T where copy T { return value }\n\
             const func integer(): i32 { return identity(1) }\n\
             const func boolean(): bool { return identity(true) }\n",
        );

        assert_eq!(output.program().compile_time_plans().len(), 4);
    }

    #[test]
    fn an_unreached_generic_body_still_validates_its_compile_time_operations() {
        let fixture = Fixture::new(
            "const func invalid<T>(value: T): T where copy T {\n\
                 var result = value\n\
                 result = value\n\
                 return result\n\
             }\n",
        );
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
        let prepared =
            prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
        let error = check_prepared_program(&input, prepared).unwrap_err();

        assert_eq!(
            error.rule(),
            Some(crate::BodyRule::InvalidCompileTimeCallable)
        );
    }
}
