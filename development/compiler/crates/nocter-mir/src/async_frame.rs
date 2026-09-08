use std::collections::{BTreeMap, BTreeSet};

use nocter_model::{MirBlockId, MirDropFlagId, MirLocalId, MirPlaceId, MirValueId};

use crate::validation_graph::{place_values, successors};
use crate::{
    MirAggregate, MirBody, MirCallAllocation, MirOperation, MirOperationKind, MirPlaceRoot,
    MirTerminator,
};

/// One ordered cancellation action selected by checked ownership and specialized for MIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirCancellationAction {
    ReleaseAwaited(MirValueId),
    Destroy {
        place: MirPlaceId,
        initialized: Option<MirDropFlagId>,
        plan: crate::MirDestructionPlan,
    },
    ReleaseRegion(MirLocalId),
    DestroyPack,
}

/// One exact body resource retained while a deferred function is suspended.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MirFrameField {
    Pack,
    Local(MirLocalId),
    Value(MirValueId),
    DropFlag(MirDropFlagId),
}

/// Inputs owned by a deferred computation before its first resume.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirInitialAsyncState {
    fields: Box<[MirFrameField]>,
    cancellation: Box<[MirCancellationAction]>,
}

impl MirInitialAsyncState {
    #[must_use]
    pub const fn fields(&self) -> &[MirFrameField] {
        &self.fields
    }

    #[must_use]
    pub const fn cancellation(&self) -> &[MirCancellationAction] {
        &self.cancellation
    }
}

/// One closed suspension state and the exact resources needed by its continuation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSuspensionState {
    suspend: MirBlockId,
    resume: MirBlockId,
    awaited: MirValueId,
    fields: Box<[MirFrameField]>,
    cancellation: Box<[MirCancellationAction]>,
}

impl MirSuspensionState {
    #[must_use]
    pub const fn suspend(&self) -> MirBlockId {
        self.suspend
    }

    #[must_use]
    pub const fn resume(&self) -> MirBlockId {
        self.resume
    }

    #[must_use]
    pub const fn awaited(&self) -> MirValueId {
        self.awaited
    }

    #[must_use]
    pub const fn fields(&self) -> &[MirFrameField] {
        &self.fields
    }

    #[must_use]
    pub const fn cancellation(&self) -> &[MirCancellationAction] {
        &self.cancellation
    }
}

/// Canonical target-independent frame layout for one deferred MIR body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirAsyncFrame {
    initial: MirInitialAsyncState,
    states: Box<[MirSuspensionState]>,
    completed_destruction: Option<crate::MirDestructionPlan>,
}

impl MirAsyncFrame {
    #[must_use]
    pub const fn initial(&self) -> &MirInitialAsyncState {
        &self.initial
    }

    #[must_use]
    pub const fn states(&self) -> &[MirSuspensionState] {
        &self.states
    }

    #[must_use]
    pub const fn completed_destruction(&self) -> Option<&crate::MirDestructionPlan> {
        self.completed_destruction.as_ref()
    }

