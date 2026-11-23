//! Original value operands are read at the selected preparation/evaluation phase.
use super::{Builder, INVALID, Result, Slot, State, ValueSource, ValueSourceId};
use crate::value_access::{ExpressionKind, Referent};
use nocter_declarations::value_access::FunctionWitness;
use nocter_model::{AccessExpressionId, AccessFlowPointId, ValueFunctionOrigin, ValueInvocation};

impl Builder<'_> {
    pub(super) fn expression(
        &mut self,
        point: AccessFlowPointId,
        id: AccessExpressionId,
        state: &State,
    ) -> Result<ValueSourceId> {
        let expression = self.query.body.expressions.get(id).ok_or(INVALID)?;
        let kind = &expression.kind;
        let value = match kind {
            ExpressionKind::Function(code) => ValueSource::Function(FunctionWitness {
                origin: ValueFunctionOrigin::Declaration(*code),
                invocation: ValueInvocation::Read,
            }),
            ExpressionKind::Closure { closure, capture } => {
                let environment = self.value(point, *capture, state)?;
                if let Some(previous) = self.creations.get(closure).copied() {
                    let merged = self.push(
                        None,
                        None,
                        ValueSource::Alternatives([previous, environment].into()),
                    );
                    self.creations.insert(*closure, merged);
                } else {
                    self.creations.insert(*closure, environment);
                }
                ValueSource::Closure {
                    closure: *closure,
                    environment,
                }
            }
            ExpressionKind::Place(place) | ExpressionKind::Move(Referent::Place(place)) => {
                let source = self.place(point, Referent::Place(*place), state)?;
                ValueSource::Adapt { source }
            }
            // Checking a tag retains this complete carrier, not a fresh or selected payload.
            ExpressionKind::GuardView { source, .. } => ValueSource::Adapt {
                source: self.place(point, *source, state)?,
            },
            ExpressionKind::PrimaryCondition { source } => {
                self.place(point, *source, state)?;
                ValueSource::Opaque
            }
            ExpressionKind::Boundary {
                source,
                action: crate::value_access::BoundaryAction::TakeValue,
            } if self.query.result_evidence.as_ref().is_some_and(|evidence| {
                evidence.transports_expression(self.query, *source, id)
            }) =>
            {
                ValueSource::Transport {
                    source: self.value(point, *source, state)?,
                }
            }
            ExpressionKind::Boundary { source, .. }
            | ExpressionKind::AdaptFunction { source }
            | ExpressionKind::Move(Referent::Evaluated(source))
            | ExpressionKind::RequirePrimary { source } => ValueSource::Adapt {
                source: self.value(point, *source, state)?,
            },
            ExpressionKind::Select { source, fields, .. } => {
                let source = self.value(point, *source, state)?;
                let path = crate::value_access::paths::known_by(fields, |_| ());
                ValueSource::Select { source, path }
            }
            ExpressionKind::Index { base, index, .. } => ValueSource::Index {
                source: self.value(point, *base, state)?,
                index: self.value(point, *index, state)?,
            },
            ExpressionKind::Aggregate { fields, .. } => {
                let fields = fields
                    .iter()
                    .map(|field| Ok((field.field, self.value(point, field.value, state)?)))
                    .collect::<Result<Box<[_]>>>()?;
                ValueSource::Aggregate(fields)
            }
            ExpressionKind::Repeat { seed } => ValueSource::Repeat {
                seed: self.value(point, *seed, state)?,
            },
            // Forward operations own control-result transport; these expressions cannot fabricate a
            // new source value before the selected producing path actually forwards it.
            ExpressionKind::If { .. }
            | ExpressionKind::EnumMatch { .. }
            | ExpressionKind::Block { .. }
            | ExpressionKind::Scoped { .. }
            | ExpressionKind::Call { .. } => {
                return Ok(self.read(point, Slot::Expression(id), state));
            }
            _ => ValueSource::Opaque,
        };
        Ok(self.push(self.data(id)?, Some(id), value))
    }
}
