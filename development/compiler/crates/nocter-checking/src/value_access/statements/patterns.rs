//! A tuple value pattern forms its named leaves from one already supplied carrier.
use super::{
    AccessExpressionId, AccessStatementId, Checker, Failure, NodeId, Rule, Statement, SyntaxOrigin,
    TypeId, ValueBindingKind,
};
use nocter_declarations::value_access::{AggregateForm, PayloadSchema};
use nocter_model::{FieldPath, ValueField};
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
        let carrier = self.binding_local(
            SyntaxOrigin::Node(pattern),
            ValueBindingKind::FixedValue,
            data,
        )?;
        statements.push(self.statement(
            statement,
            Statement::Bind {
                local: carrier,
                value,
            },
        )?);
        let mut bindings = Vec::with_capacity(leaves.len());
        for leaf in leaves {
            let place = self.intern_place(carrier, leaf.path, leaf.data);
            // The caller has already supplied the tuple. Extraction transfers from this
            // anonymous owner, even when a leaf could also be copied.
            let value = self.transfer_place(leaf.syntax, place)?;
            let local = self.binding_local(SyntaxOrigin::Token(leaf.name), kind, leaf.data)?;
            statements.push(self.statement(leaf.syntax, Statement::Bind { local, value })?);
            bindings.push((leaf.name, local));
        }
        statements.push(self.statement(pattern, Statement::Drop(carrier))?);
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
