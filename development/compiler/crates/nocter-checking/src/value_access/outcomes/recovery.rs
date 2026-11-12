//! Local recovery uses ordinary branch, binding and scope cleanup.
use super::{
    AccessExpressionId, Boundary, Checker, ExpressionKind, Failure, FieldPath, Mode, NodeId,
    PayloadSchema, Rule, SelectionAction, TypeId, ValueField, value_type,
};
use crate::value_access::ir::{Local, LocalOrigin, Projection, Statement};
use nocter_model::{AccessBlockId, BuiltinType, ValueAccess, ValueBindingKind};
use nocter_syntax::{Keyword, NodeKind, SyntaxOrigin, SyntaxToken, TokenKind};

#[derive(Clone, Copy)]
enum Recovery {
    Absence,
    Failure(SyntaxToken),
}
impl Checker<'_, '_> {
    pub(in crate::value_access) fn recover_outcome(
        &mut self,
        node: NodeId,
        projection: Option<Projection>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let child = self.first_child(node)?;
        let source = self.expression_with_projection(child, None, None, projection, reachable)?;
        let Some(access) = self.ty(source).access() else {
            return self.reject(Rule::TypeMismatch, node);
        };
        let clause = self.direct(node, NodeKind::RecoveryClause)?;
        let recovery = if nocter_syntax::direct_token_iter(self.syntax().tree(), clause)
            .any(|token| token.kind() == TokenKind::Keyword(Keyword::Catch))
        {
            Recovery::Failure(
                nocter_syntax::direct_identifier_iter(self.syntax().tree(), clause)
                    .next()
                    .ok_or(Failure::InconsistentSyntax)?,
            )
        } else {
            Recovery::Absence
        };
        let ((Recovery::Absence, Some(PayloadSchema::Optional(payload)))
        | (Recovery::Failure(_), Some(PayloadSchema::Fallible(payload)))) =
            (recovery, self.program().data_schema(access.data()))
        else {
            return self.reject(Rule::TypeMismatch, node);
        };
        let block = self.direct(clause, NodeKind::Block)?;
        let fallback_reachable = reachable && self.ty(source).may_complete();
        if let Some(request) =
            projection.filter(|request| matches!(request.mode, Mode::Read | Mode::Update))
            && let Some(place) = self.expression_referent(source)
        {
            if request.mode == Mode::Update {
                self.require_writable_referent(place, node, Rule::InvalidUpdate)?;
            }
            let expected = if request.temporary {
                Boundary::Input(ValueAccess::new(payload, request.mode))
            } else {
                Boundary::Place(ValueAccess::new(payload, request.mode))
            };
            let error = if let Recovery::Failure(name) = recovery
                && self.text(name)? != "_"
            {
                Some(self.project_outcome(
                    node,
                    place,
                    access.data(),
                    self.data(BuiltinType::Error),
                    false,
                )?)
            } else {
                None
            };
            let fallback =
                self.recovery_block(block, recovery, error, expected, fallback_reachable)?;
            let present = self.project_outcome(node, place, access.data(), payload, true)?;
            let condition = self.emit(
                node,
                value_type(self.data(BuiltinType::Bool), Mode::Own),
                ExpressionKind::PrimaryCondition { source: place },
            )?;
            let projected = self.outcome_branch(node, condition, present, fallback)?;
            return self.emit(
                node,
                value_type(payload, Mode::Read).with_continuation(self.ty(source).may_complete()),
                ExpressionKind::Scoped { source, projected },
            );
        }
        let (source, data, payload) = self.supply_outcome(node, source)?;
        let owned = self.owned_outcome(node, source, data, payload)?;
        let error = if matches!(recovery, Recovery::Failure(_)) {
            let carrier = self.transfer_place(node, owned.place)?;
            Some(self.select_outcome_field(node, carrier, self.data(BuiltinType::Error), false)?)
        } else {
            None
        };
        let fallback = self.recovery_block(
            block,
            recovery,
            error,
            Boundary::Value(payload),
            fallback_reachable,
        )?;
        self.finish_outcome(node, &owned, fallback)
    }

    fn project_outcome(
        &mut self,
        node: NodeId,
        source: crate::value_access::Referent,
        data: TypeId,
        payload: TypeId,
        primary: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let guard = self.emit(
            node,
            value_type(data, Mode::Read),
            ExpressionKind::GuardView { source, primary },
        )?;
        self.emit(
            node,
            value_type(payload, Mode::Read),
            ExpressionKind::Select {
                source: guard,
                fields: FieldPath::new([ValueField::Variant(primary)]),
                action: SelectionAction::Access,
            },
        )
    }

    fn recovery_block(
        &mut self,
        block: NodeId,
        recovery: Recovery,
        error: Option<AccessExpressionId>,
        expected: Boundary,
        reachable: bool,
    ) -> Result<AccessBlockId, Failure> {
        let Recovery::Failure(name) = recovery else {
            return self.block(block, Some(expected), reachable);
        };
        // Suppression of a retained carrier neither extracts nor destroys its error.
        let Some(error) = error else {
            return self.block(block, Some(expected), reachable);
        };
        self.scopes.push(std::collections::BTreeMap::new());
        let statement = if self.text(name)? == "_" {
            // An owning suppression destroys the received error before evaluating the fallback.
            self.statement(block, Statement::Discard(error))?
        } else {
            let spelling = self.text(name)?.to_owned();
            if self.shadows_name(&spelling, name)? {
                return self.reject(Rule::ShadowedName, name);
            }
            let mode = match expected {
                Boundary::Place(access) | Boundary::Input(access) => access.mode(),
                _ => Mode::Own,
            };
            let kind = match mode {
                Mode::Read => ValueBindingKind::ReadAlias,
                Mode::Update => ValueBindingKind::EditAlias,
                Mode::Own => ValueBindingKind::FixedValue,
            };
            let data = self.data(BuiltinType::Error);
            let error = self.boundary(
                block,
                error,
                Boundary::Binding(ValueAccess::new(data, mode)),
            )?;
            let local = self.locals.insert(Local {
                data,
                kind,
                origin: LocalOrigin::Binding,
            });
            self.origins
                .locals
                .insert(local, self.site(SyntaxOrigin::Token(name))?);
            self.bind_local(spelling, local);
            self.statement(
                block,
                Statement::Bind {
                    local,
                    value: error,
                },
            )?
        };
        let body = self.block(block, Some(expected), reachable)?;
        self.scopes.pop();
        let ty = self.blocks.get(body).ok_or(Failure::InconsistentSyntax)?.ty;
        let result = self.emit(block, ty, ExpressionKind::Block { block: body })?;
        self.emit_block(block, vec![statement], Some(result), ty)
    }
}
