//! Original call writes select completed receiver environments or explicit body inputs.
use super::{
    CodeInstanceKey, Fact, Key, LocalOrigin, Query, ReplacementPath, Result, ValuePath,
    ValueSource, ValueSourceNode, replacement_path,
};
use crate::value_access::{CallKind, ValueSourceSlot};
use nocter_model::{
    AccessExpressionId, AccessLocalId, TypeId, ValueApplication, ValueCode, ValueInputOperation,
};

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
        let tail = match replacement_path(written, selected) {
            ReplacementPath::Unchanged => {
                return self.read(owner, Key::Value(frame.clone(), *source, selected.clone()));
            }
            ReplacementPath::Supplied(tail) => tail,
            ReplacementPath::Components => return Ok(Fact::Unavailable),
        };
        let receiver = *argument == 0
            && self
                .evidence
                .value_sources(frame.code())
                .and_then(|graph| graph.call(*call))
                .is_some_and(|bundles| {
                    !bundles.is_empty()
                        && bundles.iter().all(|bundle| bundle.kind == CallKind::Value)
                });
        let before = self.read(owner, Key::Value(frame.clone(), *source, selected.clone()))?;
        let after = if receiver {
            if !tail.is_root() {
                return Ok(Fact::Unavailable);
            }
            self.completed_receiver(owner, frame, *call, before.clone())?
        } else {
            let data = self.completed_explicit_input(owner, frame, *call, *argument, &tail)?;
            self.callable_fact(data)
        };
        Ok(if *weak { before.join(after) } else { after })
    }
    pub(super) fn source_call_write_data(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        kind: &ValueSource,
        selected: &ValuePath<()>,
        target: TypeId,
    ) -> Result {
        let ValueSource::CallWrite {
            call,
            argument,
            source,
            path,
            weak,
        } = kind
        else {
            return Ok(Fact::Unavailable);
        };
        let ReplacementPath::Supplied(tail) = replacement_path(path, selected) else {
            return Ok(Fact::Unavailable);
        };
        let after = match self.completed_explicit_input(owner, frame, *call, *argument, &tail)? {
            Fact::Data(data) if data.data() == target => Fact::Data(data),
            Fact::Data(data)
                if self.program.matches_compound_input(data.data(), target)
                    || self.program.matches_compound_input(target, data.data()) =>
            {
                // Keep the completed input's exact contexts and empty alternatives. The canonical
                // reprojection changes only its descriptor; no write or positive witness is added.
                data.reproject(self.program, target)
                    .map_or(Fact::Unavailable, Fact::Data)
            }
            Fact::Empty => Fact::Empty,
            _ => Fact::Unavailable,
        };
        Ok(if *weak {
            after.join(self.read(
                owner,
                Key::Data(frame.clone(), *source, selected.clone(), target),
            )?)
        } else {
            after
        })
    }
    fn callable_fact(&self, fact: Fact) -> Fact {
        match fact {
            Fact::Data(data) => match data.schema(self.program).ok() {
                Some(nocter_declarations::value_access::InstanceSchema::Function(value)) => {
                    Fact::Witness(value)
                }
                _ => Fact::Unavailable,
            },
            Fact::Empty => Fact::Empty,
            _ => Fact::Unavailable,
        }
    }
    fn completed_explicit_input(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        call: AccessExpressionId,
        argument: usize,
        path: &ValuePath<()>,
    ) -> Result {
        let Some(bundles) = self
            .evidence
            .value_sources(frame.code())
            .and_then(|graph| graph.call(call))
        else {
            return Ok(Fact::Unavailable);
        };
        let Some(first) = bundles.first() else {
            return Ok(Fact::Unavailable);
        };
        if bundles.iter().any(|bundle| bundle.kind != first.kind) {
            return Ok(Fact::Unavailable);
        }
        let Some(explicit) = argument.checked_sub(usize::from(first.kind == CallKind::Value))
        else {
            return Ok(Fact::Unavailable);
        };
        let selected = match self.read(
            owner,
            Key::Call(
                frame.clone(),
                ValueApplication {
                    owner: frame.code(),
                    call,
                },
            ),
        )? {
            Fact::Frame(selected) => selected,
            Fact::Empty => return Ok(Fact::Empty),
            _ => return Ok(Fact::Unavailable),
        };
        let Some(graph) = self.evidence.value_sources(selected.code()) else {
            return Ok(Fact::Unavailable);
        };
        let Some(channel) = graph
            .explicit_channels()
            .and_then(|channels| channels.get(explicit))
        else {
            return Ok(Fact::Unavailable);
        };
        self.work
            .step()
            .map_err(|_| super::InstantiationFailure::WorkLimit)?;
        if channel.operation != ValueInputOperation::Edit {
            return Ok(Fact::Unavailable);
        }
        let Some(node) = graph.node(channel.source) else {
            return Ok(Fact::Unavailable);
        };
        let ValueSource::Input { local, origin } = node.value else {
            return Ok(Fact::Unavailable);
        };
        if !origin.is_input() || origin == LocalOrigin::Capture {
            return Ok(Fact::Unavailable);
        }
        let Some(data) = node.data else {
            return Ok(Fact::Unavailable);
        };
        let Some(data) = self
            .program
            .project_data_by(data, path, |(), length| length > 0)
        else {
            return Ok(Fact::Unavailable);
        };
        self.read(
            owner,
            Key::CompletedInput(selected, local, path.clone(), data),
        )
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
