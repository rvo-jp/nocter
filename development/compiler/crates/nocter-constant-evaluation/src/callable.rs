use nocter_model::{
    Arena, BodyNodeId, BorrowCapability, BuiltinType, CallableId, ConstantId, GenericParameterId,
    LocalBindingId, ParameterId,
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

/// Closed semantic type shape used to identify one callable specialization.
///
/// This domain deliberately distinguishes `str` from `&str`: only the borrow is a value, but the
/// referent can still be a generic argument. It contains no `TypeId`, so a plan identity cannot be
/// paired with the wrong checked-program type store.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompileTimeType {
    Builtin(BuiltinType),
    Borrow {
        capability: BorrowCapability,
        referent: Box<CompileTimeType>,
    },
    Tuple(Box<[CompileTimeType]>),
    FixedArray {
        element: Box<CompileTimeType>,
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

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompileTimeGenericValue<T = CompileTimeType> {
    Type(T),
    Usize(nocter_model::UsizeTerm),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompileTimeGenericArgument<T = CompileTimeType> {
    parameter: GenericParameterId,
    value: CompileTimeGenericValue<T>,
}

impl<T> CompileTimeGenericArgument<T> {
    #[must_use]
    pub const fn new(parameter: GenericParameterId, ty: T) -> Self {
        Self {
            parameter,
            value: CompileTimeGenericValue::Type(ty),
        }
    }

    #[must_use]
    pub const fn from_value(
        parameter: GenericParameterId,
        value: CompileTimeGenericValue<T>,
    ) -> Self {
        Self { parameter, value }
    }

    #[must_use]
    pub const fn parameter(&self) -> GenericParameterId {
        self.parameter
    }

    #[must_use]
    pub const fn value(&self) -> &CompileTimeGenericValue<T> {
        &self.value
    }

    #[must_use]
    pub const fn ty(&self) -> Option<&T> {
        match &self.value {
            CompileTimeGenericValue::Type(ty) => Some(ty),
            CompileTimeGenericValue::Usize(_) => None,
        }
    }
}

/// One already-selected compile-time call target.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompileTimeCallTarget<T = CompileTimeType> {
    callable: CallableId,
    generic_arguments: Box<[CompileTimeGenericArgument<T>]>,
}

impl<T> CompileTimeCallTarget<T> {
    /// Creates a call target with one canonical argument per generic parameter.
    ///
    /// # Errors
    ///
    /// Returns the repeated or out-of-order parameter instead of accepting a call identity whose
    /// meaning depends on caller convention.
    pub fn new(
        callable: CallableId,
        generic_arguments: impl Into<Box<[CompileTimeGenericArgument<T>]>>,
    ) -> Result<Self, InvalidCompileTimeCallTarget> {
        let generic_arguments = generic_arguments.into();
        if let Some(pair) = generic_arguments
            .windows(2)
            .find(|pair| pair[0].parameter() >= pair[1].parameter())
        {
            return Err(InvalidCompileTimeCallTarget {
                parameter: pair[1].parameter(),
            });
        }
        Ok(Self {
            callable,
            generic_arguments,
        })
    }

    #[must_use]
    pub const fn callable(&self) -> CallableId {
        self.callable
    }

    #[must_use]
    pub const fn generic_arguments(&self) -> &[CompileTimeGenericArgument<T>] {
        &self.generic_arguments
    }
}

/// Checked-program-relative call edge retained before closed type specialization.
pub type CompileTimeRecipeCallTarget = CompileTimeCallTarget<nocter_model::TypeId>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidCompileTimeCallTarget {
    parameter: GenericParameterId,
}

impl InvalidCompileTimeCallTarget {
    #[must_use]
    pub const fn parameter(self) -> GenericParameterId {
        self.parameter
    }
}

/// Syntax-independent operation admitted by checked-body compile-time projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileTimeOperation<C = CompileTimeCallTarget, G = u64> {
    Complete,
    Literal(nocter_model::ConstantValue),
    DeclaredConstant(ConstantId),
    /// A specialization value: recipes carry a parameter identity, closed plans carry `u64`.
    GenericConstant(G),
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
        target: C,
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

impl<C, G> CompileTimeOperation<C, G> {
    /// Rebinds both representation-specific edges while preserving the checked operation.
    ///
    /// This is the exhaustive recipe-to-plan boundary. Adding an operation variant therefore
    /// cannot silently leave specialization with a second partial operation model.
    ///
    /// # Errors
    ///
    /// Returns either mapper's failure without publishing a partially rebound operation.
    pub fn try_map_edges<D, H, E>(
        self,
        mut map_call: impl FnMut(C) -> Result<D, E>,
        mut map_generic: impl FnMut(G) -> Result<H, E>,
    ) -> Result<CompileTimeOperation<D, H>, E> {
        Ok(match self {
            Self::Complete => CompileTimeOperation::Complete,
            Self::Literal(value) => CompileTimeOperation::Literal(value),
            Self::DeclaredConstant(id) => CompileTimeOperation::DeclaredConstant(id),
            Self::GenericConstant(parameter) => {
                CompileTimeOperation::GenericConstant(map_generic(parameter)?)
            }
            Self::ReadParameter(parameter) => CompileTimeOperation::ReadParameter(parameter),
            Self::ReadLocal(local) => CompileTimeOperation::ReadLocal(local),
            Self::Unary { operation, operand } => {
                CompileTimeOperation::Unary { operation, operand }
            }
            Self::Binary {
                operation,
                left,
                right,
            } => CompileTimeOperation::Binary {
                operation,
                left,
                right,
            },
            Self::NumericConversion { operand, target } => {
                CompileTimeOperation::NumericConversion { operand, target }
            }
            Self::Comparison {
                operation,
                left,
                right,
            } => CompileTimeOperation::Comparison {
                operation,
                left,
                right,
            },
            Self::Tuple(elements) => CompileTimeOperation::Tuple(elements),
            Self::FixedArray(elements) => CompileTimeOperation::FixedArray(elements),
            Self::Call {
                target,
                receiver,
                arguments,
            } => CompileTimeOperation::Call {
                target: map_call(target)?,
                receiver,
                arguments,
            },
            Self::Block { statements, result } => {
                CompileTimeOperation::Block { statements, result }
            }
            Self::Bind {
                binding,
                initializer,
            } => CompileTimeOperation::Bind {
                binding,
                initializer,
            },
            Self::Discard(value) => CompileTimeOperation::Discard(value),
            Self::Unreachable => CompileTimeOperation::Unreachable,
            Self::Return(value) => CompileTimeOperation::Return(value),
            Self::If {
                condition,
                then_branch,
                else_branch,
            } => CompileTimeOperation::If {
                condition,
                then_branch,
                else_branch,
            },
            Self::Logical {
                operation,
                left,
                right,
            } => CompileTimeOperation::Logical {
                operation,
                left,
                right,
            },
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileTimeNode<T = CompileTimeValueType, C = CompileTimeCallTarget, G = u64> {
    ty: T,
    operation: CompileTimeOperation<C, G>,
}

impl<T, C, G> CompileTimeNode<T, C, G> {
    #[must_use]
    pub const fn new(ty: T, operation: CompileTimeOperation<C, G>) -> Self {
        Self { ty, operation }
    }

    #[must_use]
    pub const fn ty(&self) -> &T {
        &self.ty
    }

    #[must_use]
    pub const fn operation(&self) -> &CompileTimeOperation<C, G> {
        &self.operation
    }
}

/// One positional input and its value shape in a compile-time callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileTimeParameter<T> {
    id: ParameterId,
    ty: T,
}

impl<T> CompileTimeParameter<T> {
    #[must_use]
    pub const fn new(id: ParameterId, ty: T) -> Self {
        Self { id, ty }
    }

    #[must_use]
    pub const fn id(&self) -> ParameterId {
        self.id
    }

    #[must_use]
    pub const fn ty(&self) -> &T {
        &self.ty
    }
}

/// One ordinary checked body lowered into a compile-time operation domain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileTimeCallable<T, C, G = u64> {
    parameters: Box<[CompileTimeParameter<T>]>,
    result: T,
    locals: Arena<LocalBindingId, T>,
    nodes: Arena<BodyNodeId, CompileTimeNode<T, C, G>>,
    root: BodyNodeId,
    constant_dependencies: Box<[ConstantId]>,
    call_dependencies: Box<[C]>,
}

/// One checked body recipe before its generic type domain is closed.
pub type CompileTimeCallableRecipe =
    CompileTimeCallable<nocter_model::TypeId, CompileTimeRecipeCallTarget, GenericParameterId>;

/// One checked body plan after its complete generic type domain is closed.
pub type CompileTimeCallablePlan = CompileTimeCallable<CompileTimeValueType, CompileTimeCallTarget>;

impl<T, C, G> CompileTimeCallable<T, C, G> {
    /// Builds a recipe or plan whose identities remain in one canonical body-node domain.
    ///
    /// # Errors
    ///
    /// Returns the first missing node, parameter, or local reference. No partially valid plan is
    /// published.
    pub fn new(
        parameters: impl Into<Box<[CompileTimeParameter<T>]>>,
        result: T,
        locals: Arena<LocalBindingId, T>,
        nodes: Arena<BodyNodeId, CompileTimeNode<T, C, G>>,
        root: BodyNodeId,
    ) -> Result<Self, InvalidCompileTimeCallable>
    where
        T: PartialEq,
        C: Clone + Ord,
    {
        let mut constant_dependencies = nodes
            .iter()
            .filter_map(|(_, node)| match node.operation() {
                CompileTimeOperation::DeclaredConstant(id) => Some(*id),
                _ => None,
            })
            .collect::<Vec<_>>();
        constant_dependencies.sort_unstable();
        constant_dependencies.dedup();
        let mut call_dependencies = nodes
            .iter()
            .filter_map(|(_, node)| match node.operation() {
                CompileTimeOperation::Call { target, .. } => Some(target.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        call_dependencies.sort_unstable();
        call_dependencies.dedup();
        let plan = Self {
            parameters: parameters.into(),
            result,
            locals,
            nodes,
            root,
            constant_dependencies: constant_dependencies.into_boxed_slice(),
            call_dependencies: call_dependencies.into_boxed_slice(),
        };
        if let Some(parameter) =
            plan.parameters
                .iter()
                .enumerate()
                .find_map(|(index, parameter)| {
                    plan.parameters[..index]
                        .iter()
                        .any(|previous| previous.id() == parameter.id())
                        .then_some(parameter.id())
                })
        {
            return Err(InvalidCompileTimeCallable::DuplicateParameter(parameter));
        }
        plan.validate()?;
        Ok(plan)
    }

    #[must_use]
    pub const fn parameters(&self) -> &[CompileTimeParameter<T>] {
        &self.parameters
    }

    #[must_use]
    pub const fn result(&self) -> &T {
        &self.result
    }

    #[must_use]
    pub const fn locals(&self) -> &Arena<LocalBindingId, T> {
        &self.locals
    }

    #[must_use]
    pub const fn nodes(&self) -> &Arena<BodyNodeId, CompileTimeNode<T, C, G>> {
        &self.nodes
    }

    #[must_use]
    pub const fn root(&self) -> BodyNodeId {
        self.root
    }

    /// Returns the canonical direct constant inputs read by this plan.
    ///
    /// The list is derived and frozen at construction, so dependency queries do not need to scan
    /// operation nodes or maintain a second interpretation of the operation domain.
    #[must_use]
    pub const fn constant_dependencies(&self) -> &[ConstantId] {
        &self.constant_dependencies
    }

    /// Returns the canonical direct call inputs selected by this plan.
    ///
    /// Call-graph consumers use this frozen edge set rather than rediscovering calls from
    /// operation nodes.
    #[must_use]
    pub const fn call_dependencies(&self) -> &[C] {
        &self.call_dependencies
    }

    fn validate(&self) -> Result<(), InvalidCompileTimeCallable>
    where
        T: PartialEq,
    {
        self.require_node(self.root)?;
        for (node_id, node) in self.nodes.iter() {
            match node.operation() {
                CompileTimeOperation::ReadParameter(parameter) => {
                    let Some(parameter) = self
                        .parameters
                        .iter()
                        .find(|candidate| candidate.id() == *parameter)
                    else {
                        return Err(InvalidCompileTimeCallable::MissingParameter(*parameter));
                    };
                    if parameter.ty() != node.ty() {
                        return Err(InvalidCompileTimeCallable::TypeMismatch(node_id));
                    }
                }
                CompileTimeOperation::ReadLocal(local) => {
                    self.require_local(*local)?;
                    if self.locals.get(*local) != Some(node.ty()) {
                        return Err(InvalidCompileTimeCallable::TypeMismatch(node_id));
                    }
                }
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
                    self.require_node(*initializer)?;
                    if let Some(binding) = binding {
                        self.require_local(*binding)?;
                        if self.locals.get(*binding)
                            != self.nodes.get(*initializer).map(CompileTimeNode::ty)
                        {
                            return Err(InvalidCompileTimeCallable::TypeMismatch(node_id));
                        }
                    }
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
                | CompileTimeOperation::Literal(_)
                | CompileTimeOperation::DeclaredConstant(_)
                | CompileTimeOperation::GenericConstant(_)
                | CompileTimeOperation::Unreachable => {}
            }
        }
        Ok(())
    }

    fn require_node(&self, node: BodyNodeId) -> Result<(), InvalidCompileTimeCallable> {
        self.nodes
            .get(node)
            .map(|_| ())
            .ok_or(InvalidCompileTimeCallable::MissingNode(node))
    }

    fn require_local(&self, local: LocalBindingId) -> Result<(), InvalidCompileTimeCallable> {
        self.locals
            .get(local)
            .map(|_| ())
            .ok_or(InvalidCompileTimeCallable::MissingLocal(local))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidCompileTimeCallable {
    MissingNode(BodyNodeId),
    MissingParameter(ParameterId),
    DuplicateParameter(ParameterId),
    MissingLocal(LocalBindingId),
    TypeMismatch(BodyNodeId),
}

#[cfg(test)]
mod tests {
    use nocter_model::{Arena, ArenaBuilder};

    use super::{
        CompileTimeCallablePlan, CompileTimeNode, CompileTimeOperation, CompileTimeParameter,
        CompileTimeValueType,
    };

    #[test]
    fn construction_freezes_sorted_unique_constant_dependencies() {
        let mut constant_ids = ArenaBuilder::new();
        let first = constant_ids.insert(());
        let second = constant_ids.insert(());
        let mut nodes = ArenaBuilder::new();
        let second_read = nodes.insert(CompileTimeNode::new(
            CompileTimeValueType::Never,
            CompileTimeOperation::DeclaredConstant(second),
        ));
        nodes.insert(CompileTimeNode::new(
            CompileTimeValueType::Never,
            CompileTimeOperation::DeclaredConstant(first),
        ));
        nodes.insert(CompileTimeNode::new(
            CompileTimeValueType::Never,
            CompileTimeOperation::DeclaredConstant(second),
        ));
        let plan = CompileTimeCallablePlan::new(
            Vec::<CompileTimeParameter<CompileTimeValueType>>::new(),
            CompileTimeValueType::Never,
            Arena::default(),
            nodes.finish(),
            second_read,
        )
        .unwrap();

        assert_eq!(plan.constant_dependencies(), &[first, second]);
    }
}
