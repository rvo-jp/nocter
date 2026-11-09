//! Private activation requirements and storage effects, not complete executable MIR bodies.
mod lower;

use std::collections::BTreeMap;

use nocter_model::{
    AccessExpressionId, AccessFlowPointId, AccessInstanceId, AccessLocalId, Arena, MirLocalId,
    TypeId,
};
use nocter_runtime_contract::{RuntimeAbiIdentity, RuntimeEnvironment, RuntimeValueSignature};
use nocter_target_program::value_access::{lifecycle, storage};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SlotKey {
    Local(AccessLocalId),
    Expression(AccessExpressionId),
    Prepared {
        call: AccessExpressionId,
        index: usize,
    },
    Temporary {
        call: AccessExpressionId,
        index: usize,
    },
    Argument {
        call: AccessExpressionId,
        index: usize,
    },
    Operand {
        expression: AccessExpressionId,
        index: usize,
    },
    Predicate(AccessExpressionId),
    Answer(AccessExpressionId),
    Return,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Storage {
    Payload,
    Address,
}

#[derive(Clone, Debug)]
pub struct Slot {
    pub key: SlotKey,
    pub storage: Storage,
    pub representations: Box<[TypeId]>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Projection {
    Field(nocter_model::AccessFieldId),
    Environment,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Place {
    pub slot: MirLocalId,
    pub root: TypeId,
    pub projections: Box<[Projection]>,
    pub data: TypeId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Disposal {
    Trivial,
    Error,
    Record,
    Capture,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    /// The selected value computation initializes these bytes; this is not another constructor.
    Initialize(Place),
    Transfer {
        source: Place,
        destination: Place,
    },
    Reinterpret {
        source: Place,
        destination: Place,
    },
    Dispose {
        source: Place,
        disposal: Disposal,
    },
    End(Place),
    BindAddress {
        destination: MirLocalId,
        source: Place,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BitValue {
    pub word: usize,
    pub mask: u64,
    pub value: bool,
}

#[derive(Clone, Debug)]
pub struct Alternative {
    pub tests: Box<[BitValue]>,
    pub before: Box<[Effect]>,
    pub before_writes: Box<[BitValue]>,
    pub after: Box<[Effect]>,
    pub after_writes: Box<[BitValue]>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Location {
    pub point: AccessFlowPointId,
    pub action: usize,
}

#[derive(Clone, Debug)]
pub struct Action {
    /// Run after before-effects/writes and before entering the selected call.
    pub call_bindings: Box<[Effect]>,
    pub alternatives: Box<[Alternative]>,
}

#[derive(Clone, Debug)]
pub struct Body {
    signature: RuntimeValueSignature,
    slots: Arena<MirLocalId, Slot>,
    flags: Box<[u64]>,
    actions: BTreeMap<Location, Action>,
}
impl Body {
    #[must_use]
    pub const fn signature(&self) -> &RuntimeValueSignature {
        &self.signature
    }
    #[must_use]
    pub const fn slots(&self) -> &Arena<MirLocalId, Slot> {
        &self.slots
    }
    #[must_use]
    pub fn flags(&self) -> &[u64] {
        &self.flags
    }
    #[must_use]
    pub const fn actions(&self) -> &BTreeMap<Location, Action> {
        &self.actions
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    Storage(storage::Failure),
    InvalidBody(AccessInstanceId),
}

/// Frozen storage companion. It deliberately exposes no executable or native conversion.
#[derive(Clone, Debug)]
pub struct StorageProgram {
    runtime: RuntimeEnvironment,
    bodies: BTreeMap<AccessInstanceId, Body>,
}
impl StorageProgram {
    /// Freeze activation requirements from exact selected lifecycle effects and input channels.
    /// # Errors
    /// Rejects missing identities, inconsistent slot roles or unresolved concrete data.
    pub fn lower(program: &lifecycle::Program, abi: RuntimeAbiIdentity) -> Result<Self, Failure> {
        let runtime =
            storage::prepare(program.instance_program(), abi).map_err(Failure::Storage)?;
        let bodies = program
            .bodies()
            .iter()
            .map(|(&id, body)| Ok((id, lower::body(program.instance_program(), id, body)?)))
            .collect::<Result<_, Failure>>()?;
        Ok(Self { runtime, bodies })
    }
    #[must_use]
    pub const fn runtime(&self) -> &RuntimeEnvironment {
        &self.runtime
    }
    #[must_use]
    pub const fn bodies(&self) -> &BTreeMap<AccessInstanceId, Body> {
        &self.bodies
    }
}
