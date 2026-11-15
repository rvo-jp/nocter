//! Request-local, budgeted fixed point for concrete invocation metadata, never executable admission.
use super::{Evidence, ProducerRelation, ProducerTerm};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::{
    CodeInstanceKey, ConcreteWitness, DataInstanceKey, FunctionWitness, Program,
};
use nocter_model::{
    AccessLocalId, TypeId, ValueApplication, ValueCode, ValueFunctionOrigin as Origin,
    ValueInvocation, ValuePath,
};
use std::collections::{BTreeMap, BTreeSet, btree_map::Entry as MapEntry};
mod creations;
#[cfg(test)]
mod tests;
mod values;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstantiationFailure {
    WrongOwner,
    MissingCall,
    Unresolved,
    WorkLimit,
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Key {
    Witness(CodeInstanceKey, FunctionWitness),
    Result(CodeInstanceKey, ValuePath<()>),
    DataResult(CodeInstanceKey, ValuePath<()>, TypeId),
    Call(CodeInstanceKey, ValueApplication),
    Creation(CodeInstanceKey, nocter_model::ValueClosure),
    Value(CodeInstanceKey, super::ValueSourceId, ValuePath<()>),
    Data(CodeInstanceKey, super::ValueSourceId, ValuePath<()>, TypeId),
    Representation(CodeInstanceKey, super::ValueSourceId, ValuePath<()>, TypeId),
    Stored(CodeInstanceKey, super::ValueSourceSelection, TypeId),
    CompletedInput(CodeInstanceKey, AccessLocalId, ValuePath<()>, TypeId),
}
#[derive(Clone, Debug, Eq, PartialEq, Default)]
enum Fact {
    #[default]
    Empty,
    Witness(ConcreteWitness),
    Frame(CodeInstanceKey),
    Data(DataInstanceKey),
    Unavailable,
}
impl Fact {
    fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Empty, fact) | (fact, Self::Empty) => fact,
            (left, right) if left == right => left,
            _ => Self::Unavailable,
        }
    }
}
#[derive(Default)]
struct Entry {
    fact: Fact,
    dependents: BTreeSet<Key>,
}
struct Query<'a, 'work> {
    program: &'a Program,
    evidence: &'a Evidence,
    work: &'work mut WorkBudget,
    entries: BTreeMap<Key, Entry>,
    pending: BTreeSet<Key>,
}

pub(in crate::value_access) fn call(
    program: &Program,
    evidence: &Evidence,
    caller: &CodeInstanceKey,
    site: ValueApplication,
    work: &mut WorkBudget,
) -> Result<CodeInstanceKey, InstantiationFailure> {
    if caller.code() != site.owner {
        return Err(InstantiationFailure::WrongOwner);
    }
    if evidence.invocation(site).is_none() {
        return Err(InstantiationFailure::MissingCall);
    }
    match Query::new(program, evidence, work).solve(&Key::Call(caller.clone(), site))? {
        Fact::Frame(frame) => Ok(frame),
        _ => Err(InstantiationFailure::Unresolved),
    }
}
pub(in crate::value_access) fn value(
    program: &Program,
    evidence: &Evidence,
    caller: &CodeInstanceKey,
    value: super::ValueSourceId,
    work: &mut WorkBudget,
) -> Result<ConcreteWitness, InstantiationFailure> {
    if evidence
        .value_sources(caller.code())
        .and_then(|graph| graph.node(value))
        .is_none()
    {
        return Err(InstantiationFailure::Unresolved);
    }
    match Query::new(program, evidence, work).solve(&Key::Value(
        caller.clone(),
        value,
        ValuePath::default(),
    ))? {
        Fact::Witness(witness) => Ok(witness),
        _ => Err(InstantiationFailure::Unresolved),
    }
}

pub(in crate::value_access) fn data(
    program: &Program,
    evidence: &Evidence,
    caller: &CodeInstanceKey,
    value: super::ValueSourceId,
    source: TypeId,
    work: &mut WorkBudget,
) -> Result<DataInstanceKey, InstantiationFailure> {
    if evidence
        .value_sources(caller.code())
        .and_then(|graph| graph.node(value))
        .is_none()
    {
        return Err(InstantiationFailure::Unresolved);
    }
    match Query::new(program, evidence, work).solve(&Key::Data(
        caller.clone(),
        value,
        ValuePath::default(),
        source,
    ))? {
        Fact::Data(data) => Ok(data),
        _ => Err(InstantiationFailure::Unresolved),
    }
}

