use nocter_constant_evaluation::{
    ConstantEvaluationError, ConstantEvaluationRule, ConstantPlanError, ConstantPlanRule,
    ConstantReference, ConstantResolver, ConstantScalarType, evaluate_expression_plan,
    plan_expression,
};
use nocter_declarations::ExportedEntity;
use nocter_diagnostics::SourceDiagnostic;
use nocter_model::{BorrowCapability, BuiltinType, ConstantValue, TypeId, TypeKind};
use nocter_source_index::{SourceOrigin, SyntaxOrigin};
use nocter_syntax::{NodeId, NodeKind};

use super::BodyChecker;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::syntax::{direct_identifier, direct_nodes};
use crate::{ConstantExpressionRule, NameTarget};

struct BodyConstantResolver<'checker, 'input, 'syntax> {
    checker: &'checker mut BodyChecker<'input, 'syntax>,
}

impl BodyChecker<'_, '_> {
    /// Evaluates one fixed-array length in the lexical scope that owns the annotation.
    ///
    /// Body name resolution has already frozen every lexical reference. This adapter consumes
    /// those decisions and supplies them to the phase-neutral constant planner; it never retries
    /// source or module lookup on its own.
    pub(super) fn evaluate_array_length(
        &mut self,
        expression: NodeId,
    ) -> Result<u64, BodyCheckError> {
        // The planner calls back into this checker for conversion types. Owning this immutable
        // syntax snapshot avoids manufacturing a second body-type resolver for that callback.
        let source = self
            .input
            .sources()
            .get(expression.source())
            .cloned()
            .ok_or(BodyCheckInternalError::InvalidSyntax(expression))?;
        let tree = self.tree().clone();
        let plan = {
            let mut resolver = BodyConstantResolver { checker: self };
            plan_expression(
                &source,
                &tree,
                expression,
                ConstantScalarType::Integer(BuiltinType::Usize),
                &mut resolver,
            )
            .map_err(|error| plan_error(&tree, error))?
        };
        let value = evaluate_expression_plan(&plan, |id| {
            self.graph
                .declarations()
                .constants()
                .get(id)
                .map(|constant| constant.value().clone())
        })
        .map_err(|error| evaluation_error(&tree, error))?;
        let ConstantValue::Integer(value) = value else {
            return Err(constant_error(
                &tree,
                ConstantExpressionRule::TypeMismatch,
                SyntaxOrigin::Node(expression),
            ));
        };
        u64::try_from(value).map_err(|_| {
            constant_error(
                &tree,
                ConstantExpressionRule::ArithmeticFailure,
                SyntaxOrigin::Node(expression),
            )
        })
    }
}

impl ConstantResolver for BodyConstantResolver<'_, '_, '_> {
    type Error = BodyCheckError;

    fn resolve_constant(&mut self, node: NodeId) -> Result<ConstantReference, Self::Error> {
        let entity = self.resolve_entity(node)?;
        let ExportedEntity::Constant(id) = entity else {
            return Err(constant_error(
                self.checker.tree(),
                ConstantExpressionRule::NonConstantExpression,
                SyntaxOrigin::Node(node),
            ));
        };
        let constant = self
            .checker
            .graph
            .declarations()
            .constants()
            .get(id)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let ty = scalar_type(self.checker.types, constant.ty())
            .ok_or(BodyCheckInternalError::UnknownType(constant.ty()))?;
        Ok(ConstantReference::new(id, ty))
    }

    fn resolve_type(&mut self, node: NodeId) -> Result<Option<ConstantScalarType>, Self::Error> {
        let ty = self.checker.resolve_type_use(node)?;
        Ok(scalar_type(self.checker.types, ty))
    }
}

