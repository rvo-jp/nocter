use nocter_model::{
    Arena, BodyNodeId, CallableId, GenericParameterId, LocalBindingId, ParameterId, TypeId,
};

use crate::ConstantScalarType;

/// Closed value shape carried by a compile-time callable plan.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CompileTimeValueType {
    Void,
    Never,
    Scalar(ConstantScalarType),
    ReadonlyBorrow(Box<CompileTimeValueType>),
    Tuple(Box<[CompileTimeValueType]>),
    FixedArray {
        element: Box<CompileTimeValueType>,
        length: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CompileTimeUnaryOperation {
    LogicalNot,
    Negate,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CompileTimeBinaryOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    ShiftLeft,
    ShiftRightSigned,
    ShiftRightUnsigned,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CompileTimeLogicalOperation {
    And,
    Or,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CompileTimeComparisonOperation {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CompileTimeGenericArgument {
    parameter: GenericParameterId,
    ty: TypeId,
}

impl CompileTimeGenericArgument {
    #[must_use]
    pub const fn new(parameter: GenericParameterId, ty: TypeId) -> Self {
        Self { parameter, ty }
    }

    #[must_use]
    pub const fn parameter(self) -> GenericParameterId {
        self.parameter
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }
}

/// One already-selected compile-time call target.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CompileTimeCallTarget {
    callable: CallableId,
    generic_arguments: Box<[CompileTimeGenericArgument]>,
}

impl CompileTimeCallTarget {
    #[must_use]
    pub fn new(
        callable: CallableId,
        generic_arguments: impl Into<Box<[CompileTimeGenericArgument]>>,
    ) -> Self {
        Self {
            callable,
            generic_arguments: generic_arguments.into(),
        }
    }

    #[must_use]
    pub const fn callable(&self) -> CallableId {
        self.callable
    }

    #[must_use]
    pub const fn generic_arguments(&self) -> &[CompileTimeGenericArgument] {
        &self.generic_arguments
    }
}

/// Syntax-independent operation admitted by checked-body compile-time projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileTimeOperation {
    Complete,
    Constant(nocter_model::ConstantValue),
    ReadParameter(ParameterId),
    ReadLocal(LocalBindingId),
    Unary {
        operation: CompileTimeUnaryOperation,
        operand: BodyNodeId,
    },
    Binary {
        operation: CompileTimeBinaryOperation,
        left: BodyNodeId,
        right: BodyNodeId,
    },
    NumericConversion {
        operand: BodyNodeId,
        target: ConstantScalarType,
    },
    Comparison {
        operation: CompileTimeComparisonOperation,
        left: BodyNodeId,
        right: BodyNodeId,
    },
    Tuple(Box<[BodyNodeId]>),
    FixedArray(Box<[BodyNodeId]>),
    Call {
        target: CompileTimeCallTarget,
        receiver: Option<BodyNodeId>,
        arguments: Box<[BodyNodeId]>,
    },
    Block {
        statements: Box<[BodyNodeId]>,
        result: Option<BodyNodeId>,
    },
    Bind {
        binding: Option<LocalBindingId>,
        initializer: BodyNodeId,
    },
    Discard(BodyNodeId),
    Unreachable,
    Return(Option<BodyNodeId>),
    If {
        condition: BodyNodeId,
        then_branch: BodyNodeId,
        else_branch: Option<BodyNodeId>,
    },
    Logical {
        operation: CompileTimeLogicalOperation,
        left: BodyNodeId,
        right: BodyNodeId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileTimeNode {
    ty: CompileTimeValueType,
    operation: CompileTimeOperation,
}

impl CompileTimeNode {
    #[must_use]
    pub const fn new(ty: CompileTimeValueType, operation: CompileTimeOperation) -> Self {
        Self { ty, operation }
    }

    #[must_use]
    pub const fn ty(&self) -> &CompileTimeValueType {
        &self.ty
    }

    #[must_use]
    pub const fn operation(&self) -> &CompileTimeOperation {
        &self.operation
    }
}

/// One ordinary checked body lowered into the closed compile-time operation domain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileTimeCallablePlan {
    parameters: Box<[ParameterId]>,
    locals: Arena<LocalBindingId, CompileTimeValueType>,
    nodes: Arena<BodyNodeId, CompileTimeNode>,
    root: BodyNodeId,
}

impl CompileTimeCallablePlan {
    /// Builds a plan whose identities remain in the checked body's canonical domains.
    ///
    /// # Errors
    ///
    /// Returns the first missing node, parameter, or local reference. No partially valid plan is
    /// published.
    pub fn new(
        parameters: impl Into<Box<[ParameterId]>>,
        locals: Arena<LocalBindingId, CompileTimeValueType>,
        nodes: Arena<BodyNodeId, CompileTimeNode>,
        root: BodyNodeId,
    ) -> Result<Self, InvalidCompileTimeCallablePlan> {
        let plan = Self {
            parameters: parameters.into(),
            locals,
            nodes,
            root,
        };
        plan.validate()?;
        Ok(plan)
    }

    #[must_use]
    pub const fn parameters(&self) -> &[ParameterId] {
        &self.parameters
    }

    #[must_use]
    pub const fn locals(&self) -> &Arena<LocalBindingId, CompileTimeValueType> {
        &self.locals
    }

    #[must_use]
    pub const fn nodes(&self) -> &Arena<BodyNodeId, CompileTimeNode> {
        &self.nodes
    }

    #[must_use]
    pub const fn root(&self) -> BodyNodeId {
        self.root
    }

    fn validate(&self) -> Result<(), InvalidCompileTimeCallablePlan> {
        self.require_node(self.root)?;
        for (_, node) in self.nodes.iter() {
            match node.operation() {
                CompileTimeOperation::ReadParameter(parameter) => {
                    if !self.parameters.contains(parameter) {
                        return Err(InvalidCompileTimeCallablePlan::MissingParameter(*parameter));
                    }
                }
                CompileTimeOperation::ReadLocal(local) => self.require_local(*local)?,
                CompileTimeOperation::Unary { operand, .. }
                | CompileTimeOperation::NumericConversion { operand, .. }
                | CompileTimeOperation::Discard(operand) => self.require_node(*operand)?,
                CompileTimeOperation::Binary { left, right, .. }
                | CompileTimeOperation::Comparison { left, right, .. }
                | CompileTimeOperation::Logical { left, right, .. } => {
                    self.require_node(*left)?;
                    self.require_node(*right)?;
                }
                CompileTimeOperation::Tuple(elements)
                | CompileTimeOperation::FixedArray(elements) => {
                    for element in elements {
                        self.require_node(*element)?;
                    }
                }
                CompileTimeOperation::Call {
                    receiver,
                    arguments,
                    ..
                } => {
                    if let Some(receiver) = receiver {
                        self.require_node(*receiver)?;
                    }
                    for argument in arguments {
                        self.require_node(*argument)?;
                    }
                }
                CompileTimeOperation::Block { statements, result } => {
                    for statement in statements {
                        self.require_node(*statement)?;
                    }
                    if let Some(result) = result {
                        self.require_node(*result)?;
                    }
                }
                CompileTimeOperation::Bind {
                    binding,
                    initializer,
                } => {
                    if let Some(binding) = binding {
                        self.require_local(*binding)?;
                    }
                    self.require_node(*initializer)?;
                }
                CompileTimeOperation::Return(value) => {
                    if let Some(value) = value {
                        self.require_node(*value)?;
                    }
                }
                CompileTimeOperation::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    self.require_node(*condition)?;
                    self.require_node(*then_branch)?;
                    if let Some(else_branch) = else_branch {
                        self.require_node(*else_branch)?;
                    }
                }
                CompileTimeOperation::Complete
                | CompileTimeOperation::Constant(_)
                | CompileTimeOperation::Unreachable => {}
            }
        }
        Ok(())
    }

    fn require_node(&self, node: BodyNodeId) -> Result<(), InvalidCompileTimeCallablePlan> {
        self.nodes
            .get(node)
            .map(|_| ())
            .ok_or(InvalidCompileTimeCallablePlan::MissingNode(node))
    }

    fn require_local(&self, local: LocalBindingId) -> Result<(), InvalidCompileTimeCallablePlan> {
        self.locals
            .get(local)
            .map(|_| ())
            .ok_or(InvalidCompileTimeCallablePlan::MissingLocal(local))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidCompileTimeCallablePlan {
    MissingNode(BodyNodeId),
    MissingParameter(ParameterId),
    MissingLocal(LocalBindingId),
}
