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
            query.body().expressions().iter().all(|(id, expression)| {
                separable_call(&expression.kind)
                    && occurrence_separable(query, id, &expression.kind)
            })
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
        if key.environment().is_some()
            || key
                .witnesses()
                .values()
                .any(|witness| witness.environment().is_some())
        {
            if self.exact.insert(key.clone()) {
                self.pending.push(key);
            }
            return Ok(());
        }
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
                        .map(|(&parameter, witness)| (parameter, BTreeSet::from([witness.clone()])))
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
        for (&parameter, witness) in key.witnesses() {
            if !frame
                .witnesses
                .get_mut(&parameter)
                .ok_or(INVALID)?
                .insert(witness.clone())
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
                        .map(|(&formal, original)| {
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
fn occurrence_separable(
    query: &Query,
    id: nocter_model::AccessExpressionId,
    expression: &ExpressionKind,
) -> bool {
    if !super::super::instances::is_completed_call(expression) {
        return true;
    }
    let Some(recipe) = super::call_support::source_recipe(query, id) else {
        return query.result_evidence.is_some(); // A dormant definition contract, not an actual commit.
    };
    let unary = |witness: nocter_declarations::value_access::FunctionWitness| {
        matches!(witness.origin, ValueFunctionOrigin::Parameter(_))
            || query.program().concrete_witness(witness).is_ok()
    };
    let Some(&callee) = recipe.callees.first().filter(|_| recipe.callees.len() == 1) else {
        return false;
    };
    unary(callee)
        && (!matches!(callee.origin, ValueFunctionOrigin::Parameter(_)) || recipe.inputs.is_empty())
        && recipe
            .inputs
            .values()
            .all(|terms| terms.len() == 1 && terms.first().is_some_and(|&witness| unary(witness)))
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

    #[test]
    fn independent_source_positions_cover_joint_inputs_without_enumerating_joint_frames() {
        let source = "func first(): void {}\nfunc second(): void {}\nfunc third(): void {}\nfunc fourth(): void {}\nfunc apply(a: func(): void, b: func(): void, c: func(): void, d: func(): void): void { a()\nb()\nc()\nd() }";
        let (sources, tree) = input(source);
        let unit = SourceUnit::prepare(bound(&sources, &tree))
            .unwrap()
            .into_unit();
        let program = unit.program();
        let code = |name| {
            program
                .resolve_code(program.symbols().get(name).unwrap())
                .unwrap()
        };
        let apply = code("apply");
        let callbacks = ["first", "second", "third", "fourth"].map(code);
        let parameters = program.codes().get(apply).unwrap().parameters();
        let scope = unit
            .bodies()
            .map(|(_, body)| (body.typed().code(), body.typed()))
            .collect();
        let mut discovery = Discovery::new(&scope);
        assert!(discovery.separable);
        for _ in 0..2 {
            for a in callbacks {
                for b in callbacks {
                    for c in callbacks {
                        for d in callbacks {
                            let frame = program
                                .bind_code(
                                    apply.into(),
                                    parameters.iter().copied().zip([a, b, c, d]).map(
                                        |(parameter, callback)| (parameter.into(), callback.into()),
                                    ),
                                )
                                .unwrap();
                            discovery.insert(program, frame).unwrap();
                        }
                    }
                }
            }
        }
        let frames = std::iter::from_fn(|| discovery.next()).collect::<Vec<_>>();
        assert_eq!(
            frames.len(),
            1 + parameters.len() * (callbacks.len() - 1),
            "only independent substitutions, rather than 4^4 joint frames"
        );
        for &parameter in parameters {
            let covered = frames
                .iter()
                .map(|frame| frame.witnesses()[&parameter.into()].code())
                .collect::<BTreeSet<_>>();
            assert_eq!(
                covered,
                callbacks.into_iter().map(ValueCode::from).collect()
            );
            assert!(
                frames
                    .iter()
                    .all(|frame| frame.witnesses().len() == parameters.len())
            );
        }
    }
}
