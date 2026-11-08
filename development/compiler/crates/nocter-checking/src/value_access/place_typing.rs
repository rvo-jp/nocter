//! Resolve existing places once. Published bodies retain IDs, field paths and selected data only.
use super::{
    Failure, Rule,
    checker::Checker,
    ir::{ExpressionKind, Place, ValueType},
};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::{
    AccessExpressionId, AccessLocalId, AccessPlaceId, FieldPath, TypeId, ValueAccess as Ty,
    ValueAccessMode as Mode,
};
use nocter_syntax::{NodeId, NodeKind, SyntaxToken};

impl Checker<'_, '_> {
    pub fn place(&mut self, node: NodeId, rule: Rule) -> Result<AccessPlaceId, Failure> {
        let mut current = self.unwrap(node)?;
        let mut members = Vec::new();
        let root = loop {
            if let Some(name) = self.name_token(current)? {
                break self.place_name(name, rule)?;
            }
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
                return self.reject(rule, node);
            }
        };
        let mut data = self.local_ty(root).data();
        let mut fields = Vec::with_capacity(members.len());
        for token in members.into_iter().rev() {
            let Some(PayloadSchema::Record(record)) = self.program().data_schema(data) else {
                return self.reject(Rule::UnknownField, token);
            };
            let field = self
                .program()
                .symbols()
                .get(self.text(token)?)
                .and_then(|name| record.resolve_field(name));
            let Some(field) = field else {
                return self.reject(Rule::UnknownField, token);
            };
            data = self
                .program()
                .fields()
                .get(field)
                .ok_or(Failure::InconsistentSyntax)?
                .data();
            fields.push(field);
        }
        Ok(self.intern_place(root, FieldPath::new(fields), data))
    }
    pub fn place_value(&mut self, node: NodeId, rule: Rule) -> Result<AccessExpressionId, Failure> {
        let node = self.unwrap(node)?;
        let place = self.place(node, rule)?;
        self.emit(
            node,
            ValueType::Value(Ty::new(self.place_ty(place).data(), Mode::Read)),
            ExpressionKind::Place(place),
        )
    }
    fn intern_place(
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
    pub fn place_name(&self, token: SyntaxToken, rule: Rule) -> Result<AccessLocalId, Failure> {
        let spelling = self.text(token)?;
        if let Some(local) = self.lookup(spelling) {
            return Ok(local);
        }
        if self
            .program()
            .symbols()
            .get(spelling)
            .and_then(|symbol| self.program().resolve(symbol))
            .is_some()
        {
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
    pub fn annotation(&self, node: NodeId) -> Result<TypeId, Failure> {
        let named = self.direct(node, NodeKind::NamedType)?;
        let token = self.token(named)?;
        let Some(data) = self.program().type_named(self.text(token)?) else {
            return self.reject(Rule::UnknownType, token);
        };
        Ok(data)
    }

    pub fn writable_place(&self, place: AccessPlaceId) -> bool {
        self.writable_local(self.selected_place(place).local)
    }
    pub fn writable_local(&self, local: AccessLocalId) -> bool {
        self.locals.get(local).expect("own local").kind.writable()
    }
}
