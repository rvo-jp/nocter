//! One helper per guarded source/destination layout pair; no recursive expansion of shared schemas.
use super::{Failure, mir};
use crate::identity::MachineId;
use crate::{MachineFunctionId, MachineLayoutKind, MachineLayoutPlan};
use nocter_runtime_contract::{RuntimeDataId, RuntimeType};
use std::collections::{BTreeMap, BTreeSet};

pub(super) type Pair = (RuntimeDataId, RuntimeDataId);
pub(super) struct Field {
    pub(super) pair: Pair,
    pub(super) source: u64,
    pub(super) target: u64,
}
pub(super) enum Alternative {
    Absent,
    Excluded,
    Payload(Field),
}
pub(super) enum Plan {
    Outcome {
        primary: Alternative,
        alternate: Alternative,
    },
    Aggregate(Box<[Field]>),
    Array {
        pair: Pair,
        length: u64,
        source_stride: u64,
        target_stride: u64,
    },
}
pub(super) struct Helper {
    pub(super) function: MachineFunctionId,
    pub(super) plan: Plan,
}
pub(super) struct Prepared {
    pub(super) helpers: BTreeMap<Pair, Helper>,
    pub(super) copies: BTreeSet<Pair>,
}
impl Plan {
    fn children(&self) -> Vec<Pair> {
        match self {
            Self::Outcome { primary, alternate } => [primary, alternate]
                .into_iter()
                .filter_map(|alternative| {
                    if let Alternative::Payload(field) = alternative {
                        Some(field.pair)
                    } else {
                        None
                    }
                })
                .collect(),
            Self::Aggregate(fields) => fields.iter().map(|field| field.pair).collect(),
            Self::Array { pair, .. } => vec![*pair],
        }
    }
}

pub(super) fn prepare(
    program: &mir::Program,
    layouts: &MachineLayoutPlan,
    first: usize,
) -> Result<Prepared, Failure> {
    let mut pending: Vec<_> = program
        .bodies()
        .values()
        .flat_map(|body| body.actions().values())
        .flat_map(super::effects)
        .filter_map(|effect| {
            if let mir::Effect::Transport {
                source,
                destination,
            } = effect
            {
                Some((source.data, destination.data))
            } else {
                None
            }
        })
        .collect();
    let mut seen = BTreeSet::new();
    let mut plans = BTreeMap::new();
    let mut copies = BTreeSet::new();
    let mut relations = program.runtime().value_relations();
    while let Some(pair) = pending.pop() {
        if !seen.insert(pair) {
            continue;
        }
        if !relations.permits_guarded_value_transport(pair.0, pair.1) {
            return Err(Failure::InvalidRoot);
        }
        if relations.has_same_value_representation(pair.0, pair.1) {
            copies.insert(pair);
            continue;
        }
        let plan = select(program, layouts, pair)?;
        pending.extend(plan.children());
        plans.insert(pair, plan);
    }
    let helpers = plans
        .into_iter()
        .enumerate()
        .map(|(index, (pair, plan))| {
            (
                pair,
                Helper {
                    function: MachineFunctionId::new(first + index),
                    plan,
                },
            )
        })
        .collect();
    Ok(Prepared { helpers, copies })
}

fn select(
    program: &mir::Program,
    layouts: &MachineLayoutPlan,
    pair: Pair,
) -> Result<Plan, Failure> {
    let from = layouts.get(pair.0).ok_or(Failure::InvalidRoot)?.kind();
    let to = layouts.get(pair.1).ok_or(Failure::InvalidRoot)?.kind();
    match (from, to) {
        (
            MachineLayoutKind::Outcome {
                payload_offset: source,
                ..
            },
            MachineLayoutKind::Outcome {
                payload_offset: target,
                ..
            },
        ) => {
            let alternative = |primary| {
                let source_data = program.runtime().types().outcome_payload(pair.0, primary);
                Ok::<_, Failure>(match source_data {
                    None => Alternative::Absent,
                    Some(data)
                        if matches!(
                            program.runtime().types().get(data),
                            Some(RuntimeType::Uninhabited)
                        ) =>
                    {
                        Alternative::Excluded
                    }
                    Some(data) => Alternative::Payload(Field {
                        pair: (
                            data,
                            program
                                .runtime()
                                .types()
                                .outcome_payload(pair.1, primary)
                                .ok_or(Failure::InvalidRoot)?,
                        ),
                        source: *source,
                        target: *target,
                    }),
                })
            };
            Ok(Plan::Outcome {
                primary: alternative(true)?,
                alternate: alternative(false)?,
            })
        }
        (
            MachineLayoutKind::Struct { fields: source },
            MachineLayoutKind::Struct { fields: target },
        ) if source.len() == target.len() => Ok(Plan::Aggregate(
            source
                .iter()
                .zip(target)
                .map(|(source, target)| Field {
                    pair: (source.ty(), target.ty()),
                    source: source.offset(),
                    target: target.offset(),
                })
                .collect(),
        )),
        (
            MachineLayoutKind::FixedArray {
                element: source,
                length: a,
                stride: source_stride,
            },
            MachineLayoutKind::FixedArray {
                element: target,
                length: b,
                stride: target_stride,
            },
        ) if a == b => Ok(Plan::Array {
            pair: (*source, *target),
            length: *a,
            source_stride: *source_stride,
            target_stride: *target_stride,
        }),
        _ => Err(Failure::InvalidRoot),
    }
}