pub(in crate::value_access) fn completed_input(
    program: &Program,
    evidence: &Evidence,
    frame: &CodeInstanceKey,
    local: AccessLocalId,
    source: TypeId,
    work: &mut WorkBudget,
) -> Result<DataInstanceKey, InstantiationFailure> {
    match Query::new(program, evidence, work).solve(&Key::CompletedInput(
        frame.clone(),
        local,
        ValuePath::default(),
        source,
    ))? {
        Fact::Data(data) => Ok(data),
        _ => Err(InstantiationFailure::Unresolved),
    }
}
pub(in crate::value_access) fn stored(
    program: &Program,
    evidence: &Evidence,
    caller: &CodeInstanceKey,
    selected: &super::ValueSourceSelection,
    source: TypeId,
    work: &mut WorkBudget,
) -> Result<DataInstanceKey, InstantiationFailure> {
    match Query::new(program, evidence, work).solve(&Key::Stored(
        caller.clone(),
        selected.clone(),
        source,
    ))? {
        Fact::Data(data) => Ok(data),
        _ => Err(InstantiationFailure::Unresolved),
    }
}

pub(in crate::value_access) fn result(
    program: &Program,
    evidence: &Evidence,
    frame: &CodeInstanceKey,
    source: TypeId,
    work: &mut WorkBudget,
) -> Result<DataInstanceKey, InstantiationFailure> {
    match Query::new(program, evidence, work).solve(&Key::DataResult(
        frame.clone(),
        ValuePath::default(),
        source,
    ))? {
        Fact::Data(data) => Ok(data),
        _ => Err(InstantiationFailure::Unresolved),
    }
}

impl<'a, 'work> Query<'a, 'work> {
    fn new(program: &'a Program, evidence: &'a Evidence, work: &'work mut WorkBudget) -> Self {
        Self {
            program,
            evidence,
            work,
            entries: BTreeMap::new(),
            pending: BTreeSet::new(),
        }
    }
    fn solve(mut self, root: &Key) -> Result<Fact, InstantiationFailure> {
        self.ensure(root.clone())?;
        while let Some(key) = self.pending.pop_first() {
            self.step()?;
            let fact = self.evaluate(&key)?;
            let entry = self
                .entries
                .get_mut(&key)
                .ok_or(InstantiationFailure::Unresolved)?;
            let joined = entry.fact.clone().join(fact);
            if joined != entry.fact {
                entry.fact = joined;
                self.pending.extend(entry.dependents.iter().cloned());
            }
        }
        self.entries
            .remove(root)
            .map(|entry| entry.fact)
            .ok_or(InstantiationFailure::Unresolved)
    }