    pub(crate) fn derive(
        body: &MirBody,
        initial_cancellation: Box<[MirCancellationAction]>,
        mut cancellation: BTreeMap<MirBlockId, Box<[MirCancellationAction]>>,
        completed_destruction: Option<crate::MirDestructionPlan>,
    ) -> Result<Self, crate::MirBodyBuildError> {
        let liveness = Liveness::analyze(body);
        let mut states = Vec::new();
        for (suspend, block) in body.blocks().iter() {
            let MirTerminator::Suspend {
                computation,
                resume,
            } = block.terminator()
            else {
                continue;
            };
            let mut fields = liveness.before[&resume.block()].clone();
            fields.insert(MirFrameField::Value(*computation));
            let actions = cancellation.remove(&suspend).ok_or(
                crate::MirBodyBuildError::MissingSuspensionCancellation(suspend),
            )?;
            for action in &actions {
                match action {
                    MirCancellationAction::ReleaseAwaited(value) => {
                        fields.insert(MirFrameField::Value(*value));
                    }
                    MirCancellationAction::Destroy {
                        place, initialized, ..
                    } => {
                        let place = body
                            .places()
                            .get(*place)
                            .expect("cancellation plan owns a valid MIR place");
                        if let MirPlaceRoot::Local(local) = place.root() {
                            fields.insert(MirFrameField::Local(local));
                        }
                        fields.extend(place_values(place).map(MirFrameField::Value));
                        fields.extend(initialized.map(MirFrameField::DropFlag));
                    }
                    MirCancellationAction::ReleaseRegion(local) => {
                        fields.insert(MirFrameField::Local(*local));
                    }
                    MirCancellationAction::DestroyPack => {
                        fields.insert(MirFrameField::Pack);
                    }
                }
            }
            states.push(MirSuspensionState {
                suspend,
                resume: resume.block(),
                awaited: *computation,
                fields: fields.into_iter().collect(),
                cancellation: actions,
            });
        }
        if let Some(unexpected) = cancellation.keys().next().copied() {
            return Err(crate::MirBodyBuildError::UnexpectedSuspensionCancellation(
                unexpected,
            ));
        }
        let mut initial_fields = body
            .parameters()
            .iter()
            .copied()
            .map(MirFrameField::Local)
            .collect::<BTreeSet<_>>();
        if body.pack().is_some() {
            initial_fields.insert(MirFrameField::Pack);
        }
        Ok(Self {
            initial: MirInitialAsyncState {
                fields: initial_fields.into_iter().collect(),
                cancellation: initial_cancellation,
            },
            states: states.into_boxed_slice(),
            completed_destruction,
        })
    }
}

struct Liveness {
    before: BTreeMap<MirBlockId, BTreeSet<MirFrameField>>,
}

