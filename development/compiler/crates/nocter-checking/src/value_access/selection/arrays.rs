//! Read selection and ordinary operand checking for still-gated element operations.
use super::{Checker, Failure, PayloadSchema, Rule};
use crate::value_access::ir::Boundary;
use crate::value_access::ir::{ExpressionKind, IndexAction, value_type};
use nocter_model::{AccessExpressionId, BuiltinType, ValueAccessMode as Mode};
use nocter_syntax::{NodeId, NodeKind};

impl Checker<'_, '_> {
    pub(in crate::value_access) fn index_component(
        &mut self,
        node: NodeId,
        suffix: NodeId,
        base: AccessExpressionId,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let Some(access) = self.ty(base).access() else {
            return self.reject(Rule::UnknownResultType, node);
        };
        let Some(PayloadSchema::Array(array)) = self.program().data_schema(access.data()) else {
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
            reachable && self.ty(base).may_complete(),
        )?;
        let known = self
            .expressions
            .get(index)
            .ok_or(Failure::InconsistentSyntax)?
            .index_fact();
        let bounds = match known {
            _ if array.is_empty() => nocter_model::IndexBoundsCheck::ProvenTrap,
            super::super::IndexFact::Known(index) if index >= array.length() => {
                nocter_model::IndexBoundsCheck::ProvenTrap
            }
            super::super::IndexFact::Known(_) => nocter_model::IndexBoundsCheck::ProvenInBounds,
            super::super::IndexFact::Unknown => nocter_model::IndexBoundsCheck::Required,
        };
        let continues = self.ty(base).may_complete()
            && self.ty(index).may_complete()
            && bounds != nocter_model::IndexBoundsCheck::ProvenTrap;
        if continues {
            self.index_element_headers(element, node)?;
        }
        if matches!(
            self.program().data_schema(element),
            Some(PayloadSchema::Function(_))
        ) && self.program().payload_readiness(element)
            == nocter_declarations::value_access::PayloadReadiness::Unsupported
        {
            return self.reject(Rule::UnsupportedData, node);
        }
        let action = if access.mode() == Mode::Own {
            if continues
                && !self.copyable(element)
                && self.program().payload_readiness(element)
                    != nocter_declarations::value_access::PayloadReadiness::Pending
            {
                return self.reject(Rule::UnsupportedData, node);
            }
            IndexAction::CopyPayload
        } else {
            IndexAction::Access
        };
        self.emit(
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
                base,
                index,
                action,
                bounds,
            },
        )
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
            let place = self.member_place(local, &members)?;
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