    fn step(&mut self) -> Result<(), InstantiationFailure> {
        self.work
            .step()
            .map_err(|_| InstantiationFailure::WorkLimit)
    }
    fn ensure(&mut self, key: Key) -> Result<&mut Entry, InstantiationFailure> {
        match self.entries.entry(key) {
            MapEntry::Occupied(entry) => Ok(entry.into_mut()),
            MapEntry::Vacant(entry) => {
                self.work
                    .step()
                    .map_err(|_| InstantiationFailure::WorkLimit)?;
                self.pending.insert(entry.key().clone());
                Ok(entry.insert(Entry::default()))
            }
        }
    }
    fn read(&mut self, owner: &Key, dependency: Key) -> Result<Fact, InstantiationFailure> {
        self.step()?;
        let entry = self.ensure(dependency)?;
        entry.dependents.insert(owner.clone());
        Ok(entry.fact.clone())
    }
    fn union(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        terms: &BTreeSet<FunctionWitness>,
    ) -> Result<Fact, InstantiationFailure> {
        let mut fact = Fact::Empty;
        for &selected in terms {
            fact = fact.join(self.read(owner, Key::Witness(frame.clone(), selected))?);
        }
        Ok(fact)
    }
    fn adapt(&self, fact: Fact, view: ValueInvocation) -> Fact {
        match fact {
            Fact::Witness(witness) => self
                .program
                .adapt_witness(&witness, view)
                .map_or(Fact::Unavailable, Fact::Witness),
            Fact::Empty | Fact::Unavailable => fact,
            Fact::Frame(_) | Fact::Data(_) => Fact::Unavailable,
        }
    }
    fn evaluate(&mut self, key: &Key) -> Result<Fact, InstantiationFailure> {
        match key {
            Key::Witness(frame, selected) => self.witness(key, frame, *selected),
            Key::Result(frame, path) if self.evidence.value_sources(frame.code()).is_some() => {
                self.source_result(key, frame, path)
            }
            Key::Result(frame, path) if path.is_root() => {
                match self
                    .evidence
                    .recipe(ProducerTerm::Result {
                        code: frame.code(),
                        ordinal: 0,
                    })
                    .cloned()
                {
                    Some(ProducerRelation::Union(terms)) => self.union(key, frame, &terms),
                    _ => Ok(Fact::Unavailable),
                }
            }
            Key::Result(_, _) => Ok(Fact::Unavailable),
            Key::DataResult(frame, path, target) => {
                self.source_data_result(key, frame, path, *target)
            }
            Key::Call(frame, site) => self.invocation(key, frame, *site),
            Key::Creation(frame, closure) => self.creation(key, frame, *closure),
            Key::Value(frame, value, path) => self.source_value(key, frame, *value, path),
            Key::Data(frame, value, path, target) => {
                self.source_data(key, frame, *value, path, *target)
            }
            Key::Representation(frame, value, path, target) => {
                self.source_representation(key, frame, *value, path, *target)
            }
            Key::Stored(frame, selected, target) => {
                self.source_stored_data(key, frame, selected, *target)
            }
            Key::CompletedInput(frame, local, path, target) => {
                self.source_completed_input(key, frame, *local, path, *target)
            }
        }
    }
    fn witness(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        selected: FunctionWitness,
    ) -> Result<Fact, InstantiationFailure> {
        if let Origin::Closure(closure) = selected.origin {
            if let Ok(witness) = self.program.concrete_witness(selected) {
                return Ok(Fact::Witness(witness));
            }
            let created = self.read(owner, Key::Creation(frame.clone(), closure))?;
            return Ok(self.adapt(created, selected.invocation));
        }
        if !matches!(selected.origin, Origin::Result(_) | Origin::Application(_)) {
            return Ok(frame
                .resolve(self.program, selected)
                .map_or(Fact::Unavailable, Fact::Witness));
        }
        if self.evidence.has_no_callable_value(selected.origin) {
            return Ok(Fact::Empty);
        }
        // A matched source summary plus a context-free concrete-witness proof needs no creation
        // frame. This never reconstructs a dependent environment from a signature or code name.
        if let Ok(summary) = self.evidence.witness(selected)
            && let Ok(witness) = self.program.concrete_witness(summary)
        {
            return Ok(Fact::Witness(witness));
        }
        let fact = match selected.origin {
            Origin::Application(result) if frame.code() == result.application.owner => {
                match self.read(owner, Key::Call(frame.clone(), result.application))? {
                    Fact::Frame(callee) => self.result_leaf(owner, &callee, result.ordinal)?,
                    Fact::Empty => Fact::Empty,
                    _ => Fact::Unavailable,
                }
            }
            Origin::Result(result) if frame.code() == ValueCode::Declaration(result.owner) => {
                self.result_leaf(owner, frame, result.ordinal)?
            }
            _ => Fact::Unavailable,
        };
        Ok(self.adapt(fact, selected.invocation))
    }
    fn result_leaf(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        ordinal: usize,
    ) -> Result<Fact, InstantiationFailure> {
        let path = match frame.code() {
            ValueCode::Declaration(code) => {
                self.program
                    .callable_result_path(nocter_model::ValueCallableResult {
                        owner: code,
                        ordinal,
                    })
            }
            ValueCode::Closure(_) if ordinal == 0 => Some(ValuePath::default()),
            ValueCode::Closure(_) | ValueCode::Expression(_) => None,
        };
        let Some(path) = path else {
            return Ok(Fact::Unavailable);
        };
        self.read(owner, Key::Result(frame.clone(), path))
    }
    fn invocation(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        site: ValueApplication,
    ) -> Result<Fact, InstantiationFailure> {
        if frame.code() != site.owner {
            return Ok(Fact::Unavailable);
        }
        if self.evidence.value_sources(frame.code()).is_some() {
            return self.source_invocation(owner, frame, site);
        }
        let Some(recipe) = self.evidence.invocations.get(&site).cloned() else {
            return Ok(Fact::Unavailable);
        };
        let selected = self.union(owner, frame, &recipe.callees)?;
        let mut bindings = Vec::new();
        let mut pending = matches!(selected, Fact::Empty);
        let mut unavailable =
            matches!(selected, Fact::Unavailable | Fact::Frame(_) | Fact::Data(_));
        for (&parameter, terms) in &recipe.inputs {
            match self.union(owner, frame, terms)? {
                Fact::Witness(witness) => bindings.push((parameter, witness)),
                Fact::Empty => pending = true,
                _ => unavailable = true,
            }
        }
        if unavailable {
            return Ok(Fact::Unavailable);
        }
        if pending {
            return Ok(Fact::Empty);
        }
        let Fact::Witness(selected) = selected else {
            return Ok(Fact::Unavailable);
        };
        Ok(self
            .program
            .bind_instance_with_inputs(&selected, bindings, [])
            .map_or(Fact::Unavailable, Fact::Frame))
    }
}