impl Liveness {
    fn analyze(body: &MirBody) -> Self {
        let facts = body
            .blocks()
            .iter()
            .map(|(block, value)| (block, BlockFacts::collect(body, value)))
            .collect::<BTreeMap<_, _>>();
        let mut before = body
            .blocks()
            .iter()
            .map(|(block, _)| (block, BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        loop {
            let mut changed = false;
            let blocks = body.blocks().iter().collect::<Vec<_>>();
            for (block, value) in blocks.into_iter().rev() {
                let mut after = BTreeSet::new();
                for successor in successors(value.terminator()) {
                    after.extend(before[&successor.block()].iter().copied());
                }
                let facts = &facts[&block];
                after.retain(|field| !facts.definitions.contains(field));
                after.extend(facts.uses.iter().copied());
                if before[&block] != after {
                    before.insert(block, after);
                    changed = true;
                }
            }
            if !changed {
                return Self { before };
            }
        }
    }
}

struct BlockFacts {
    uses: BTreeSet<MirFrameField>,
    definitions: BTreeSet<MirFrameField>,
}

impl BlockFacts {
    fn collect(body: &MirBody, block: &crate::MirBlock) -> Self {
        let mut facts = Self {
            uses: BTreeSet::new(),
            definitions: block
                .parameters()
                .iter()
                .copied()
                .map(MirFrameField::Value)
                .collect(),
        };
        for operation in block.operations() {
            let operation = body
                .operations()
                .get(*operation)
                .expect("MIR construction owns valid operation identities");
            facts.record_operation(body, operation);
        }
        facts.record_terminator(body, block.terminator());
        facts
    }

    fn use_field(&mut self, field: MirFrameField) {
        if !self.definitions.contains(&field) {
            self.uses.insert(field);
        }
    }

    fn use_value(&mut self, value: MirValueId) {
        self.use_field(MirFrameField::Value(value));
    }

    fn use_place(&mut self, body: &MirBody, place: MirPlaceId) {
        let place = body
            .places()
            .get(place)
            .expect("MIR construction owns valid place identities");
        if let MirPlaceRoot::Local(local) = place.root() {
            self.use_field(MirFrameField::Local(local));
        }
        for value in place_values(place) {
            self.use_value(value);
        }
    }

    fn use_branch(&mut self, target: &crate::MirBranchTarget) {
        for argument in target.arguments() {
            self.use_value(*argument);
        }
    }

    fn record_operation(&mut self, body: &MirBody, operation: &MirOperation) {
        match operation.kind() {
            MirOperationKind::Constant(_)
            | MirOperationKind::PackLength
            | MirOperationKind::PackNext
            | MirOperationKind::DestroyPack => {}
            MirOperationKind::Read { place, .. }
            | MirOperationKind::Borrow { place, .. }
            | MirOperationKind::InvokeDrop { place, .. }
            | MirOperationKind::ReportError { place }
            | MirOperationKind::ReleaseError { place }
            | MirOperationKind::ReleaseComputation { place } => self.use_place(body, *place),
            MirOperationKind::Store { destination, value }
            | MirOperationKind::Initialize { destination, value } => {
                self.use_place(body, *destination);
                self.use_value(*value);
            }
            MirOperationKind::SetDropFlag { flag, .. } => {
                self.definitions.insert(MirFrameField::DropFlag(*flag));
            }
            MirOperationKind::Unary { operand, .. }
            | MirOperationKind::NumericConversion { operand } => self.use_value(*operand),
            MirOperationKind::Binary { left, right, .. } => {
                self.use_value(*left);
                self.use_value(*right);
            }
            MirOperationKind::Aggregate(aggregate) => match aggregate {
                MirAggregate::Struct { fields, .. } => {
                    for (_, value) in fields {
                        self.use_value(*value);
                    }
                }
                MirAggregate::Enum { payload, .. }
                | MirAggregate::FixedArray(payload)
                | MirAggregate::Tuple(payload) => {
                    for value in payload {
                        self.use_value(*value);
                    }
                }
                MirAggregate::Optional(value) | MirAggregate::FallibleSuccess(value) => {
                    if let Some(value) = value {
                        self.use_value(*value);
                    }
                }
                MirAggregate::FallibleFailure(value) | MirAggregate::Opaque { witness: value } => {
                    self.use_value(*value);
                }
                MirAggregate::Closure { captures, .. } => {
                    for capture in captures {
                        self.use_value(capture.value());
                    }
                }
            },
            MirOperationKind::Call(call) => {
                for argument in call.arguments() {
                    self.use_value(*argument);
                }
                if let MirCallAllocation::Explicit(place) = call.allocation() {
                    self.use_place(body, place);
                }
                if let Some(pack) = call.pack().and_then(crate::MirCallPack::prepared) {
                    self.use_value(pack.length());
                    for segment in pack.segments() {
                        match segment {
                            crate::MirPackSegment::Value { value, .. } => self.use_value(*value),
                            crate::MirPackSegment::KeyedValue { key, value, .. } => {
                                self.use_value(*key);
                                self.use_value(*value);
                            }
                            crate::MirPackSegment::Spread(spread) => {
                                self.use_value(spread.remaining());
                                self.use_value(spread.receiver());
                                self.use_place(body, spread.iterator());
                            }
                        }
                    }
                }
            }
            MirOperationKind::CreateRegion { parent, region } => {
                self.use_value(*parent);
                self.use_field(MirFrameField::Local(*region));
            }
            MirOperationKind::ReleaseRegion { region } => {
                self.use_field(MirFrameField::Local(*region));
            }
        }
        if let Some(result) = operation.result() {
            self.definitions.insert(MirFrameField::Value(result));
        }
    }

    fn record_terminator(&mut self, body: &MirBody, terminator: &MirTerminator) {
        match terminator {
            MirTerminator::Goto(target) => self.use_branch(target),
            MirTerminator::Branch {
                condition,
                then_target,
                else_target,
            } => {
                self.use_value(*condition);
                self.use_branch(then_target);
                self.use_branch(else_target);
            }
            MirTerminator::BranchDropFlag {
                flag,
                initialized,
                uninitialized,
            } => {
                self.use_field(MirFrameField::DropFlag(*flag));
                self.use_branch(initialized);
                self.use_branch(uninitialized);
            }
            MirTerminator::Switch {
                subject,
                cases,
                fallback,
            } => {
                match subject {
                    crate::MirSwitchSubject::Value(value) => self.use_value(*value),
                    crate::MirSwitchSubject::Place(place) => self.use_place(body, *place),
                }
                for case in cases {
                    self.use_branch(case.target());
                }
                self.use_branch(fallback);
            }
            MirTerminator::Suspend { computation, .. } => self.use_value(*computation),
            MirTerminator::Return(value) | MirTerminator::Exit(value) => {
                if let Some(value) = value {
                    self.use_value(*value);
                }
            }
            MirTerminator::Trap | MirTerminator::Unreachable => {}
        }
    }
}
