//! Read selection and ordinary operand checking for still-gated element operations.
use super::{Checker, Failure, PayloadSchema, Rule};
use crate::value_access::ir::Boundary;
use crate::value_access::ir::{ExpressionKind, IndexAction, value_type};
use nocter_model::{AccessExpressionId, BuiltinType, ValueAccessMode as Mode};
use nocter_syntax::{NodeId, NodeKind};

impl Checker<'_, '_> {
    pub(in crate::value_access) fn array_index(
        &mut self,
        node: NodeId,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        // Walk joint index chains iteratively. One failed inner selection must not recurse
        // through an arbitrary number of postfix wrappers before reporting its local cause.
        let mut suffixes = Vec::new();
        let mut root = self.unwrap(node)?;
        loop {
            let children = self.children(root);
            if self.kind(root)? != NodeKind::PostfixExpression {
                break;
            }
            let [base, suffix] = children.as_slice() else {
                return Err(Failure::InconsistentSyntax);
            };
            if self.kind(*suffix)? != NodeKind::IndexSuffix {
                break;
            }
            suffixes.push((root, *suffix));
            root = self.unwrap(*base)?;
        }
        let mut value = self.expression(root, None, reachable)?;
        for (node, suffix) in suffixes.into_iter().rev() {
            let Some(access) = self.ty(value).access() else {
                return self.reject(Rule::UnknownResultType, root);
            };
            let Some(PayloadSchema::Array(array)) = self.program().data_schema(access.data())
            else {
                return self.reject_message(
                    Rule::TypeMismatch,
                    node,
                    "this primitive bracket selection needs fixed-array data".into(),
                    Vec::new(),
                );
            };
            let element = array.element();
            let index = self.direct(suffix, NodeKind::Expression)?;
            let index = self.expression(
                index,
                Some(Boundary::Value(self.data(BuiltinType::Usize))),
                reachable && self.ty(value).may_complete(),
            )?;
            self.index_element_headers(element, node)?;
            if matches!(
                self.program().data_schema(element),
                Some(PayloadSchema::Function(_))
            ) {
                return self.reject(Rule::UnsupportedData, node);
            }
            let action = if access.mode() == Mode::Own {
                if !self.copyable(element) {
                    return self.reject(Rule::UnsupportedData, node);
                }
                IndexAction::CopyPayload
            } else {
                IndexAction::Access
            };
            let continues = self.ty(value).may_complete() && self.ty(index).may_complete();
            value = self.emit(
                node,
                value_type(
                    element,
                    if action == IndexAction::Access {
                        Mode::Read
                    } else {
                        Mode::Own
                    },
                )
                .with_continuation(continues),
                ExpressionKind::Index {
                    base: value,
                    index,
                    action,
                },
            )?;
        }
        Ok(value)
    }
    fn index_element_headers(
        &self,
        element: nocter_model::TypeId,
        node: NodeId,
    ) -> Result<(), Failure> {
        let mut pending = vec![element];
        let mut seen = std::collections::BTreeSet::new();
        while let Some(data) = pending.pop() {
            if !seen.insert(data) {
                continue;
            }
            if let Some(record) = self.program().record_identity(data)
                && !self.program().header_ready(record)
            {
                return Err(Failure::RequiredHeader {
                    declaration: record.into(),
                    origin: nocter_syntax::SyntaxOrigin::Node(node),
                });
            }
            match self.program().data_schema(data) {
                Some(PayloadSchema::Aggregate(schema))
                    if matches!(
                        schema.form(),
                        nocter_declarations::value_access::AggregateForm::Tuple(_)
                    ) =>
                {
                    pending.extend(schema.components().map(|c| c.data));
                }
                Some(PayloadSchema::Array(array)) if !array.is_empty() => {
                    pending.push(array.element());
                }
                Some(_) => {}
                None => return self.reject(Rule::UnsupportedData, node),
            }
        }
        Ok(())
    }
    /// Find an index without recursively preparing intervening calls, members or transfers.
    pub(in crate::value_access) fn first_array_index(
        &self,
        node: NodeId,
    ) -> Result<Option<NodeId>, Failure> {
        let mut current = self.unwrap(node)?;
        let mut first = None;
        loop {
            match self.kind(current)? {
                NodeKind::TransferExpression => {
                    current = self.unwrap(self.first_child(current)?)?;
                }
                NodeKind::PostfixExpression => {
                    let children = self.children(current);
                    let [base, suffix] = children.as_slice() else {
                        return Err(Failure::InconsistentSyntax);
                    };
                    if self.kind(*suffix)? == NodeKind::IndexSuffix {
                        first = Some(current);
                    }
                    current = self.unwrap(*base)?;
                }
                _ => return Ok(first),
            }
        }
    }
    pub(in crate::value_access) fn reject_array_selection<T>(
        &mut self,
        node: NodeId,
        reachable: bool,
    ) -> Result<T, Failure> {
        let node = self
            .first_array_index(node)?
            .ok_or(Failure::InconsistentSyntax)?;
        let children = self.children(node);
        let [base, suffix] = children.as_slice() else {
            return Err(Failure::InconsistentSyntax);
        };
        if self.kind(*suffix)? != NodeKind::IndexSuffix {
            return Err(Failure::InconsistentSyntax);
        }
        let (root, members) = self.selection(*base)?;
        let (data, continues) = if let Some(name) = self.name_token(root)?
            && let Some(local) = self.named_place(name)?
        {
            // Identify existing storage without emitting a complete-array read or snapshot.
            let place = self.local_selection(local, &members)?;
            (self.place_ty(place).data(), true)
        } else {
            let value = self.expression(*base, None, reachable)?;
            let Some(access) = self.ty(value).access() else {
                return self.reject(Rule::UnknownResultType, *base);
            };
            (access.data(), self.ty(value).may_complete())
        };
        if !matches!(
            self.program().data_schema(data),
            Some(PayloadSchema::Array(_))
        ) {
            return self.reject_message(
                Rule::TypeMismatch,
                *base,
                "this primitive bracket selection needs fixed-array data".into(),
                Vec::new(),
            );
        }
        let index = self.direct(*suffix, NodeKind::Expression)?;
        self.expression(
            index,
            Some(Boundary::Value(self.data(BuiltinType::Usize))),
            reachable && continues,
        )?;
        // This draft grants no bounds result, stable saved-index identity, place or access. All
        // three analysis consumers must close those facts before an element operation can freeze.
        self.reject(Rule::UnsupportedData, *suffix)
    }
}
