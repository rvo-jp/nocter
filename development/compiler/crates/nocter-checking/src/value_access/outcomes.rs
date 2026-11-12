//! Contextual outcome construction preserves exact carriers before adding a layer.
use super::{
    Failure, Rule,
    checker::Checker,
    ir::{Boundary, ExpressionKind, FieldValue, SelectionAction, value_type},
};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::{AccessExpressionId, FieldPath, TypeId, ValueAccessMode as Mode, ValueField};
use nocter_syntax::NodeId;
mod recovery;
struct OwnedOutcome {
    source: AccessExpressionId,
    payload: TypeId,
    binding: nocter_model::AccessStatementId,
    place: nocter_model::AccessPlaceId,
}
impl Checker<'_, '_> {
    pub(super) fn outcome_payload(&self, mut data: TypeId) -> TypeId {
        while let Some(PayloadSchema::Optional(payload) | PayloadSchema::Fallible(payload)) =
            self.program().data_schema(data)
        {
            data = payload;
        }
        data
    }
    fn absence_type(&self, mut data: TypeId) -> Option<TypeId> {
        while let Some(PayloadSchema::Fallible(payload)) = self.program().data_schema(data) {
            data = payload;
        }
        matches!(
            self.program().data_schema(data),
            Some(PayloadSchema::Optional(_))
        )
        .then_some(data)
    }
    pub(super) fn absent(
        &mut self,
        node: NodeId,
        data: Option<TypeId>,
    ) -> Result<AccessExpressionId, Failure> {
        let Some(data) = data.and_then(|data| self.absence_type(data)) else {
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
    pub(super) fn inject_outcomes(
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
            let Some(PayloadSchema::Optional(payload) | PayloadSchema::Fallible(payload)) =
                self.program().data_schema(inner)
            else {
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
        projection: Option<super::ir::Projection>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let propagate = nocter_syntax::direct_token_iter(self.syntax().tree(), node).any(|token| {
            token.kind()
                == nocter_syntax::TokenKind::Punctuation(nocter_syntax::Punctuation::Question)
        });
        if matches!(
            projection.map(|request| request.mode),
            Some(Mode::Read | Mode::Update)
        ) {
            let child = self.first_child(node)?;
            let source =
                self.expression_with_projection(child, None, None, projection, reachable)?;
            if let Some(place) = self.expression_referent(source) {
                let Some(access) = self.ty(source).access() else {
                    return self.reject(Rule::TypeMismatch, node);
                };
                let payload = match self.program().data_schema(access.data()) {
                    Some(PayloadSchema::Optional(payload)) => payload,
                    Some(PayloadSchema::Fallible(payload)) if !propagate => payload,
                    Some(PayloadSchema::Fallible(_)) => return self.reject_message(Rule::MissingTransfer, node,
                        "scoped failure propagation cannot extract an owned error; supply the carrier as a value or recover locally".into(), Vec::new()),
                    _ => return self.reject(Rule::TypeMismatch, node),
                };
                if projection.is_some_and(|request| request.mode == Mode::Update) {
                    self.require_writable_referent(place, node, Rule::InvalidUpdate)?;
                }
                let fallback = if propagate {
                    Some(self.absence_return(node)?)
                } else {
                    None
                };
                return self.scoped_outcome(node, source, place, access.data(), payload, fallback);
            }
            // Reuse the already checked produced value rather than preparing it twice.
            return if propagate {
                self.propagate_outcome_source(node, source)
            } else {
                self.require_primary_source(node, source)
            };
        }
        if propagate {
            self.propagate_outcome(node, reachable)
        } else {
            self.require_primary(node, reachable)
        }
    }
    fn supply_outcome(
        &mut self,
        node: NodeId,
        source: AccessExpressionId,
    ) -> Result<(AccessExpressionId, TypeId, TypeId), Failure> {
        let Some(access) = self.ty(source).access() else {
            return self.reject(Rule::TypeMismatch, node);
        };
        let Some(PayloadSchema::Optional(payload) | PayloadSchema::Fallible(payload)) =
            self.program().data_schema(access.data())
        else {
            return self.reject(Rule::TypeMismatch, node);
        };
        let source = self.boundary(node, source, Boundary::Value(access.data()))?;
        Ok((source, access.data(), payload))
    }
    fn require_primary(
        &mut self,
        node: NodeId,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let child = self.first_child(node)?;
        let source = self.expression(child, None, reachable)?;
        self.require_primary_source(node, source)
    }
    fn require_primary_source(
        &mut self,
        node: NodeId,
        source: AccessExpressionId,
    ) -> Result<AccessExpressionId, Failure> {
        let (source, data, payload) = self.supply_outcome(node, source)?;
        let required = self.emit(
            node,
            value_type(data, Mode::Own).with_continuation(self.ty(source).may_complete()),
            ExpressionKind::RequirePrimary { source },
        )?;
        self.select_outcome_payload(node, required, payload)
    }
    /// The caller places this selection after its primary-branch guard.
    fn select_outcome_payload(
        &mut self,
        node: NodeId,
        source: AccessExpressionId,
        payload: TypeId,
    ) -> Result<AccessExpressionId, Failure> {
        self.select_outcome_field(node, source, payload, true)
    }
    fn select_outcome_field(
        &mut self,
        node: NodeId,
        source: AccessExpressionId,
        payload: TypeId,
        primary: bool,
    ) -> Result<AccessExpressionId, Failure> {
        self.emit(
            node,
            value_type(payload, Mode::Own).with_continuation(self.ty(source).may_complete()),
            ExpressionKind::Select {
                source,
                fields: FieldPath::new([ValueField::Variant(primary)]),
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
    fn propagate_outcome(
        &mut self,
        node: NodeId,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let child = self.first_child(node)?;
        let source = self.expression(child, None, reachable)?;
        self.propagate_outcome_source(node, source)
    }
    fn propagate_outcome_source(
        &mut self,
        node: NodeId,
        source: AccessExpressionId,
    ) -> Result<AccessExpressionId, Failure> {
        let (source, data, payload) = self.supply_outcome(node, source)?;
        self.outcome_value_scope(node, source, data, payload, None)
    }
    fn outcome_value_scope(
        &mut self,
        node: NodeId,
        source: AccessExpressionId,
        data: TypeId,
        payload: TypeId,
        fallback: Option<nocter_model::AccessBlockId>,
    ) -> Result<AccessExpressionId, Failure> {
        let owned = self.owned_outcome(node, source, data, payload)?;
        let else_block = if let Some(block) = fallback {
            block
        } else {
            match self.program().data_schema(data) {
                Some(PayloadSchema::Optional(_)) => self.absence_return(node)?,
                Some(PayloadSchema::Fallible(_)) => {
                    let carrier = self.transfer_place(node, owned.place)?;
                    let error = self.select_outcome_field(
                        node,
                        carrier,
                        self.data(nocter_model::BuiltinType::Error),
                        false,
                    )?;
                    self.failure_return(node, error)?
                }
                _ => return Err(Failure::InconsistentSyntax),
            }
        };
        self.finish_outcome(node, &owned, else_block)
    }
    fn owned_outcome(
        &mut self,
        node: NodeId,
        source: AccessExpressionId,
        data: TypeId,
        payload: TypeId,
    ) -> Result<OwnedOutcome, Failure> {
        use super::ir::{Local, LocalOrigin, Statement};
        use nocter_model::ValueBindingKind;
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
        Ok(OwnedOutcome {
            source,
            payload,
            binding,
            place,
        })
    }
    fn finish_outcome(
        &mut self,
        node: NodeId,
        owned: &OwnedOutcome,
        else_block: nocter_model::AccessBlockId,
    ) -> Result<AccessExpressionId, Failure> {
        let condition = self.emit(
            node,
            value_type(self.data(nocter_model::BuiltinType::Bool), Mode::Own),
            ExpressionKind::PrimaryCondition {
                source: owned.place.into(),
            },
        )?;
        let carrier = self.transfer_place(node, owned.place)?;
        let present = self.select_outcome_payload(node, carrier, owned.payload)?;
        let selected = self.outcome_branch(node, condition, present, else_block)?;
        let ty = self
            .ty(selected)
            .with_continuation(self.ty(owned.source).may_complete());
        let block = self.emit_block(node, vec![owned.binding], Some(selected), ty)?;
        self.emit(node, ty, ExpressionKind::Block { block })
    }
    fn outcome_branch(
        &mut self,
        node: NodeId,
        condition: AccessExpressionId,
        present: AccessExpressionId,
        else_block: nocter_model::AccessBlockId,
    ) -> Result<AccessExpressionId, Failure> {
        let then_block = self.emit_block(node, Vec::new(), Some(present), self.ty(present))?;
        self.emit(
            node,
            self.ty(present),
            ExpressionKind::If {
                condition,
                then_block,
                else_block,
            },
        )
    }
    fn absence_return(&mut self, node: NodeId) -> Result<nocter_model::AccessBlockId, Failure> {
        use super::ir::{Statement, ValueType};
        self.allow_return(node)?;
        let result = self.known_result(node)?;
        let Some(target) = self.absence_type(result) else {
            return self.reject_message(
                Rule::TypeMismatch,
                node,
                "optional propagation needs an optional callable result".into(),
                Vec::new(),
            );
        };
        let mut absent = self.absent(node, Some(target))?;
        if self
            .ty(absent)
            .access()
            .is_some_and(|ty| ty.data() != result)
        {
            absent = self.boundary(node, absent, Boundary::Result(result))?;
        }
        let returning = self.statement(node, Statement::Return(absent))?;
        self.emit_block(node, vec![returning], None, ValueType::NEVER)
    }
    pub(super) fn failure_value(
        &mut self,
        node: NodeId,
        error: AccessExpressionId,
    ) -> Result<AccessExpressionId, Failure> {
        self.allow_return(node)?;
        let result = self.known_result(node)?;
        let mut data = result;
        while let Some(PayloadSchema::Optional(payload)) = self.program().data_schema(data) {
            data = payload;
        }
        if !matches!(
            self.program().data_schema(data),
            Some(PayloadSchema::Fallible(_))
        ) {
            return self.reject_message(
                Rule::TypeMismatch,
                node,
                "failure needs a fallible callable result".into(),
                Vec::new(),
            );
        }
        let error = self.boundary(
            node,
            error,
            Boundary::Value(self.data(nocter_model::BuiltinType::Error)),
        )?;
        let mut value = self.emit(
            node,
            value_type(data, Mode::Own).with_continuation(self.ty(error).may_complete()),
            ExpressionKind::Aggregate {
                fields: vec![FieldValue {
                    field: ValueField::Variant(false),
                    value: error,
                }]
                .into(),
            },
        )?;
        if data != result {
            value = self.boundary(node, value, Boundary::Result(result))?;
        }
        Ok(value)
    }
    fn failure_return(
        &mut self,
        node: NodeId,
        error: AccessExpressionId,
    ) -> Result<nocter_model::AccessBlockId, Failure> {
        let value = self.failure_value(node, error)?;
        let statement = self.statement(node, super::ir::Statement::Return(value))?;
        self.emit_block(node, vec![statement], None, super::ir::ValueType::NEVER)
    }
    fn scoped_outcome(
        &mut self,
        node: NodeId,
        source: AccessExpressionId,
        place: super::Referent,
        data: TypeId,
        payload: TypeId,
        fallback: Option<nocter_model::AccessBlockId>,
    ) -> Result<AccessExpressionId, Failure> {
        let guard = self.emit(
            node,
            value_type(data, Mode::Read),
            ExpressionKind::GuardView {
                source: place,
                primary: true,
            },
        )?;
        let projected = if let Some(else_block) = fallback {
            if self.block_ty(else_block).may_complete() {
                return Err(Failure::InconsistentSyntax);
            }
            let condition = self.emit(
                node,
                value_type(self.data(nocter_model::BuiltinType::Bool), Mode::Own),
                ExpressionKind::PrimaryCondition { source: place },
            )?;
            self.outcome_branch(node, condition, guard, else_block)?
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
