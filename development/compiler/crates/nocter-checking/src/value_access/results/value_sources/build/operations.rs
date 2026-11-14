//! Source definitions follow actual graph actions, without rechecking access or initialization.
use super::{Builder, CallValueSources, INVALID, Result, Slot, State, ValueSource, ValueSourceId};
use crate::value_access::{ArgumentAction, CallKind, ExpressionKind, Referent, flow};
use nocter_model::{AccessExpressionId, AccessFlowPointId, ValueApplication, ValuePath};
use std::collections::BTreeSet;

impl Builder<'_> {
    pub(super) fn operation(
        &mut self,
        point: AccessFlowPointId,
        operation: &flow::Operation,
        state: &mut State,
    ) -> Result<()> {
        match *operation {
            flow::Operation::Evaluate(id) => {
                let value = self.expression(point, id, state)?;
                self.define(Slot::Expression(id), value, state)?;
            }
            flow::Operation::Forward { source, result, .. }
            | flow::Operation::BooleanAnswer { source, result } => {
                let source = self.value(point, source, state)?;
                let value = self.push(
                    self.data(result)?,
                    Some(result),
                    ValueSource::Adapt { source },
                );
                self.define(Slot::Expression(result), value, state)?;
            }
            flow::Operation::BooleanConstant { result, .. } => {
                let value = self.push(self.data(result)?, Some(result), ValueSource::Opaque);
                self.define(Slot::Expression(result), value, state)?;
            }
            flow::Operation::PrepareArgument { call, argument } => {
                let ExpressionKind::Call { arguments, .. } =
                    &self.query.body.expressions.get(call).ok_or(INVALID)?.kind
                else {
                    return Err(INVALID);
                };
                let source =
                    self.value(point, arguments.get(argument).ok_or(INVALID)?.value, state)?;
                self.define(
                    Slot::Argument {
                        call,
                        index: argument,
                    },
                    source,
                    state,
                )?;
            }
            flow::Operation::CommitCall(call) => self.commit(point, call, state)?,
            flow::Operation::Bind { local, value } => {
                let source = self.value(point, value, state)?;
                self.define(Slot::Local(local), source, state)?;
            }
            flow::Operation::Assign { target, value } => {
                let value = self.value(point, value, state)?;
                self.assign(point, target, value, state)?;
            }
            flow::Operation::CompoundAssign { target, .. } => {
                let value = self.push(None, None, ValueSource::Opaque);
                self.assign(point, target, value, state)?;
            }
            flow::Operation::StageReturn(id) => {
                let value = self.value(point, id, state)?;
                self.returned.insert(value);
                self.define(Slot::Return, value, state)?;
            }
            // Source identities survive retirement; the existing ownership and initialization
            // products alone determine whether an old identity can still be accessed.
            _ => {}
        }
        Ok(())
    }
    fn commit(
        &mut self,
        point: AccessFlowPointId,
        call: AccessExpressionId,
        state: &mut State,
    ) -> Result<()> {
        let ExpressionKind::Call {
            kind, arguments, ..
        } = &self.query.body.expressions.get(call).ok_or(INVALID)?.kind
        else {
            return Err(INVALID);
        };
        let values = (0..arguments.len())
            .map(|index| self.read(point, Slot::Argument { call, index }, state))
            .collect::<Box<[_]>>();
        let callee = if *kind == CallKind::Value {
            *values.first().ok_or(INVALID)?
        } else {
            let selected = super::super::super::applications::callee(self.query, call)?;
            self.push(
                self.query.program().function_data(selected),
                None,
                ValueSource::Function(selected),
            )
        };
        self.calls.entry(call).or_default().push(CallValueSources {
            kind: *kind,
            invocation: kind.invocation(arguments).ok_or(INVALID)?,
            parameters: arguments
                .iter()
                .enumerate()
                .filter_map(|(index, arg)| {
                    arg.binding
                        .as_ref()
                        .map(|binding| (binding.parameter, index))
                })
                .collect(),
            origin: self.origin.ok_or(INVALID)?,
            callee,
            arguments: values,
        });
        for (index, argument) in arguments.iter().enumerate() {
            if argument.operation != nocter_model::ValueInputOperation::Edit {
                continue;
            }
            let paths = match argument.action {
                ArgumentAction::ReserveUpdate(target) => self.paths(target)?,
                // This action lends a freshly produced owned temporary, not its source place.
                ArgumentAction::AccessOwnedValue => {
                    vec![(Slot::Expression(argument.value), ValuePath::default())]
                }
                _ => continue,
            };
            for (slot, path) in paths {
                let source = self.read(point, slot, state);
                let data = self.slot_data(slot)?;
                let value = self.push(
                    data,
                    None,
                    ValueSource::CallWrite {
                        call,
                        argument: index,
                        source,
                        path,
                    },
                );
                self.define(slot, value, state)?;
            }
        }
        let value = self.push(
            self.data(call)?,
            Some(call),
            ValueSource::CallResult(ValueApplication {
                owner: self.query.code(),
                call,
            }),
        );
        self.define(Slot::Expression(call), value, state)?;
        Ok(())
    }
    fn slot_data(&self, slot: Slot) -> Result<Option<nocter_model::TypeId>> {
        match slot {
            Slot::Return => Ok(Some(self.query.result_data())),
            Slot::Local(local) => Ok(Some(self.query.body.locals.get(local).ok_or(INVALID)?.data)),
            Slot::Expression(expression) => self.data(expression),
            Slot::Argument { call, index } => {
                let ExpressionKind::Call { arguments, .. } =
                    &self.query.body.expressions.get(call).ok_or(INVALID)?.kind
                else {
                    return Err(INVALID);
                };
                self.data(arguments.get(index).ok_or(INVALID)?.value)
            }
        }
    }
    fn assign(
        &mut self,
        point: AccessFlowPointId,
        target: Referent,
        value: ValueSourceId,
        state: &mut State,
    ) -> Result<()> {
        let paths = self.paths(target)?;
        let weak = paths.len() != 1;
        for (slot, path) in paths {
            let source = self.read(point, slot, state);
            let value = self.push(
                self.slot_data(slot)?,
                None,
                ValueSource::Replace {
                    source,
                    value,
                    weak: weak
                        || path
                            .fields()
                            .iter()
                            .any(|field| matches!(field, nocter_model::ValueProjection::Array(()))),
                    path,
                },
            );
            self.define(slot, value, state)?;
        }
        Ok(())
    }
    pub(super) fn paths(&self, target: Referent) -> Result<Vec<(Slot, ValuePath<()>)>> {
        if let Some(paths) = self.aliases.producer_paths(self.query, target) {
            return paths
                .into_iter()
                .map(|(root, path)| {
                    let (slot, _) = super::super::root(self.query, root)?;
                    Ok((slot, path))
                })
                .collect();
        }
        let paths = target
            .projections(
                |id| self.query.body.expressions.get(id),
                |id| self.query.body.blocks.get(id),
                crate::value_access::paths::ProjectionRoots::All,
                |_| (),
                |_, _| Some(()),
            )
            .ok_or(INVALID)?;
        let mut result = BTreeSet::new();
        for (root, suffix) in paths {
            let (slot, _) = super::super::root(self.query, root)?;
            let path = if let crate::value_access::paths::ProjectionRoot::Place(place) = root {
                let place = self.query.body.places.get(place).ok_or(INVALID)?;
                let prefix = crate::value_access::paths::known_by(&place.fields, |_| ());
                prefix.concat(&suffix)
            } else {
                suffix
            };
            result.insert((slot, path));
        }
        Ok(result.into_iter().collect())
    }
}
