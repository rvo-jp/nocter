//! Contextual outcome construction preserves exact carriers before adding a layer.
use super::{
    Failure, Rule,
    checker::Checker,
    ir::{Boundary, ExpressionKind, FieldValue, SelectionAction, value_type},
};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::{AccessExpressionId, FieldPath, TypeId, ValueAccessMode as Mode, ValueField};
use nocter_syntax::NodeId;
impl Checker<'_, '_> {
    pub(super) fn optional_payload(&self, mut data: TypeId) -> TypeId {
        while let Some(PayloadSchema::Optional(payload)) = self.program().data_schema(data) {
            data = payload;
        }
        data
    }
    pub(super) fn absent(
        &mut self,
        node: NodeId,
        data: Option<TypeId>,
    ) -> Result<AccessExpressionId, Failure> {
        let Some(data) = data.filter(|&data| {
            matches!(
                self.program().data_schema(data),
                Some(PayloadSchema::Optional(_))
            )
        }) else {
            return self.reject_message(
                Rule::TypeMismatch,
                node,
                "none needs an expected optional data type".into(),
                Vec::new(),
            );
        };
        self.emit(
            node,
            value_type(data, Mode::Own),
            ExpressionKind::Aggregate {
                fields: Box::new([]),
            },
        )
    }
    pub(super) fn inject_optional(
        &mut self,
        node: NodeId,
        mut source: AccessExpressionId,
        data: TypeId,
    ) -> Result<AccessExpressionId, Failure> {
        let actual = self
            .ty(source)
            .access()
            .ok_or(Failure::InconsistentSyntax)?
            .data();
        let mut wrappers = Vec::new();
        let mut inner = data;
        while inner != actual {
            let Some(PayloadSchema::Optional(payload)) = self.program().data_schema(inner) else {
                break;
            };
            wrappers.push(inner);
            inner = payload;
        }
        source = self.boundary(node, source, Boundary::Value(inner))?;
        for data in wrappers.into_iter().rev() {
            source = self.emit(
                node,
                value_type(data, Mode::Own).with_continuation(self.ty(source).may_complete()),
                ExpressionKind::Aggregate {
                    fields: vec![FieldValue {
                        field: ValueField::Variant(true),
                        value: source,
                    }]
                    .into(),
                },
            )?;
        }
        Ok(source)
    }
    pub(super) fn require_optional(
        &mut self,
        node: NodeId,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let child = self.first_child(node)?;
        let source = self.expression(child, None, reachable)?;
        let Some(access) = self.ty(source).access() else {
            return self.reject(Rule::TypeMismatch, node);
        };
        let Some(PayloadSchema::Optional(payload)) = self.program().data_schema(access.data())
        else {
            return self.reject(Rule::TypeMismatch, node);
        };
        let source = self.boundary(node, source, Boundary::Value(access.data()))?;
        let required = self.emit(
            node,
            value_type(access.data(), Mode::Own).with_continuation(self.ty(source).may_complete()),
            ExpressionKind::RequirePrimary { source },
        )?;
        self.emit(
            node,
            value_type(payload, Mode::Own).with_continuation(self.ty(required).may_complete()),
            ExpressionKind::Select {
                source: required,
                fields: FieldPath::new([ValueField::Variant(true)]),
                action: if self.copyable(payload) {
                    SelectionAction::CopyPayload
                } else {
                    SelectionAction::MovePayload
                },
            },
        )
    }
}