impl BodyConstantResolver<'_, '_, '_> {
    fn resolve_entity(&mut self, node: NodeId) -> Result<ExportedEntity, BodyCheckError> {
        match self.checker.kind(node)? {
            NodeKind::ReferenceExpression => {
                let token = direct_identifier(self.checker.tree(), node)
                    .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
                let origin = SyntaxOrigin::Token(token);
                let target = self
                    .checker
                    .uses
                    .get(&origin)
                    .copied()
                    .ok_or(BodyCheckInternalError::MissingNameUse(node))?;
                self.checker.consumed_uses.insert(origin);
                let NameTarget::Exported(entity) = target else {
                    return Err(constant_error(
                        self.checker.tree(),
                        ConstantExpressionRule::NonConstantExpression,
                        origin,
                    ));
                };
                Ok(entity)
            }
            NodeKind::PostfixExpression => {
                let children = direct_nodes(self.checker.tree(), node);
                let [base, suffix] = children.as_slice() else {
                    return Err(BodyCheckInternalError::InvalidSyntax(node).into());
                };
                if self.checker.kind(*suffix)? != NodeKind::MemberSuffix {
                    return Err(constant_error(
                        self.checker.tree(),
                        ConstantExpressionRule::NonConstantExpression,
                        SyntaxOrigin::Node(node),
                    ));
                }
                let ExportedEntity::Module(_) = self.resolve_entity(*base)? else {
                    return Err(constant_error(
                        self.checker.tree(),
                        ConstantExpressionRule::NonConstantExpression,
                        SyntaxOrigin::Node(node),
                    ));
                };
                let token = direct_identifier(self.checker.tree(), *suffix)
                    .ok_or(BodyCheckInternalError::InvalidSyntax(*suffix))?;
                let NameTarget::Exported(entity) = self.checker.consume_name_use(*suffix, token)?
                else {
                    return Err(constant_error(
                        self.checker.tree(),
                        ConstantExpressionRule::NonConstantExpression,
                        SyntaxOrigin::Token(token),
                    ));
                };
                Ok(entity)
            }
            _ => Err(constant_error(
                self.checker.tree(),
                ConstantExpressionRule::NonConstantExpression,
                SyntaxOrigin::Node(node),
            )),
        }
    }
}

fn scalar_type(types: &nocter_model::TypeStore, ty: TypeId) -> Option<ConstantScalarType> {
    match types.get(ty)? {
        TypeKind::Builtin(BuiltinType::Bool) => Some(ConstantScalarType::Bool),
        TypeKind::Builtin(builtin) if integer_builtin(*builtin) => {
            Some(ConstantScalarType::Integer(*builtin))
        }
        TypeKind::Borrow {
            capability: BorrowCapability::Readonly,
            referent,
        } if matches!(
            types.get(*referent),
            Some(TypeKind::Builtin(BuiltinType::Str))
        ) =>
        {
            Some(ConstantScalarType::Text)
        }
        _ => None,
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

fn plan_error(
    tree: &nocter_syntax::SyntaxTree,
    error: ConstantPlanError<BodyCheckError>,
) -> BodyCheckError {
    match error {
        ConstantPlanError::Context(error) => error,
        ConstantPlanError::Rule { rule, origin } => constant_error(
            tree,
            match rule {
                ConstantPlanRule::NonConstantExpression => {
                    ConstantExpressionRule::NonConstantExpression
                }
                ConstantPlanRule::TypeMismatch => ConstantExpressionRule::TypeMismatch,
            },
            origin,
        ),
        ConstantPlanError::InvalidSyntax(node) => {
            BodyCheckInternalError::InvalidSyntax(node).into()
        }
    }
}

fn evaluation_error(
    tree: &nocter_syntax::SyntaxTree,
    error: ConstantEvaluationError,
) -> BodyCheckError {
    match error.rule() {
        ConstantEvaluationRule::ArithmeticFailure => constant_error(
            tree,
            ConstantExpressionRule::ArithmeticFailure,
            error.origin(),
        ),
        ConstantEvaluationRule::DependencyCycle => constant_error(
            tree,
            ConstantExpressionRule::DependencyCycle,
            error.origin(),
        ),
        ConstantEvaluationRule::MissingConstant | ConstantEvaluationRule::InvalidPlan => {
            BodyCheckInternalError::InvalidSyntax(origin_node(tree, error.origin())).into()
        }
    }
}

fn constant_error(
    tree: &nocter_syntax::SyntaxTree,
    rule: ConstantExpressionRule,
    origin: SyntaxOrigin,
) -> BodyCheckError {
    let projected = match origin {
        SyntaxOrigin::Node(node) => SourceOrigin::from_node(tree, node)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(node)),
        SyntaxOrigin::Token(token) => SourceOrigin::from_token(tree, token)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(tree.root_id())),
    };
    match projected {
        Ok(origin) => BodyCheckError::from_constant_expression(
            rule,
            SourceDiagnostic::new(rule.code(), rule.message(), origin, [], Some(rule.help())),
        ),
        Err(error) => error.into(),
    }
}

fn origin_node(tree: &nocter_syntax::SyntaxTree, origin: SyntaxOrigin) -> NodeId {
    match origin {
        SyntaxOrigin::Node(node) => node,
        SyntaxOrigin::Token(_) => tree.root_id(),
    }
}
