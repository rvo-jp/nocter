//! Body discovery can forget joint witness correlations only for independent call positions.
use super::super::{ExpressionKind, Query, flow};
use nocter_declarations::value_access::{CodeInstanceKey, ConcreteWitness, Program};
use nocter_model::{ValueCode, ValueFunctionOrigin, ValueParameter};
use std::collections::{BTreeMap, BTreeSet};
const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;

struct Frame {
    representative: CodeInstanceKey,
    witnesses: BTreeMap<ValueParameter, BTreeSet<ConcreteWitness>>,
}
/// This projection discovers bodies only: no condition, resource, layout or plan equivalence.
/// Fixed callees forward unary witness terms. A parameter-selected target may be factored only
/// when it receives no callable witnesses: otherwise target/argument correlations matter.
pub(super) struct Discovery {
    separable: bool,
    frames: BTreeMap<ValueCode, Frame>,
    independent: BTreeSet<ValueCode>,
    plain: BTreeSet<ValueCode>,
    exact: BTreeSet<CodeInstanceKey>,
    pending: Vec<CodeInstanceKey>,
}
impl Discovery {
    pub(super) fn new(scope: &BTreeMap<ValueCode, &Query>) -> Self {
        let separable = scope.values().all(|query| {
            query
                .body()
                .expressions()
                .iter()
                .all(|(_, expression)| separable_call(&expression.kind))
        });
        Self {
            separable,
            frames: BTreeMap::new(),
            independent: super::call_support::witness_independent(scope),
            plain: BTreeSet::new(),
            exact: BTreeSet::new(),
            pending: Vec::new(),
        }
    }
    pub(super) fn insert(
        &mut self,
        program: &Program,
        key: CodeInstanceKey,
    ) -> Result<(), flow::IntegrityError> {
        if self.independent.contains(&key.code()) {
            if self.plain.insert(key.code()) {
                self.pending.push(key);
            }
            return Ok(());
        }
        if !self.separable {
            if self.exact.insert(key.clone()) {
                self.pending.push(key);
            }
            return Ok(());
        }
        let Some(frame) = self.frames.get_mut(&key.code()) else {
            self.frames.insert(
                key.code(),
                Frame {
                    witnesses: key
                        .witnesses()
                        .iter()
                        .map(|(&parameter, &witness)| (parameter, BTreeSet::from([witness])))
                        .collect(),
                    representative: key.clone(),
                },
            );
            self.pending.push(key);
            return Ok(());
        };
        if frame.witnesses.len() != key.witnesses().len() {
            return Err(INVALID);
        }
        for (&parameter, &witness) in key.witnesses() {
            if !frame
                .witnesses
                .get_mut(&parameter)
                .ok_or(INVALID)?
                .insert(witness)
            {
                continue;
            }
            // Vary one independent witness at a time; declaration-owned binding still checks
            // complete formal coverage and view compatibility. No owner/value is merged.
            let variant = program
                .bind_code(
                    key.code(),
                    frame
                        .representative
                        .witnesses()
                        .iter()
                        .map(|(&formal, &original)| {
                            (
                                formal,
                                if formal == parameter {
                                    witness
                                } else {
                                    original
                                }
                                .witness(),
                            )
                        }),
                )
                .map_err(|_| INVALID)?;
            self.pending.push(variant);
        }
        Ok(())
    }
    pub(super) fn next(&mut self) -> Option<CodeInstanceKey> {
        self.pending.pop()
    }
}

fn separable_call(expression: &ExpressionKind) -> bool {
    if !super::super::instances::is_completed_call(expression) {
        return true;
    }
    let ExpressionKind::Call {
        target, arguments, ..
    } = expression
    else {
        return false;
    };
    target.code().is_some()
        || (matches!(target, ValueFunctionOrigin::Parameter(_))
            && arguments.iter().all(|argument| argument.binding.is_none()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_access::{
        accepted::SourceUnit,
        tests::{bound, input},
    };

    #[test]
    fn a_selected_target_receiving_another_callable_preserves_joint_bindings() {
        let source = "func sink(action: func(): void): void {}\nfunc dispatch(action: func(): void): void { sink(action) }";
        let (sources, tree) = input(source);
        let unit = SourceUnit::prepare(bound(&sources, &tree))
            .unwrap()
            .into_unit();
        let program = unit.program();
        let dispatch = program
            .resolve_code(program.symbols().get("dispatch").unwrap())
            .unwrap();
        let formal = program.codes().get(dispatch).unwrap().parameters()[0];
        let body = unit.bodies().find(|(id, _)| *id == dispatch).unwrap().1;
        let mut call = body
            .typed()
            .body()
            .expressions()
            .iter()
            .find_map(|(_, expression)| {
                matches!(expression.kind, ExpressionKind::Call { .. })
                    .then(|| expression.kind.clone())
            })
            .unwrap();
        assert!(separable_call(&call));
        let ExpressionKind::Call { target, .. } = &mut call else {
            unreachable!();
        };
        // Nested callable headers are not qualified yet. Test the discovery boundary directly:
        // a selected target receiving real callable-binding metadata cannot lose correlations.
        *target = ValueFunctionOrigin::Parameter(formal.into());
        assert!(!separable_call(&call));
    }
}
