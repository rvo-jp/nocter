//! One member-chain and field resolver for existing places, computed values and construction.
use super::{
    Failure, Rule,
    checker::Checker,
    ir::{ExpressionKind, Place, SelectionAction, value_type},
};
use nocter_declarations::value_access::{Namespace, PayloadSchema};
use nocter_model::{
    AccessExpressionId, AccessLocalId, AccessPlaceId, FieldPath, TypeId, ValueAccess as Ty,
    ValueAccessMode as Mode, ValueField,
};
use nocter_syntax::{NodeId, NodeKind, SyntaxToken};
mod arrays;

impl Checker<'_, '_> {
    fn selection(&mut self, node: NodeId) -> Result<(NodeId, Vec<SyntaxToken>), Failure> {
        let mut current = self.unwrap(node)?;
        let mut members = Vec::new();
        loop {
            let children = self.children(current);
            if self.kind(current)? == NodeKind::PostfixExpression
                && let Some(&suffix) = children.last()
                && self.kind(suffix)? == NodeKind::IndexSuffix
            {
                return self.reject_array_selection(current, true);
            }
            if self.kind(current)? == NodeKind::PostfixExpression
                && let [base, suffix] = children.as_slice()
                && self.kind(*suffix)? == NodeKind::MemberSuffix
            {
                members.push(self.member_token(*suffix)?);
                current = self.unwrap(*base)?;
            } else {
                members.reverse();
                return Ok((current, members));
            }
        }
    }
    pub(super) fn member_token(&self, suffix: NodeId) -> Result<SyntaxToken, Failure> {
        nocter_syntax::direct_token_iter(self.syntax().tree(), suffix)
            .find(|token| {
                matches!(
                    token.kind(),
                    nocter_syntax::TokenKind::Identifier | nocter_syntax::TokenKind::IntegerLiteral
                )
            })
            .ok_or(Failure::InconsistentSyntax)
    }
    pub fn transfer(
        &mut self,
        node: NodeId,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let source = self.unwrap(self.first_child(node)?)?;
        if self.kind(source)? == NodeKind::TransferExpression {
            return self.reject_message(
                Rule::MissingTransfer,
                node,
                "the source is already marked for transfer; remove the repeated postfix ^".into(),
                Vec::new(),
            );
        }
        if self.first_array_index(source)?.is_some() {
            let value = self.expression(source, None, reachable)?;
            let Some(referent) = self.expression_referent(value) else {
                return self.reject(Rule::MissingTransfer, node);
            };
            return self.transfer_referent(node, referent);
        }
        let place = self.place(source, Rule::MissingTransfer)?;
        self.transfer_place(node, place)
    }
    pub(super) fn transfer_place(
        &mut self,
        node: NodeId,
        place: AccessPlaceId,
    ) -> Result<AccessExpressionId, Failure> {
        self.transfer_referent(node, place.into())
    }
    pub(super) fn transfer_referent(
        &mut self,
        node: NodeId,
        source: super::Referent,
    ) -> Result<AccessExpressionId, Failure> {
        let Some(base) =
            source.static_base(|id| self.expressions.get(id), |id| self.blocks.get(id))
        else {
            return self.reject_message(
                Rule::MissingTransfer,
                node,
                "postfix transfer needs one proven owning source; this place has alternatives"
                    .into(),
                Vec::new(),
            );
        };
        let access = self.place_ty(base);
        if access.mode() != Mode::Own {
            return self.reject_message(
                Rule::MissingTransfer, node,
                "postfix transfer requires an owning source; scoped access cannot surrender its referent".into(),
                Vec::new(),
            );
        }
        let ty = source
            .value_type(|id| self.places.get(id), |id| self.expressions.get(id))
            .ok_or(Failure::InconsistentSyntax)?;
        self.emit(
            node,
            value_type(
                ty.access().ok_or(Failure::InconsistentSyntax)?.data(),
                Mode::Own,
            )
            .with_continuation(ty.may_complete()),
            ExpressionKind::Move(source),
        )
    }
    pub fn field(&self, data: TypeId, token: SyntaxToken) -> Result<ValueField, Failure> {
        if let Some(record) = self.program().record_identity(data)
            && !self.program().header_ready(record)
        {
            return Err(Failure::RequiredHeader {
                declaration: record.into(),
                origin: nocter_syntax::SyntaxOrigin::Token(token),
            });
        }
        let schema = self
            .program()
            .data_schema(data)
            .ok_or(Failure::InconsistentSyntax)?;
        let field = if let PayloadSchema::Aggregate(aggregate) = schema {
            match aggregate.form() {
                nocter_declarations::value_access::AggregateForm::Record(record) => self
                    .program()
                    .symbols()
                    .get(self.text(token)?)
                    .and_then(|name| record.resolve_field(name))
                    .map(ValueField::Record),
                nocter_declarations::value_access::AggregateForm::Tuple(_) => {
                    nocter_syntax::decode_integer_literal(self.text(token)?)
                        .and_then(|index| usize::try_from(index).ok())
                        .map(ValueField::Tuple)
                        .filter(|&field| aggregate.component(field).is_some())
                }
            }
        } else {
            None
        };
        if let Some(field) = field {
            if let ValueField::Record(id) = field {
                let record = self
                    .program()
                    .record_identity(data)
                    .ok_or(Failure::InconsistentSyntax)?;
                if !self.context.record_field_visible(record, id) {
                    return self.reject(Rule::InaccessibleField, token);
                }
            }
            return Ok(field);
        }
        self.reject_message(
            Rule::UnknownField,
            token,
            format!(
                "{} has no field `{}`",
                self.data_label(data),
                self.text(token)?
            ),
            Vec::new(),
        )
    }
    fn fields(
        &self,
        mut data: TypeId,
        members: &[SyntaxToken],
    ) -> Result<(FieldPath, TypeId), Failure> {
        let mut fields = Vec::with_capacity(members.len());
        for &token in members {
            let field = self.field(data, token)?;
            data = self
                .program()
                .component(data, field)
                .ok_or(Failure::InconsistentSyntax)?
                .data;
            fields.push(field);
        }
        Ok((FieldPath::new(fields), data))
    }
    fn member_place(
        &mut self,
        base: AccessPlaceId,
        members: &[SyntaxToken],
    ) -> Result<AccessPlaceId, Failure> {
        let selected = self.selected_place(base).clone();
        let (fields, data) = self.fields(selected.data, members)?;
        let fields = selected.fields.concat(&fields);
        Ok(self.intern_place(selected.local, fields, data))
    }
    pub fn place(&mut self, node: NodeId, rule: Rule) -> Result<AccessPlaceId, Failure> {
        let (root, members) = self.selection(node)?;
        let Some(name) = self.name_token(root)? else {
            let message = if rule == Rule::MissingTransfer {
                "a produced value has no existing source to retire; remove postfix ^"
            } else {
                "this operation needs existing storage; bind the computed result before taking scoped access or updating it"
            };
            return self.reject_message(rule, node, message.to_owned(), Vec::new());
        };
        let place = self.place_name(name, rule)?;
        self.member_place(place, &members)
    }
    pub fn member_value(
        &mut self,
        node: NodeId,
        projection: Option<super::ir::Projection>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        if self.first_array_index(node)?.is_some() {
            return self.postfix_value(node, projection, reachable);
        }
        let (root, members) = self.selection(node)?;
        if let Some(name) = self.name_token(root)?
            && let Some(base) = self.named_place(name)?
        {
            let place = self.member_place(base, &members)?;
            return self.emit_place(node, place);
        }
        let source = self.expression_with_projection(root, None, None, projection, reachable)?;
        self.component_value(node, source, &members)
    }
    pub(super) fn component_value(
        &mut self,
        node: NodeId,
        source: AccessExpressionId,
        members: &[SyntaxToken],
    ) -> Result<AccessExpressionId, Failure> {
        let Some(access) = self.ty(source).access() else {
            return self.reject(Rule::UnknownResultType, node);
        };
        let (fields, data) = self.fields(access.data(), members)?;
        if self.expression_referent(source).is_some() {
            return self.emit(
                node,
                value_type(data, Mode::Read).with_continuation(self.ty(source).may_complete()),
                ExpressionKind::Select {
                    source,
                    fields,
                    action: SelectionAction::Access,
                },
            );
        }
        if access.mode() != Mode::Own {
            return Err(Failure::InconsistentSyntax);
        }
        if let ExpressionKind::Constant(mut value) = self
            .expressions
            .get(source)
            .ok_or(Failure::InconsistentSyntax)?
            .kind
            .clone()
        {
            let mut container = access.data();
            for &field in fields.fields() {
                let component = self
                    .program()
                    .component(container, field)
                    .ok_or(Failure::InconsistentSyntax)?;
                let index = self
                    .program()
                    .component_position(field)
                    .ok_or(Failure::InconsistentSyntax)?;
                let nocter_model::FrozenNode::Aggregate(children) = value.node() else {
                    return Err(Failure::InconsistentSyntax);
                };
                value = value
                    .at(*children.get(index).ok_or(Failure::InconsistentSyntax)?)
                    .ok_or(Failure::InconsistentSyntax)?;
                container = component.data;
            }
            return self.constant_value(node, data, &value);
        }
        let action = if self.copyable(data) {
            SelectionAction::CopyPayload
        } else {
            SelectionAction::MovePayload
        };
        self.emit(
            node,
            value_type(data, Mode::Own).with_continuation(self.ty(source).may_complete()),
            ExpressionKind::Select {
                source,
                fields,
                action,
            },
        )
    }
    pub(super) fn alias_value(
        &mut self,
        node: NodeId,
        rule: Rule,
        mode: Mode,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let node = self.unwrap(node)?;
        let mut root = node;
        while self.kind(root)? == NodeKind::PostfixExpression {
            root = self.unwrap(self.first_child(root)?)?;
        }
        if self.first_array_index(node)?.is_some()
            || matches!(
                self.kind(root)?,
                NodeKind::OutcomeExpression | NodeKind::RecoveryExpression
            )
        {
            let value = self.expression_with_projection(
                node,
                None,
                None,
                Some(super::ir::Projection::place(mode)),
                reachable,
            )?;
            if self.expression_referent(value).is_none() {
                return self.reject(rule, node);
            }
            return Ok(value);
        }
        self.place_value(node, rule)
    }
    pub fn place_value(&mut self, node: NodeId, rule: Rule) -> Result<AccessExpressionId, Failure> {
        let node = self.unwrap(node)?;
        let place = self.place(node, rule)?;
        self.emit_place(node, place)
    }
    pub(super) fn emit_place(
        &mut self,
        node: NodeId,
        place: AccessPlaceId,
    ) -> Result<AccessExpressionId, Failure> {
        self.emit(
            node,
            value_type(self.place_ty(place).data(), Mode::Read),
            ExpressionKind::Place(place),
        )
    }
    pub(super) fn intern_place(
        &mut self,
        local: AccessLocalId,
        fields: FieldPath,
        data: TypeId,
    ) -> AccessPlaceId {
        let key = (local, fields.clone());
        if let Some(&id) = self.place_index.get(&key) {
            return id;
        }
        let id = self.places.insert(Place {
            local,
            fields,
            data,
        });
        self.place_index.insert(key, id);
        id
    }
    pub fn selected_place(&self, place: AccessPlaceId) -> &Place {
        self.places.get(place).expect("own selected place")
    }
    pub fn place_ty(&self, place: AccessPlaceId) -> Ty {
        let selected = self.selected_place(place);
        Ty::new(selected.data, self.local_ty(selected.local).mode())
    }
    fn named_place(&mut self, token: SyntaxToken) -> Result<Option<AccessPlaceId>, Failure> {
        self.require_header(Namespace::Value, token)?;
        let spelling = self.text(token)?;
        if let Some(place) = self.lookup(spelling) {
            return Ok(Some(place));
        }
        if let Some(id) = self.program().symbols().get(spelling).and_then(|name| {
            self.program()
                .resolve_static_in(self.context.lookup_scope(), name)
        }) {
            if let Some(&local) = self.statics.get(&id) {
                return Ok(Some(self.intern_place(
                    local,
                    FieldPath::default(),
                    self.local_ty(local).data(),
                )));
            }
            let data = self
                .program()
                .static_declaration(id)
                .ok_or(Failure::InconsistentSyntax)?
                .result();
            let local = self.locals.insert(super::Local {
                data,
                kind: nocter_model::ValueBindingKind::ReadAlias,
                origin: super::LocalOrigin::Static(id),
            });
            self.origins
                .locals
                .insert(local, self.site(nocter_syntax::SyntaxOrigin::Token(token))?);
            self.statics.insert(id, local);
            return Ok(Some(self.intern_place(local, FieldPath::default(), data)));
        }
        Ok(None)
    }
    pub fn place_name(&mut self, token: SyntaxToken, rule: Rule) -> Result<AccessPlaceId, Failure> {
        if let Some(place) = self.named_place(token)? {
            return Ok(place);
        }
        let spelling = self.text(token)?;
        if self
            .program()
            .symbols()
            .get(spelling)
            .and_then(|symbol| {
                self.program()
                    .resolve_code_in(self.context.lookup_scope(), symbol)
            })
            .is_some()
        {
            if rule == Rule::MissingTransfer {
                return self.reject_message(
                    rule, token,
                    "this declaration constructs a value and has no owning source to retire; remove postfix ^".into(),
                    Vec::new(),
                );
            }
            return self.reject(rule, token);
        }
        self.reject(Rule::UnknownName, token)
    }
    pub fn expression_referent(&self, expr: AccessExpressionId) -> Option<super::Referent> {
        self.expressions.get(expr)?.referent(expr)
    }

    pub(super) fn require_writable_referent(
        &self,
        source: super::Referent,
        node: NodeId,
        rule: Rule,
    ) -> Result<(), Failure> {
        let roots = source
            .static_roots(|id| self.expressions.get(id), |id| self.blocks.get(id))
            .ok_or(Failure::InconsistentSyntax)?;
        for root in roots {
            match root {
                super::paths::ProjectionRoot::Place(place) => {
                    if !self.writable_place(place) {
                        return self.reject_local(rule, node, self.selected_place(place).local);
                    }
                }
                super::paths::ProjectionRoot::Temporary(source) => {
                    if self
                        .ty(source)
                        .normal()
                        .is_none_or(|ty| ty.mode() != Mode::Own)
                    {
                        return Err(Failure::InconsistentSyntax);
                    }
                }
            }
        }
        Ok(())
    }
    pub fn writable_place(&self, place: AccessPlaceId) -> bool {
        self.writable_local(self.selected_place(place).local)
    }
    pub fn writable_local(&self, local: AccessLocalId) -> bool {
        self.locals.get(local).expect("own local").kind.writable()
    }
}
