use nocter_model::{BuiltinType, ConstantId, ConstantValue};
use nocter_syntax::SyntaxOrigin;
use nocter_syntax::{NodeId, Punctuation};

use crate::ConstantExpressionRule;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ConstantScalarType {
    Bool,
    Character,
    Integer(BuiltinType),
    Text,
}

/// The closed recursive type domain admitted for immutable static initializers.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum FrozenType {
    Scalar(ConstantScalarType),
    FixedArray {
        element: Box<FrozenType>,
        length: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConstantReference {
    id: ConstantId,
    ty: ConstantScalarType,
}

impl ConstantReference {
    #[must_use]
    pub const fn new(id: ConstantId, ty: ConstantScalarType) -> Self {
        Self { id, ty }
    }

    #[must_use]
    pub const fn id(self) -> ConstantId {
        self.id
    }

    #[must_use]
    pub const fn ty(self) -> ConstantScalarType {
        self.ty
    }
}

/// Caller-owned name and type decisions required to close a constant-expression plan.
pub trait ConstantResolver {
    type Error;

    /// Resolves one syntactic constant reference to its frozen semantic identity and scalar type.
    ///
    /// # Errors
    ///
    /// Returns the caller context's name-resolution or semantic-access failure.
    fn resolve_constant(&mut self, node: NodeId) -> Result<ConstantReference, Self::Error>;

    /// Resolves one conversion target through the caller context's ordinary type authority.
    ///
    /// # Errors
    ///
    /// Returns the caller context's type-resolution failure. `Ok(None)` denotes a resolved type
    /// outside the scalar constant domain.
    fn resolve_type(&mut self, node: NodeId) -> Result<Option<ConstantScalarType>, Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstantPlanRule {
    NonConstantExpression,
    TypeMismatch,
}

#[derive(Debug)]
pub enum ConstantPlanError<E> {
    Rule {
        rule: ConstantPlanRule,
        origin: SyntaxOrigin,
    },
    Context(E),
    InvalidSyntax(NodeId),
}

impl<E> ConstantPlanError<E> {
    #[must_use]
    pub const fn expression_rule(&self) -> Option<ConstantExpressionRule> {
        match self {
            Self::Rule {
                rule: ConstantPlanRule::NonConstantExpression,
                ..
            } => Some(ConstantExpressionRule::NonConstantExpression),
            Self::Rule {
                rule: ConstantPlanRule::TypeMismatch,
                ..
            } => Some(ConstantExpressionRule::TypeMismatch),
            Self::Context(_) | Self::InvalidSyntax(_) => None,
        }
    }

    #[must_use]
    pub const fn rule(&self) -> Option<ConstantPlanRule> {
        match self {
            Self::Rule { rule, .. } => Some(*rule),
            Self::Context(_) | Self::InvalidSyntax(_) => None,
        }
    }

    #[must_use]
    pub const fn origin(&self) -> Option<SyntaxOrigin> {
        match self {
            Self::Rule { origin, .. } => Some(*origin),
            Self::Context(_) | Self::InvalidSyntax(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PlanNodeId(pub(crate) usize);

#[derive(Clone, Debug)]
pub(crate) struct PlanNode {
    pub(crate) ty: ConstantScalarType,
    pub(crate) origin: SyntaxOrigin,
    pub(crate) operation: ConstantOperation,
}

#[derive(Clone, Debug)]
pub(crate) enum ConstantOperation {
    Value(ConstantValue),
    IntegerLiteral(u64),
    Reference(ConstantId),
    Unary {
        operator: Punctuation,
        operand: PlanNodeId,
    },
    Binary {
        operator: Punctuation,
        left: PlanNodeId,
        right: PlanNodeId,
    },
    Conversion {
        operand: PlanNodeId,
    },
}

#[derive(Clone, Debug)]
pub struct ConstantExpressionPlan {
    pub(crate) nodes: Vec<PlanNode>,
    pub(crate) root: PlanNodeId,
}

/// One syntax-independent plan for a recursively typed immutable static initializer.
#[derive(Clone, Debug)]
pub enum FrozenExpressionPlan {
    Scalar(ConstantExpressionPlan),
    FixedArray {
        ty: FrozenType,
        elements: Box<[FrozenExpressionPlan]>,
    },
}

impl FrozenExpressionPlan {
    #[must_use]
    pub fn result_type(&self) -> FrozenType {
        match self {
            Self::Scalar(plan) => FrozenType::Scalar(plan.result_type()),
            Self::FixedArray { ty, .. } => ty.clone(),
        }
    }
}

impl ConstantExpressionPlan {
    pub(crate) fn references(&self) -> impl Iterator<Item = (ConstantId, SyntaxOrigin)> + '_ {
        self.nodes.iter().filter_map(|node| match node.operation {
            ConstantOperation::Reference(id) => Some((id, node.origin)),
            _ => None,
        })
    }

    #[must_use]
    pub fn result_type(&self) -> ConstantScalarType {
        self.nodes[self.root.0].ty
    }
}
