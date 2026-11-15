//! One call owns independent result-leaf templates and one atomic containing-type extension.
use super::{BodyTypeError, BodyTypes};
use crate::value_access::{FunctionWitness, functions};
use nocter_model::{
    AccessExpressionId, TypeId, TypeKind, TypeTransaction, ValueApplication, ValueAppliedResult,
    ValueCode, ValueFunctionOrigin,
};
use std::collections::BTreeMap;

impl BodyTypes {
    /// Intern a selected call's result descriptors without supplying a returned value.
    /// # Errors
    /// Rejects a non-factory callee, foreign owner or inconsistent template/type extension.
    pub fn application(
        &mut self,
        owner: ValueCode,
        call: AccessExpressionId,
        callee: impl Into<FunctionWitness>,
    ) -> Result<TypeId, BodyTypeError> {
        if owner.root() != self.owner {
            return Err(BodyTypeError::InvalidApplication);
        }
        let result = self
            .program
            .function_contract(callee.into())
            .ok_or(BodyTypeError::InvalidApplication)?
            .result;
        let mut types = self.program.types.transaction();
        let mut projected = BTreeMap::new();
        let mut applications = BTreeMap::new();
        let mut pending = vec![result];
        while let Some(data) = pending.pop() {
            if projected.contains_key(&data) {
                continue;
            }
            let kind = self
                .program
                .types
                .store()
                .get(data)
                .ok_or(BodyTypeError::InvalidExtension)?;
            let missing = children(kind)
                .into_iter()
                .filter(|child| !projected.contains_key(child))
                .collect::<Vec<_>>();
            if !missing.is_empty() {
                pending.push(data);
                pending.extend(missing.into_iter().rev());
                continue;
            }
            let selected = if let TypeKind::ValueFunction { origin, invocation } = kind {
                let application = ValueAppliedResult {
                    application: ValueApplication { owner, call },
                    ordinal: applications.len(),
                };
                let template = FunctionWitness {
                    origin: *origin,
                    invocation: *invocation,
                };
                if self
                    .program
                    .application_template(application)
                    .is_some_and(|old| old != template)
                {
                    return Err(BodyTypeError::InvalidApplication);
                }
                applications.insert(application, template);
                functions::intern_views(
                    &mut types,
                    ValueFunctionOrigin::Application(application),
                    *invocation,
                )
            } else {
                rebuild(&mut types, kind, data, &projected)?
            };
            projected.insert(data, selected);
        }
        if applications.is_empty() {
            return Err(BodyTypeError::InvalidApplication);
        }
        let site = ValueApplication { owner, call };
        let existing = self
            .program
            .applications
            .range(
                ValueAppliedResult {
                    application: site,
                    ordinal: 0,
                }..=ValueAppliedResult {
                    application: site,
                    ordinal: usize::MAX,
                },
            )
            .count();
        if existing != 0 && existing != applications.len() {
            return Err(BodyTypeError::InvalidApplication);
        }
        self.program.types = types
            .commit(&self.program.types)
            .map_err(|_| BodyTypeError::InvalidExtension)?;
        self.program.applications.extend(applications);
        Ok(projected[&result])
    }
}

fn children(kind: &TypeKind) -> Vec<TypeId> {
    match kind {
        TypeKind::Optional(data) | TypeKind::Fallible(data) => vec![*data],
        TypeKind::FixedArray { element, .. } => vec![*element],
        TypeKind::Tuple(fields) => fields.iter().collect(),
        _ => Vec::new(),
    }
}
fn rebuild(
    types: &mut TypeTransaction,
    kind: &TypeKind,
    original: TypeId,
    projected: &BTreeMap<TypeId, TypeId>,
) -> Result<TypeId, BodyTypeError> {
    let kind = match kind {
        TypeKind::Optional(data) => TypeKind::Optional(projected[data]),
        TypeKind::Fallible(data) => TypeKind::Fallible(projected[data]),
        TypeKind::FixedArray { element, length } => TypeKind::FixedArray {
            element: projected[element],
            length: *length,
        },
        TypeKind::Tuple(fields) => {
            let fields = fields
                .iter()
                .map(|data| projected[&data])
                .collect::<Vec<_>>();
            TypeKind::Tuple(nocter_model::TupleElements::new(
                fields[0],
                fields[1],
                fields[2..].iter().copied(),
            ))
        }
        _ => return Ok(original),
    };
    types
        .intern(kind)
        .map_err(|_| BodyTypeError::InvalidExtension)
}
