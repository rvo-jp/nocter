//! Tuple bindings form their named leaves from one supplied carrier or prepared place.
use super::{
    AccessExpressionId, AccessStatementId, Checker, Failure, NodeId, Rule, Statement, SyntaxOrigin,
    TypeId, ValueBindingKind,
};
use crate::value_access::ir::{Boundary, ExpressionKind, SelectionAction, value_type};
use nocter_declarations::value_access::{AggregateForm, PayloadSchema};
use nocter_model::{FieldPath, ValueAccess, ValueAccessMode as Mode, ValueField};
use nocter_syntax::SyntaxToken;
use std::collections::BTreeSet;

struct Leaf {
    syntax: NodeId,
    name: SyntaxToken,
    data: TypeId,
    path: FieldPath,
}

impl Checker<'_, '_> {
    pub(super) fn tuple_binding(
        &mut self,
        statement: NodeId,
        pattern: NodeId,
        kind: ValueBindingKind,
        data: TypeId,
        value: AccessExpressionId,
        statements: &mut Vec<AccessStatementId>,
    ) -> Result<(), Failure> {
        let leaves = self.tuple_leaves(pattern, data)?;
        let mode = kind.access_mode();
        let carrier = if mode == Mode::Own {
            let local = self.binding_local(
                SyntaxOrigin::Node(pattern),
                ValueBindingKind::FixedValue,
                data,
            )?;
            statements.push(self.statement(statement, Statement::Bind { local, value })?);
            Some(local)
        } else {
            statements.push(self.statement(statement, Statement::PreparePlace(value))?);
            None
        };
        let mut bindings = Vec::with_capacity(leaves.len());
        for leaf in leaves {
            let leaf_value = if let Some(carrier) = carrier {
                // The tuple is already supplied. Extract each named leaf from its anonymous owner.
                let place = self.intern_place(carrier, leaf.path, leaf.data);
                self.transfer_place(leaf.syntax, place)?
            } else {
                let projected = self.emit(
                    leaf.syntax,
                    value_type(leaf.data, Mode::Read)
                        .with_continuation(self.ty(value).may_complete()),
                    ExpressionKind::Select {
                        source: value,
                        fields: leaf.path,
                        action: SelectionAction::PreparedAccess,
                    },
                )?;
                self.boundary(
                    leaf.syntax,
                    projected,
                    Boundary::Binding(ValueAccess::new(leaf.data, mode)),
                )?
            };
            let local = self.binding_local(SyntaxOrigin::Token(leaf.name), kind, leaf.data)?;
            statements.push(self.statement(
                leaf.syntax,
                Statement::Bind {
                    local,
                    value: leaf_value,
                },
            )?);
            bindings.push((leaf.name, local));
        }
        if let Some(carrier) = carrier {
            statements.push(self.statement(pattern, Statement::Drop(carrier))?);
        }
        for (name, local) in bindings {
            self.bind_local(self.text(name)?.to_owned(), local);
        }
        Ok(())
    }

    fn tuple_leaves(&self, pattern: NodeId, data: TypeId) -> Result<Vec<Leaf>, Failure> {
        let mut pending = vec![(pattern, data, FieldPath::default())];
        let mut names = BTreeSet::new();
        let mut leaves = Vec::new();
        while let Some((syntax, data, path)) = pending.pop() {
            let items = self.children(syntax);
            if items.is_empty() {
                let name = self.token(syntax)?;
                let spelling = self.text(name)?;
                if spelling == "_" {
                    continue;
                }
                if !names.insert(spelling) || self.shadows_name(spelling, name)? {
                    return self.reject(Rule::ShadowedName, name);
                }
                leaves.push(Leaf {
                    syntax,
                    name,
                    data,
                    path,
                });
                continue;
            }
            let fields = match self.program().data_schema(data) {
                Some(PayloadSchema::Aggregate(aggregate)) => match aggregate.form() {
                    AggregateForm::Tuple(fields) if fields.len() == items.len() => fields,
                    _ => return self.tuple_shape_mismatch(syntax, items.len()),
                },
                _ => return self.tuple_shape_mismatch(syntax, items.len()),
            };
            pending.extend(
                items
                    .into_iter()
                    .zip(fields.iter().copied())
                    .enumerate()
                    .rev()
                    .map(|(position, (syntax, data))| {
                        (syntax, data, path.project(ValueField::Tuple(position)))
                    }),
            );
        }
        if leaves.is_empty() {
            return self.reject_message(
                Rule::AnnotatedDiscard,
                pattern,
                "a tuple binding must name at least one component".into(),
                Vec::new(),
            );
        }
        Ok(leaves)
    }

    fn tuple_shape_mismatch<T>(&self, syntax: NodeId, length: usize) -> Result<T, Failure> {
        self.reject_message(
            Rule::TypeMismatch,
            syntax,
            format!("this binding pattern requires a tuple with {length} components"),
            Vec::new(),
        )
    }
}
