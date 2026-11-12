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
    pub(super) fn outcome(
        &mut self,
        node: NodeId,
        projection: Option<Mode>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let propagate = nocter_syntax::direct_token_iter(self.syntax().tree(), node).any(|token| {
            token.kind()
                == nocter_syntax::TokenKind::Punctuation(nocter_syntax::Punctuation::Question)
        });
        if matches!(projection, Some(Mode::Read | Mode::Update)) {
            let child = self.first_child(node)?;
            let source =
                self.expression_with_projection(child, None, None, projection, reachable)?;
            if let Some(place) = self.expression_referent(source) {
                let Some(access) = self.ty(source).access() else {
                    return self.reject(Rule::TypeMismatch, node);
                };
                let Some(PayloadSchema::Optional(payload)) =
                    self.program().data_schema(access.data())
                else {
                    return self.reject(Rule::TypeMismatch, node);
                };
                if projection == Some(Mode::Update) {
                    self.require_writable_referent(place, node, Rule::InvalidUpdate)?;
                }
                return self.scoped_optional(
                    node,
                    source,
                    place,
                    access.data(),
                    payload,
                    propagate,
                );
            }
            // Reuse the already checked produced value rather than preparing it twice.
            return if propagate {
                self.propagate_optional_source(node, source)
            } else {
                self.require_optional_source(node, source)
            };
        }
        if propagate {
            self.propagate_optional(node, reachable)
        } else {
            self.require_optional(node, reachable)
        }
    }
    fn supply_optional(
        &mut self,
        node: NodeId,
        source: AccessExpressionId,
    ) -> Result<(AccessExpressionId, TypeId, TypeId), Failure> {
        let Some(access) = self.ty(source).access() else {
            return self.reject(Rule::TypeMismatch, node);
        };
        let Some(PayloadSchema::Optional(payload)) = self.program().data_schema(access.data())
        else {
            return self.reject(Rule::TypeMismatch, node);
        };
        let source = self.boundary(node, source, Boundary::Value(access.data()))?;
        Ok((source, access.data(), payload))
    }
    fn require_optional(
        &mut self,
        node: NodeId,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let child = self.first_child(node)?;
        let source = self.expression(child, None, reachable)?;
        self.require_optional_source(node, source)
    }
    fn require_optional_source(
        &mut self,
        node: NodeId,
        source: AccessExpressionId,
    ) -> Result<AccessExpressionId, Failure> {
        let (source, data, payload) = self.supply_optional(node, source)?;
        let required = self.emit(
            node,
            value_type(data, Mode::Own).with_continuation(self.ty(source).may_complete()),
            ExpressionKind::RequirePrimary { source },
        )?;
        self.select_optional_payload(node, required, payload)
    }
    /// The caller places this selection after its primary-branch guard.
    fn select_optional_payload(
        &mut self,
        node: NodeId,
        source: AccessExpressionId,
        payload: TypeId,
    ) -> Result<AccessExpressionId, Failure> {
        self.emit(
            node,
            value_type(payload, Mode::Own).with_continuation(self.ty(source).may_complete()),
            ExpressionKind::Select {
                source,
                fields: FieldPath::new([ValueField::Variant(true)]),
                action: if self.copyable(payload) {
                    SelectionAction::CopyPayload
                } else {
                    SelectionAction::MovePayload
                },
            },
        )
    }

    /// Expand value propagation into ordinary binding, conditional and return operations.
    /// The local owns the supplied carrier once; its scope protects the selected answer before cleanup.
    fn propagate_optional(
        &mut self,
        node: NodeId,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let child = self.first_child(node)?;
        let source = self.expression(child, None, reachable)?;
        self.propagate_optional_source(node, source)
    }
    fn propagate_optional_source(
        &mut self,
        node: NodeId,
        source: AccessExpressionId,
    ) -> Result<AccessExpressionId, Failure> {
        use super::ir::{Local, LocalOrigin, Statement};
        use nocter_model::{BuiltinType, ValueBindingKind};
        let else_block = self.absence_return(node)?;
        let (source, data, payload) = self.supply_optional(node, source)?;
        let local = self.locals.insert(Local {
            data,
            kind: ValueBindingKind::FixedValue,
            origin: LocalOrigin::Binding,
        });
        self.origins
            .locals
            .insert(local, self.site(nocter_syntax::SyntaxOrigin::Node(node))?);
        let binding = self.statement(
            node,
            Statement::Bind {
                local,
                value: source,
            },
        )?;
        let place = self.intern_place(local, FieldPath::default(), data);
        let condition = self.emit(
            node,
            value_type(self.data(BuiltinType::Bool), Mode::Own),
            ExpressionKind::PrimaryCondition {
                source: place.into(),
            },
        )?;
        let carrier = self.transfer_place(node, place)?;
        let present = self.select_optional_payload(node, carrier, payload)?;
        let then_block = self.emit_block(node, Vec::new(), Some(present), self.ty(present))?;
        let selected = self.emit(
            node,
            self.ty(present),
            ExpressionKind::If {
                condition,
                then_block,
                else_block,
            },
        )?;
        let ty = self
            .ty(selected)
            .with_continuation(self.ty(source).may_complete());
        let block = self.emit_block(node, vec![binding], Some(selected), ty)?;
        self.emit(node, ty, ExpressionKind::Block { block })
    }
    fn absence_return(&mut self, node: NodeId) -> Result<nocter_model::AccessBlockId, Failure> {
        use super::ir::{Statement, ValueType};
        if matches!(self.code, nocter_model::ValueCode::Expression(_))
            || matches!(self.code, nocter_model::ValueCode::Declaration(code)
                if self.program().codes().get(code).is_some_and(|code| code.kind().is_initializer()))
        {
            return self.reject(Rule::InvalidReturn, node);
        }
        if !matches!(
            self.program().data_schema(self.result),
            Some(PayloadSchema::Optional(_))
        ) {
            return self.reject_message(
                Rule::TypeMismatch,
                node,
                "optional propagation needs an optional callable result".into(),
                Vec::new(),
            );
        }
        let absent = self.absent(node, Some(self.result))?;
        let returning = self.statement(node, Statement::Return(absent))?;
        self.emit_block(node, vec![returning], None, ValueType::NEVER)
    }
    fn scoped_optional(
        &mut self,
        node: NodeId,
        source: AccessExpressionId,
        place: super::Referent,
        data: TypeId,
        payload: TypeId,
        propagate: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let guard = self.emit(
            node,
            value_type(data, Mode::Read),
            ExpressionKind::GuardView { source: place },
        )?;
        let projected = if propagate {
            let condition = self.emit(
                node,
                value_type(self.data(nocter_model::BuiltinType::Bool), Mode::Own),
                ExpressionKind::PrimaryCondition { source: place },
            )?;
            let then_block = self.emit_block(node, Vec::new(), Some(guard), self.ty(guard))?;
            let else_block = self.absence_return(node)?;
            self.emit(
                node,
                self.ty(guard),
                ExpressionKind::If {
                    condition,
                    then_block,
                    else_block,
                },
            )?
        } else {
            guard
        };
        let receipt = self.emit(
            node,
            value_type(data, Mode::Read).with_continuation(self.ty(source).may_complete()),
            ExpressionKind::Scoped { source, projected },
        )?;
        self.emit(
            node,
            value_type(payload, Mode::Read).with_continuation(self.ty(source).may_complete()),
            ExpressionKind::Select {
                source: receipt,
                fields: FieldPath::new([ValueField::Variant(true)]),
                action: SelectionAction::Access,
            },
        )
    }
}
