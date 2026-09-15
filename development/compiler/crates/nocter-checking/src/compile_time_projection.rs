use nocter_constant_evaluation::{
    CompileTimeBinaryOperation, CompileTimeCallTarget, CompileTimeCallablePlan,
    CompileTimeComparisonOperation, CompileTimeGenericArgument, CompileTimeLogicalOperation,
    CompileTimeNode, CompileTimeOperation, CompileTimePlanTable, CompileTimeUnaryOperation,
    CompileTimeValueType, ConstantScalarType, FloatFormat, InvalidCompileTimeCallablePlan,
};
use nocter_declarations::DeclarationGraph;
use nocter_model::{
    Arena, BorrowCapability, BuiltinType, CallableId, CompileTimeGuarantee, TypeId, TypeKind,
    TypeStore,
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

/// Builds the only compile-time plan authority for one checked program generation.
///
/// This operation performs no lookup, inference, overload selection, or type checking. It accepts
/// only closed decisions already present in canonical checked bodies. Runtime-only declarations
/// retain empty identity slots, while authored compile-time declarations with bodies must project
/// completely or reject program finalization.
///
/// # Errors
///
/// Returns the exact unsupported operation or invalid plan edge without publishing a partial
/// table.
pub(crate) fn build_compile_time_plan_table(
    graph: &DeclarationGraph,
    types: &TypeStore,
    bodies: &Arena<nocter_model::BodyId, CheckedBody>,
) -> Result<CompileTimePlanTable, CompileTimeProjectionError> {
    let plans = graph
        .declarations()
        .callables()
        .try_map(|callable, declaration| {
            if declaration.guarantees().compile_time() != CompileTimeGuarantee::Evaluatable {
                return Ok(None);
            }
            let Some(body_id) = declaration.body() else {
                // Interface requirements have no executable body of their own. A concrete static
                // implementation is selected before any call can enter a compile-time plan.
                return Ok(None);
            };
            let body = bodies.get(body_id).ok_or(CompileTimeProjectionError {
                callable,
                body: Some(body_id),
                node: None,
                rule: CompileTimeProjectionRule::InvalidPlan,
            })?;
            project_compile_time_callable(graph, types, callable, body_id, body).map(Some)
        })?;
    CompileTimePlanTable::new(plans).map_err(|error| CompileTimeProjectionError {
        callable: error.caller(),
        body: graph
            .declarations()
            .callables()
            .get(error.caller())
            .and_then(nocter_declarations::CallableDeclaration::body),
        node: Some(error.node()),
        rule: CompileTimeProjectionRule::UnavailableCallTarget,
    })
}

fn project_compile_time_callable(
    graph: &DeclarationGraph,
    types: &TypeStore,
    callable: CallableId,
    body_id: nocter_model::BodyId,
    body: &CheckedBody,
) -> Result<CompileTimeCallablePlan, CompileTimeProjectionError> {
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
        projector
            .value_type(local.ty())
            .ok_or_else(|| projector.error(None, CompileTimeProjectionRule::UnsupportedValueType))
    })?;
    let nodes = body.nodes().try_map(|node, checked| {
        let ty = projector.value_type(checked.ty()).ok_or_else(|| {
            projector.error(Some(node), CompileTimeProjectionRule::UnsupportedValueType)
        })?;
        let operation = projector.operation(node, checked.operation())?;
        Ok(CompileTimeNode::new(ty, operation))
    })?;
    let parameters = declaration
        .receiver()
        .into_iter()
        .chain(declaration.parameters().iter().copied())
        .collect::<Vec<_>>();
    CompileTimeCallablePlan::new(parameters, locals, nodes, body.root()).map_err(|error| {
        let node = match error {
            InvalidCompileTimeCallablePlan::MissingNode(node) => Some(node),
            InvalidCompileTimeCallablePlan::MissingParameter(_)
            | InvalidCompileTimeCallablePlan::DuplicateParameter(_)
            | InvalidCompileTimeCallablePlan::MissingLocal(_) => None,
        };
        projector.error(node, CompileTimeProjectionRule::InvalidPlan)
    })
}

impl Projector<'_> {
    fn operation(
        &self,
        node: nocter_model::BodyNodeId,
        operation: &CheckedOperation,
    ) -> Result<CompileTimeOperation, CompileTimeProjectionError> {
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
    ) -> Result<CompileTimeOperation, CompileTimeProjectionError> {
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
    ) -> Result<CompileTimeOperation, CompileTimeProjectionError> {
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
    ) -> Result<CompileTimeOperation, CompileTimeProjectionError> {
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
                let Some(CompileTimeValueType::Scalar(target)) = self.value_type(*target) else {
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
    ) -> Result<CompileTimeOperation, CompileTimeProjectionError> {
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
            .map(|argument| CompileTimeGenericArgument::new(argument.parameter(), argument.ty()));
        let target = CompileTimeCallTarget::new(callee, generic_arguments.collect::<Vec<_>>())
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
    ) -> Result<CompileTimeOperation, CompileTimeProjectionError> {
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

    fn value_type(&self, ty: TypeId) -> Option<CompileTimeValueType> {
        match self.types.get(ty)? {
            TypeKind::Builtin(BuiltinType::Void) => Some(CompileTimeValueType::Void),
            TypeKind::Builtin(BuiltinType::Never) => Some(CompileTimeValueType::Never),
            TypeKind::Builtin(BuiltinType::Bool) => {
                Some(CompileTimeValueType::Scalar(ConstantScalarType::Bool))
            }
            TypeKind::Builtin(BuiltinType::Char) => {
                Some(CompileTimeValueType::Scalar(ConstantScalarType::Character))
            }
            TypeKind::Builtin(BuiltinType::F32) => Some(CompileTimeValueType::Scalar(
                ConstantScalarType::Float(FloatFormat::Binary32),
            )),
            TypeKind::Builtin(BuiltinType::F64) => Some(CompileTimeValueType::Scalar(
                ConstantScalarType::Float(FloatFormat::Binary64),
            )),
            TypeKind::Builtin(builtin) if integer_builtin(*builtin) => Some(
                CompileTimeValueType::Scalar(ConstantScalarType::Integer(*builtin)),
            ),
            TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent,
            } if matches!(
                self.types.get(*referent),
                Some(TypeKind::Builtin(BuiltinType::Str))
            ) =>
            {
                Some(CompileTimeValueType::Scalar(ConstantScalarType::Text))
            }
            TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent,
            } => self
                .value_type(*referent)
                .map(Box::new)
                .map(CompileTimeValueType::ReadonlyBorrow),
            TypeKind::Tuple(elements) => elements
                .iter()
                .map(|element| self.value_type(element))
                .collect::<Option<Vec<_>>>()
                .map(|elements| CompileTimeValueType::Tuple(elements.into_boxed_slice())),
            TypeKind::FixedArray { element, length } => {
                self.value_type(*element)
                    .map(|element| CompileTimeValueType::FixedArray {
                        element: Box::new(element),
                        length: *length,
                    })
            }
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
            body: Some(self.body_id),
            node,
            rule,
        }
    }
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

    use nocter_constant_evaluation::CompileTimeOperation;

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
        let plan = program.compile_time_plans().get(callable).unwrap();

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

        assert!(program.compile_time_plans().get(callable).is_none());
        assert_eq!(
            program.compile_time_plans().len(),
            program.graph().declarations().callables().len()
        );
    }
}
