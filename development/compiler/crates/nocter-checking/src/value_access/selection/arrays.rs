//! Ordinary index operand checking; evaluated-index and array flow admission remain separate.
use super::{Checker, Failure, PayloadSchema, Rule};
use crate::value_access::ir::Boundary;
use nocter_model::BuiltinType;
use nocter_syntax::{NodeId, NodeKind};

impl Checker<'_, '_> {
    /// Find the first unclosed index operation without recursing through a long postfix chain.
    fn first_index_draft(&self, node: NodeId) -> Result<NodeId, Failure> {
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
                _ => return first.ok_or(Failure::InconsistentSyntax),
            }
        }
    }
    pub(in crate::value_access) fn reject_array_selection<T>(
        &mut self,
        node: NodeId,
        reachable: bool,
    ) -> Result<T, Failure> {
        let node = self.first_index_draft(node)?;
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
