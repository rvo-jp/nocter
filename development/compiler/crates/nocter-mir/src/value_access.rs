//! Closed private control, computation and lifecycle bodies. No released or native admission.
mod computation;
mod lower;
#[cfg(test)]
mod tests;
mod validate;

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
    Field(nocter_model::ValueField),
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
    Aggregate,
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
    DisposeArrayElements {
        source: Place,
        start: u64,
        end: u64,
    },
    End(Place),
    BindAddress {
        destination: MirLocalId,
        source: Place,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Constant {
    Integer(i128),
    Boolean(bool),
    Zero,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallInput {
    pub source: Place,
    pub operation: nocter_model::ValueInputOperation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Call {
    pub target: AccessInstanceId,
    /// Frozen callee signature order, including its environment when present.
    pub inputs: Box<[CallInput]>,
    /// Filled on normal return before adapter cleanup or result receipt.
    pub answer: Place,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operation {
    /// A terminal source bounds failure; no element payload or selected address exists.
    BoundsTrap,
    Frozen {
        destination: Place,
        value: nocter_model::FrozenData,
    },
    Constant {
        destination: Place,
        value: Constant,
    },
    Copy {
        source: Place,
        destination: Place,
    },
    /// Check and capture one evaluated element address, or snapshot its selected Copy payload.
    Index {
        base: Place,
        index: Place,
        destination: Place,
        copy: bool,
        bounds: nocter_model::IndexBoundsCheck,
    },
    /// Copy the established seed into the homogeneous result; count belongs to its schema.
    Repeat {
        source: Place,
        destination: Place,
    },
    Storage(Effect),
    Unary {
        operation: crate::MirUnaryOperation,
        operand: Place,
        destination: Place,
        check: crate::MirArithmeticCheck,
    },
    Binary {
        operation: crate::MirBinaryOperation,
        left: Place,
        right: Place,
        destination: Place,
        invert: bool,
        check: crate::MirArithmeticCheck,
    },
    Call(Call),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Terminator {
    Jump(AccessFlowPointId),
    Branch {
        condition: Place,
        yes: AccessFlowPointId,
        no: AccessFlowPointId,
    },
    Return(Place),
    Unreachable,
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
    pub operations: Box<[Operation]>,
    pub alternatives: Box<[Alternative]>,
}

#[derive(Clone, Debug)]
pub struct Body {
    statics: Box<[(MirLocalId, nocter_model::ValueStatic)]>,
    signature: RuntimeValueSignature,
    slots: Arena<MirLocalId, Slot>,
    flags: Box<[u64]>,
    actions: BTreeMap<Location, Action>,
    entry: AccessFlowPointId,
    terminators: BTreeMap<AccessFlowPointId, Terminator>,
    inputs: Box<[MirLocalId]>,
}
impl Body {
    #[must_use]
    pub fn statics(&self) -> &[(MirLocalId, nocter_model::ValueStatic)] {
        &self.statics
    }
    #[must_use]
    pub const fn entry(&self) -> AccessFlowPointId {
        self.entry
    }
    #[must_use]
    pub const fn terminators(&self) -> &BTreeMap<AccessFlowPointId, Terminator> {
        &self.terminators
    }
    #[must_use]
    pub fn inputs(&self) -> &[MirLocalId] {
        &self.inputs
    }
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

/// One immutable global object with its exact closed runtime data.
#[derive(Clone, Debug)]
pub struct Static {
    pub data: TypeId,
    pub value: nocter_model::FrozenData,
}
/// Closed private MIR bodies and global data, without a released-program adapter.
#[derive(Clone, Debug)]
pub struct Program {
    statics: BTreeMap<nocter_model::ValueStatic, Static>,
    runtime: RuntimeEnvironment,
    bodies: BTreeMap<AccessInstanceId, Body>,
    root: AccessInstanceId,
}
impl Program {
    /// Freeze computation, control and lifecycle from one exact selected instance program.
    /// # Errors
    /// Rejects missing identities, inconsistent slot roles or unresolved concrete data.
    pub fn lower(program: &lifecycle::Program, abi: RuntimeAbiIdentity) -> Result<Self, Failure> {
        let runtime =
            storage::prepare(program.instance_program(), abi).map_err(Failure::Storage)?;
        let bodies: BTreeMap<AccessInstanceId, Body> = program
            .bodies()
            .iter()
            .map(|(&id, body)| {
                Ok((
                    id,
                    lower::body(program.instance_program(), &runtime, id, body)?,
                ))
            })
            .collect::<Result<_, Failure>>()?;
        let mut statics = BTreeMap::new();
        for body in bodies.values() {
            for &(slot, id) in &body.statics {
                let source = program.instance_program().unit();
                let data = *body
                    .slots
                    .get(slot)
                    .ok_or(Failure::InvalidBody(program.instance_program().root()))?
                    .representations
                    .first()
                    .ok_or(Failure::InvalidBody(program.instance_program().root()))?;
                let value = source
                    .static_value(id)
                    .ok_or(Failure::InvalidBody(program.instance_program().root()))?
                    .clone();
                if let Some(existing) = statics.insert(id, Static { data, value })
                    && existing.data != data
                {
                    return Err(Failure::InvalidBody(program.instance_program().root()));
                }
            }
        }
        let result = Self {
            statics,
            runtime,
            bodies,
            root: program.instance_program().root(),
        };
        validate::program(&result)?;
        Ok(result)
    }
    #[must_use]
    pub const fn statics(&self) -> &BTreeMap<nocter_model::ValueStatic, Static> {
        &self.statics
    }
    #[must_use]
    pub const fn runtime(&self) -> &RuntimeEnvironment {
        &self.runtime
    }
    #[must_use]
    pub const fn bodies(&self) -> &BTreeMap<AccessInstanceId, Body> {
        &self.bodies
    }
    #[must_use]
    pub const fn root(&self) -> AccessInstanceId {
        self.root
    }
}
