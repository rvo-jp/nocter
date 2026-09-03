use nocter_checking::{
    CleanupCondition, CleanupTarget, PatternRemainder, PlaceProjection, PlaceRoot,
};
use nocter_model::{
    BodyNodeId, FieldId, LocalBindingId, MirDropFlagId, ParameterId, PlaceId, VariantId,
};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::MirOperationKind;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum CleanupIdentity {
    Path {
        root: PlaceRoot,
        projections: Box<[CleanupIdentityProjection]>,
    },
    Value {
        node: BodyNodeId,
        remainder: ValueRemainder,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum CleanupIdentityProjection {
    Field(FieldId),
    TupleElement(usize),
}

fn cleanup_identity_projection(
    projection: &nocter_checking::CleanupProjection,
) -> CleanupIdentityProjection {
    match projection {
        nocter_checking::CleanupProjection::Field { field, .. } => {
            CleanupIdentityProjection::Field(*field)
        }
        nocter_checking::CleanupProjection::TupleElement { index, .. } => {
            CleanupIdentityProjection::TupleElement(*index)
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum ValueRemainder {
    Complete,
    Enum {
        variant: VariantId,
        payload: Box<[ParameterId]>,
    },
}

impl CleanupIdentity {
    fn from_target(target: &CleanupTarget) -> Option<Self> {
        match target {
            CleanupTarget::Path(path) => Some(Self::Path {
                root: path.root(),
                projections: path
                    .projections()
                    .iter()
                    .map(cleanup_identity_projection)
                    .collect(),
            }),
            CleanupTarget::Value { node, .. } => Some(Self::Value {
                node: *node,
                remainder: ValueRemainder::Complete,
            }),
            CleanupTarget::EnumResidual {
                subject,
                variant,
                payload,
                ..
            } => Some(Self::Value {
                node: *subject,
                remainder: ValueRemainder::Enum {
                    variant: *variant,
                    payload: payload.clone(),
                },
            }),
            CleanupTarget::Place { .. } | CleanupTarget::Region { .. } => None,
        }
    }
}

impl FunctionLowerer<'_> {
    /// Reserves every conditional storage bit before CFG construction.
    ///
    /// Flags start at function entry and are updated only by MIR storage transitions. Cleanup does
    /// not infer branch history or replay the checked ownership analysis.
    pub(super) fn prepare_cleanup_flags(&mut self) -> Result<(), MirLoweringError> {
        let conditional = self
            .item
            .body()
            .nodes()
            .iter()
            .copied()
            .flat_map(|owner| {
                self.body
                    .cleanups()
                    .schedules(owner)
                    .into_iter()
                    .flatten()
                    .flat_map(move |schedule| {
                        schedule
                            .actions()
                            .iter()
                            .filter(|action| action.condition() == CleanupCondition::IfInitialized)
                            .map(move |action| (owner, action.target().clone()))
                    })
            })
            .collect::<Vec<_>>();
        for (owner, target) in conditional {
            self.reserve_cleanup_flag(owner, &target)?;
        }
        Ok(())
    }

    fn reserve_cleanup_flag(
        &mut self,
        owner: BodyNodeId,
        target: &CleanupTarget,
    ) -> Result<(), MirLoweringError> {
        let identity = CleanupIdentity::from_target(target)
            .ok_or(MirLoweringError::UnsupportedCleanup(owner))?;
        if self.cleanup_flags.contains_key(&identity) {
            return Ok(());
        }
        let (place, initially_initialized) = match target {
            CleanupTarget::Path(path) => (
                self.lower_cleanup_path(owner, path)?,
                matches!(path.root(), PlaceRoot::Parameter(_) | PlaceRoot::Capture(_)),
            ),
            CleanupTarget::Value { node, ty } => {
                let ty = self.concrete_type(*ty)?;
                (self.reserve_value_storage(*node, ty)?, false)
            }
            CleanupTarget::EnumResidual { subject, ty, .. } => {
                let ty = self.concrete_type(*ty)?;
                (self.reserve_value_storage(*subject, ty)?, false)
            }
            CleanupTarget::Place { .. } | CleanupTarget::Region { .. } => {
                return Err(MirLoweringError::UnsupportedCleanup(owner));
            }
        };
        let flag = self.builder.add_drop_flag(place, initially_initialized);
        self.cleanup_flags.insert(identity, flag);
        Ok(())
    }

    pub(super) fn cleanup_flag(
        &self,
        owner: BodyNodeId,
        target: &CleanupTarget,
    ) -> Result<MirDropFlagId, MirLoweringError> {
        CleanupIdentity::from_target(target)
            .and_then(|identity| self.cleanup_flags.get(&identity).copied())
            .ok_or(MirLoweringError::InvalidCleanup(owner))
    }

    pub(super) fn mark_binding_initialized(
        &mut self,
        binding: LocalBindingId,
    ) -> Result<(), MirLoweringError> {
        self.set_path_flags(PlaceRoot::Local(binding), &[], true)
    }

    pub(super) fn mark_place_initialized(
        &mut self,
        place: PlaceId,
        initialized: bool,
    ) -> Result<(), MirLoweringError> {
        let checked = self
            .body
            .places()
            .get(place)
            .ok_or(MirLoweringError::UnknownPlace(place))?;
        let projections = checked
            .projections()
            .iter()
            .map(|projection| match projection {
                PlaceProjection::Field { field, .. } => {
                    Some(CleanupIdentityProjection::Field(*field))
                }
                PlaceProjection::TupleElement { index, .. } => {
                    Some(CleanupIdentityProjection::TupleElement(*index))
                }
                PlaceProjection::BorrowDeref { .. }
                | PlaceProjection::BuiltinIndex { .. }
                | PlaceProjection::CoercedBuiltinIndex { .. }
                | PlaceProjection::SelectedIndex { .. } => None,
            })
            .collect::<Option<Vec<_>>>();
        if let Some(projections) = projections {
            self.set_path_flags(checked.root(), &projections, initialized)?;
        }
        Ok(())
    }

    pub(super) fn mark_value_storage_initialized(
        &mut self,
        node: BodyNodeId,
    ) -> Result<(), MirLoweringError> {
        self.set_flag(
            &CleanupIdentity::Value {
                node,
                remainder: ValueRemainder::Complete,
            },
            true,
        )
    }

    pub(super) fn mark_value_storage_uninitialized(
        &mut self,
        node: BodyNodeId,
    ) -> Result<(), MirLoweringError> {
        self.set_flag(
            &CleanupIdentity::Value {
                node,
                remainder: ValueRemainder::Complete,
            },
            false,
        )
    }

    /// Selects the one branch-specific destruction obligation after an owned enum tag matches.
    ///
    /// The subject remains in one storage slot. Only its cleanup identity changes from the whole
    /// value to the checked remainder; separate flags prevent mutually exclusive arms from
    /// destroying each other's payloads after they join.
    pub(super) fn select_pattern_remainder(
        &mut self,
        node: BodyNodeId,
        variant: Option<VariantId>,
        remainder: &PatternRemainder,
    ) -> Result<(), MirLoweringError> {
        let transitions = self
            .cleanup_flags
            .iter()
            .filter_map(|(identity, flag)| match identity {
                CleanupIdentity::Value {
                    node: candidate, ..
                } if *candidate == node => Some((identity.clone(), *flag)),
                CleanupIdentity::Path { .. } | CleanupIdentity::Value { .. } => None,
            })
            .collect::<Vec<_>>();
        for (identity, flag) in transitions {
            let initialized = match (&identity, remainder) {
                (
                    CleanupIdentity::Value {
                        remainder: ValueRemainder::Complete,
                        ..
                    },
                    PatternRemainder::Complete,
                ) => true,
                (
                    CleanupIdentity::Value {
                        remainder:
                            ValueRemainder::Enum {
                                variant: candidate,
                                payload: candidate_payload,
                            },
                        ..
                    },
                    PatternRemainder::Residual(payload),
                ) => Some(*candidate) == variant && candidate_payload == payload,
                _ => false,
            };
            self.append_effect(MirOperationKind::SetDropFlag { flag, initialized })?;
        }
        Ok(())
    }

    pub(super) fn mark_cleanup_complete(
        &mut self,
        target: &CleanupTarget,
    ) -> Result<(), MirLoweringError> {
        match target {
            CleanupTarget::Path(path) => {
                let projections = path
                    .projections()
                    .iter()
                    .map(cleanup_identity_projection)
                    .collect::<Vec<_>>();
                self.set_path_flags(path.root(), &projections, false)
            }
            CleanupTarget::Value { node, .. } => self.mark_value_storage_uninitialized(*node),
            CleanupTarget::EnumResidual {
                subject,
                variant,
                payload,
                ..
            } => self.set_flag(
                &CleanupIdentity::Value {
                    node: *subject,
                    remainder: ValueRemainder::Enum {
                        variant: *variant,
                        payload: payload.clone(),
                    },
                },
                false,
            ),
            CleanupTarget::Place { .. } | CleanupTarget::Region { .. } => Ok(()),
        }
    }

    fn set_path_flags(
        &mut self,
        root: PlaceRoot,
        projections: &[CleanupIdentityProjection],
        initialized: bool,
    ) -> Result<(), MirLoweringError> {
        let flags = self
            .cleanup_flags
            .iter()
            .filter_map(|(identity, flag)| match identity {
                CleanupIdentity::Path {
                    root: candidate,
                    projections: candidate_projections,
                } if *candidate == root
                    && projections_are_prefix(projections, candidate_projections) =>
                {
                    Some(*flag)
                }
                CleanupIdentity::Path { .. } | CleanupIdentity::Value { .. } => None,
            })
            .collect::<Vec<_>>();
        for flag in flags {
            self.append_effect(MirOperationKind::SetDropFlag { flag, initialized })?;
        }
        Ok(())
    }

    fn set_flag(
        &mut self,
        identity: &CleanupIdentity,
        initialized: bool,
    ) -> Result<(), MirLoweringError> {
        if let Some(flag) = self.cleanup_flags.get(identity).copied() {
            self.append_effect(MirOperationKind::SetDropFlag { flag, initialized })?;
        }
        Ok(())
    }
}

fn projections_are_prefix(
    prefix: &[CleanupIdentityProjection],
    projections: &[CleanupIdentityProjection],
) -> bool {
    projections.starts_with(prefix)
}
