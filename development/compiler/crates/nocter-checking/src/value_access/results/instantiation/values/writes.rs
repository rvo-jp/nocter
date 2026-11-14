//! Value-receiver call writes retain code identity while selecting the completed actual environment.
use super::{
    CodeInstanceKey, Fact, Key, LocalOrigin, Query, ReplacementPath, Result, ValuePath,
    ValueSource, ValueSourceNode, replacement_path,
};
use crate::value_access::{CallKind, ValueSourceSlot};
use nocter_model::{AccessExpressionId, AccessLocalId, TypeId, ValueApplication, ValueCode};

impl Query<'_, '_> {
    pub(super) fn source_call_write(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        node: &ValueSourceNode,
        selected: &ValuePath<()>,
    ) -> Result {
        let ValueSource::CallWrite {
            call,
            argument,
            source,
            path: written,
            weak,
        } = &node.value
        else {
            return Ok(Fact::Unavailable);
        };
        match replacement_path(written, selected) {
            ReplacementPath::Unchanged => {
                return self.read(owner, Key::Value(frame.clone(), *source, selected.clone()));
            }
            ReplacementPath::Supplied(tail) if tail.is_root() => {}
            _ => return Ok(Fact::Unavailable),
        }
        if *argument != 0
            || !self
                .evidence
                .value_sources(frame.code())
                .and_then(|graph| graph.call(*call))
                .is_some_and(|bundles| {
                    !bundles.is_empty()
                        && bundles.iter().all(|bundle| bundle.kind == CallKind::Value)
                })
        {
            return Ok(Fact::Unavailable);
        }
        let before = self.read(owner, Key::Value(frame.clone(), *source, written.clone()))?;
        let after = self.completed_receiver(owner, frame, *call, before.clone())?;
        Ok(if *weak { before.join(after) } else { after })
    }
    fn completed_receiver(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        call: AccessExpressionId,
        before: Fact,
    ) -> Result {
        let witness = match before {
            Fact::Witness(witness) => witness,
            Fact::Empty => return Ok(Fact::Empty),
            _ => return Ok(Fact::Unavailable),
        };
        // A completely context-free witness already fixes all representation dependencies.
        // This does not mean its runtime environment is absent or unchanged as a value.
        if self.program.concrete_witness(witness.witness()).as_ref() == Ok(&witness) {
            return Ok(Fact::Witness(witness));
        }
        let callee = match self.read(
            owner,
            Key::Call(
                frame.clone(),
                ValueApplication {
                    owner: frame.code(),
                    call,
                },
            ),
        )? {
            Fact::Frame(callee) if callee.code() == witness.code() => callee,
            Fact::Empty => return Ok(Fact::Empty),
            _ => return Ok(Fact::Unavailable),
        };
        let ValueCode::Closure(closure) = callee.code() else {
            return Ok(Fact::Unavailable);
        };
        let Some((local, data)) = self.capture_input(&callee)? else {
            return Ok(Fact::Unavailable);
        };
        if self
            .program
            .closure(closure)
            .map(nocter_declarations::value_access::Closure::environment)
            != Some(data)
        {
            return Ok(Fact::Unavailable);
        }
        Ok(
            match self.read(
                owner,
                Key::CompletedInput(callee, local, ValuePath::default(), data),
            )? {
                Fact::Data(environment) => self
                    .program
                    .bind_callable_data(witness.witness(), &environment)
                    .map_or(Fact::Unavailable, Fact::Witness),
                Fact::Empty => Fact::Empty,
                _ => Fact::Unavailable,
            },
        )
    }
    fn capture_input(
        &mut self,
        frame: &CodeInstanceKey,
    ) -> std::result::Result<Option<(AccessLocalId, TypeId)>, super::InstantiationFailure> {
        let Some(graph) = self.evidence.value_sources(frame.code()) else {
            return Ok(None);
        };
        let Some(flow) = graph.flow() else {
            return Ok(None);
        };
        let mut input = None;
        for (slot, source) in flow.entry_sources() {
            self.work
                .step()
                .map_err(|_| super::InstantiationFailure::WorkLimit)?;
            let Some(node) = graph.node(source) else {
                return Ok(None);
            };
            if let ValueSource::Input {
                local,
                origin: LocalOrigin::Capture,
            } = node.value
            {
                if input.is_some() || slot != ValueSourceSlot::Local(local) {
                    return Ok(None);
                }
                let Some(data) = node.data else {
                    return Ok(None);
                };
                input = Some((local, data));
            }
        }
        Ok(input)
    }
}
