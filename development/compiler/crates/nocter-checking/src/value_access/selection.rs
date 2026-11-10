//! One member-chain and field resolver for existing places, computed values and construction.
use super::{
    Failure, Rule,
    checker::Checker,
    ir::{ExpressionKind, Place, SelectionAction, value_type},
};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::{
    AccessExpressionId, AccessFieldId, AccessLocalId, AccessPlaceId, FieldPath, TypeId,
    ValueAccess as Ty, ValueAccessMode as Mode,
};
use nocter_syntax::{NodeId, NodeKind, SyntaxToken};

impl Checker<'_, '_> {
    fn selection(&self, node: NodeId) -> Result<(NodeId, Vec<SyntaxToken>), Failure> {
        let mut current = self.unwrap(node)?;
        let mut members = Vec::new();
        loop {
            let children = self.children(current);
            if self.kind(current)? == NodeKind::PostfixExpression
                && let [base, suffix] = children.as_slice()
                && self.kind(*suffix)? == NodeKind::MemberSuffix
            {
                members.push(
                    nocter_syntax::direct_identifier(self.syntax().tree(), *suffix)
                        .ok_or(Failure::InconsistentSyntax)?,
                );
                current = self.unwrap(*base)?;
            } else {
                members.reverse();
                return Ok((current, members));
            }
        }
    }
    pub fn transfer(&mut self, node: NodeId) -> Result<AccessExpressionId, Failure> {
        let source = self.unwrap(self.first_child(node)?)?;
        if self.kind(source)? == NodeKind::TransferExpression {
            return self.reject_message(
                Rule::MissingTransfer,
                node,
                "the source is already marked for transfer; remove the repeated postfix ^".into(),
                Vec::new(),
            );
        }
        let place = self.place(source, Rule::MissingTransfer)?;
        self.transfer_place(node, place)
    }
    pub(super) fn transfer_place(
        &mut self,
        node: NodeId,
        place: AccessPlaceId,
    ) -> Result<AccessExpressionId, Failure> {
        let access = self.place_ty(place);
        if access.mode() != Mode::Own {
            return self.reject_message(
                Rule::MissingTransfer, node,
                "postfix transfer requires an owning source; scoped access cannot surrender its referent".into(),
                Vec::new(),
            );
        }
        self.emit(
            node,
            value_type(access.data(), Mode::Own),
            ExpressionKind::Move(place),
        )
    }
    pub fn field(&self, data: TypeId, token: SyntaxToken) -> Result<AccessFieldId, Failure> {
        let schema = self
            .program()
            .data_schema(data)
            .ok_or(Failure::InconsistentSyntax)?;
        let field = if let PayloadSchema::Record(record) = schema {
            self.program()
                .symbols()
                .get(self.text(token)?)
                .and_then(|name| record.resolve_field(name))
        } else {
            None
        };
        if let Some(field) = field {
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
                .fields()
                .get(field)
                .ok_or(Failure::InconsistentSyntax)?
                .data();
            fields.push(field);
        }
        Ok((FieldPath::new(fields), data))
    }
    fn local_selection(
        &mut self,
        local: AccessLocalId,
        members: &[SyntaxToken],
    ) -> Result<AccessPlaceId, Failure> {
        let (fields, data) = self.fields(self.local_ty(local).data(), members)?;
        Ok(self.intern_place(local, fields, data))
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
        let local = self.place_name(name, rule)?;
        self.local_selection(local, &members)
    }
    pub fn member_value(
        &mut self,
        node: NodeId,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let (root, members) = self.selection(node)?;
        if let Some(name) = self.name_token(root)?
            && let Some(local) = self.named_place(name)?
        {
            let place = self.local_selection(local, &members)?;
            return self.emit_place(node, place);
        }
        let source = self.expression(root, None, reachable)?;
        let Some(access) = self.ty(source).access() else {
            return self.reject(Rule::UnknownResultType, root);
        };
        if access.mode() != Mode::Own {
            return Err(Failure::InconsistentSyntax);
        }
        let (fields, data) = self.fields(access.data(), &members)?;
        if let ExpressionKind::Constant(mut value) = self
            .expressions
            .get(source)
            .ok_or(Failure::InconsistentSyntax)?
            .kind
            .clone()
        {
            let mut container = access.data();
            for &field in fields.fields() {
                let Some(PayloadSchema::Record(schema)) = self.program().data_schema(container)
                else {
                    return Err(Failure::InconsistentSyntax);
                };
                let index = schema
                    .fields()
                    .iter()
                    .position(|&candidate| candidate == field)
                    .ok_or(Failure::InconsistentSyntax)?;
                let nocter_model::FrozenNode::Record(children) = value.node() else {
                    return Err(Failure::InconsistentSyntax);
                };
                value = value
                    .at(*children.get(index).ok_or(Failure::InconsistentSyntax)?)
                    .ok_or(Failure::InconsistentSyntax)?;
                container = self
                    .program()
                    .fields()
                    .get(field)
                    .ok_or(Failure::InconsistentSyntax)?
                    .data();
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
    fn named_place(&mut self, token: SyntaxToken) -> Result<Option<AccessLocalId>, Failure> {
        let spelling = self.text(token)?;
        if let Some(local) = self.lookup(spelling) {
            return Ok(Some(local));
        }
        if let Some(id) = self
            .program()
            .symbols()
            .get(spelling)
            .and_then(|name| self.program().resolve_static(name))
        {
            if let Some(&local) = self.statics.get(&id) {
                return Ok(Some(local));
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
            return Ok(Some(local));
        }
        Ok(None)
    }
    pub fn place_name(&mut self, token: SyntaxToken, rule: Rule) -> Result<AccessLocalId, Failure> {
        if let Some(local) = self.named_place(token)? {
            return Ok(local);
        }
        let spelling = self.text(token)?;
        if self
            .program()
            .symbols()
            .get(spelling)
            .and_then(|symbol| self.program().resolve_code(symbol))
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
    pub fn place_expression(&self, expr: AccessExpressionId) -> Option<AccessPlaceId> {
        match self.expressions.get(expr)?.kind {
            ExpressionKind::Place(place) => Some(place),
            _ => None,
        }
    }

    pub fn writable_place(&self, place: AccessPlaceId) -> bool {
        self.writable_local(self.selected_place(place).local)
    }
    pub fn writable_local(&self, local: AccessLocalId) -> bool {
        self.locals.get(local).expect("own local").kind.writable()
    }
}
