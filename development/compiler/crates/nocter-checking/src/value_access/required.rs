//! Ordinary lexical snapshots for body-local required evaluation; no alternate expression checker.
use super::{Failure, checker::Checker, ir::Boundary};
use nocter_declaration_lowering::access_declarations::BodySource;
use nocter_model::{BuiltinType, ConstantValue, FrozenNode, ValueCodeRoot};
use nocter_syntax::{BodySyntaxProjection, NodeId};
use std::{collections::BTreeSet, sync::Arc};

/// An unpublished request captured at its ordinary checking site, with its lexical exclusions.
/// Neither the executable source pairing nor its enclosing scope can be supplied independently.
#[derive(Clone, Debug)]
pub struct RequiredExpression {
    pub(super) source: BodySource,
    pub(super) unavailable: BTreeSet<String>,
}
impl RequiredExpression {
    pub(super) fn code(&self) -> ValueCodeRoot {
        self.source.code()
    }
}

impl Checker<'_, '_> {
    pub(super) fn required_length(&mut self, node: NodeId) -> Result<u64, Failure> {
        if self.required_roots.is_none() {
            self.required_roots = Some(self.context.body_expressions(self.source.owner())?);
        }
        let catalog = self
            .required_roots
            .as_ref()
            .ok_or(Failure::InconsistentSyntax)?;
        let id = catalog
            .expression(node)
            .ok_or(Failure::InconsistentSyntax)?;
        let code = ValueCodeRoot::Expression(id);
        if let Some(value) = self.values.value(code) {
            let FrozenNode::Scalar(ConstantValue::Integer(value)) = value.node() else {
                return Err(Failure::InconsistentSyntax);
            };
            return u64::try_from(*value).map_err(|_| Failure::InconsistentSyntax);
        }
        let mut unavailable = self.unavailable.clone();
        unavailable.extend(self.scopes.iter().flat_map(|scope| scope.keys().cloned()));
        Err(Failure::RequiredExpression(Box::new(RequiredExpression {
            source: catalog.source(id).ok_or(Failure::InconsistentSyntax)?,
            unavailable,
        })))
    }
}

pub(super) fn prepare(
    context: &super::DeclarationContext<'_>,
    requirement: &RequiredExpression,
    values: &dyn super::evaluation::Values,
) -> Result<super::Query, Failure> {
    if !matches!(requirement.code(), ValueCodeRoot::Expression(_)) {
        return Err(Failure::InconsistentSyntax);
    }
    let source = requirement.source.bind(context)?;
    let syntax = context.syntax();
    let projection = BodySyntaxProjection::for_root(syntax.tree(), source.body())
        .ok_or(Failure::InconsistentSyntax)?;
    let result = context
        .program()
        .data_type(BuiltinType::Usize)
        .ok_or(Failure::InconsistentSyntax)?;
    let mut checker = Checker::new(
        context,
        result,
        context
            .program()
            .body_types(requirement.code())
            .map_err(|_| Failure::InconsistentSyntax)?,
        Arc::new(projection),
        requirement.source.clone(),
        requirement.code().code(),
        values,
    );
    checker.unavailable.clone_from(&requirement.unavailable);
    let value = checker.expression(source.body(), Some(Boundary::Result(result)), true)?;
    let root = checker.emit_block(source.body(), Vec::new(), Some(value), checker.ty(value))?;
    checker.finish(root)
}
