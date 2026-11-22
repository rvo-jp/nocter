//! Published result identities select descriptor paths, never initialized values or addresses.
use super::{PayloadSchema, Program};
use nocter_model::{ValueCallableResult, ValueFunctionOrigin, ValuePath, ValueProjection};
use std::collections::BTreeSet;

impl Program {
    /// Locate one published callable-result descriptor by its declaration-owned identity.
    /// Array selectors describe the homogeneous element schema, including an empty array's
    /// uninhabited element descriptor. This path grants no bounds, value or storage evidence.
    /// Shared schemas are visited once; array cardinality never expands this traversal.
    #[must_use]
    pub fn callable_result_path(&self, result: ValueCallableResult) -> Option<ValuePath<()>> {
        let code = self.codes.get(result.owner)?;
        code.result_contracts().get(result.ordinal)?;
        let mut pending = vec![(code.result(), ValuePath::default())];
        let mut seen = BTreeSet::new();
        while let Some((data, path)) = pending.pop() {
            if !seen.insert(data) {
                continue;
            }
            match self.data_schema(data)? {
                PayloadSchema::Function(witness)
                    if witness.origin == ValueFunctionOrigin::Result(result) =>
                {
                    return Some(path);
                }
                PayloadSchema::Aggregate(schema) => {
                    pending.extend(schema.components().map(|component| {
                        (
                            component.data,
                            path.project(match component.field {
                                ValueProjection::Record(id) => ValueProjection::Record(id),
                                ValueProjection::Tuple(index) => ValueProjection::Tuple(index),
                                ValueProjection::Variant(primary) => {
                                    ValueProjection::Variant(primary)
                                }
                                ValueProjection::Array(_) => ValueProjection::Array(()),
                            }),
                        )
                    }));
                }
                PayloadSchema::Array(array) => {
                    pending.push((array.element(), path.project(ValueProjection::Array(()))));
                }
                PayloadSchema::Optional(payload) | PayloadSchema::Fallible(payload) => {
                    pending.push((payload, path.project(ValueProjection::Variant(true))));
                }
                PayloadSchema::Builtin(_)
                | PayloadSchema::Function(_)
                | PayloadSchema::Future(_) => {}
            }
        }
        None
    }
}
