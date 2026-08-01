use crate::abi::ValueLayout;
use crate::ir::UsizeLocation;

use super::{AggregateDrop, LoweringContext, PendingAggregateDrop};

/// Describes which initialized portion of an aggregate still owns its drop
/// obligation while IR is being lowered.
///
/// Keeping this separate from `AggregateDrop` is intentional: `AggregateDrop`
/// describes the type's drop shape, while this value describes the runtime
/// initialization state of one particular storage location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ir::lower) enum DropObligation {
    Inactive,
    Complete,
    ArrayPrefix { initialized: UsizeLocation },
}

impl DropObligation {
    pub(super) fn for_drop_kind(drop_kind: &Option<AggregateDrop>) -> Self {
        if drop_kind.is_some() {
            Self::Complete
        } else {
            Self::Inactive
        }
    }

    pub(super) fn is_active(self) -> bool {
        !matches!(self, Self::Inactive)
    }
}

impl LoweringContext<'_> {
    pub(in crate::ir::lower) fn register_temporary_array_prefix_drop(
        &mut self,
        slot_index: usize,
        layout: ValueLayout,
        drop_kind: AggregateDrop,
        initialized: UsizeLocation,
    ) -> bool {
        if !matches!(drop_kind, AggregateDrop::Array(_))
            || self
                .temporary_aggregate_drops
                .iter()
                .any(|drop_| drop_.slot_index == slot_index)
        {
            return false;
        }
        self.temporary_aggregate_drops.push(PendingAggregateDrop {
            name: format!("temporary aggregate slot {slot_index}"),
            slot_index,
            layout,
            drop_kind,
            obligation: DropObligation::ArrayPrefix { initialized },
        });
        true
    }

    pub(in crate::ir::lower) fn release_temporary_aggregate_drop(
        &mut self,
        slot_index: usize,
    ) -> bool {
        let old_len = self.temporary_aggregate_drops.len();
        self.temporary_aggregate_drops
            .retain(|drop_| drop_.slot_index != slot_index);
        self.temporary_aggregate_drops.len() != old_len
    }

    pub(super) fn pending_temporary_aggregate_drops(&self) -> Vec<PendingAggregateDrop> {
        self.temporary_aggregate_drops
            .iter()
            .rev()
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::CallTarget;

    #[test]
    fn drop_shape_activates_complete_obligation() {
        let drop_kind = Some(AggregateDrop::Direct(super::super::DropGlue {
            target: CallTarget::same_file("drop_Item"),
        }));

        assert_eq!(
            DropObligation::for_drop_kind(&drop_kind),
            DropObligation::Complete
        );
    }

    #[test]
    fn absent_drop_shape_has_no_obligation() {
        assert_eq!(
            DropObligation::for_drop_kind(&None),
            DropObligation::Inactive
        );
    }

    #[test]
    fn an_initialized_array_prefix_is_active() {
        assert!(
            DropObligation::ArrayPrefix {
                initialized: UsizeLocation::Local(3),
            }
            .is_active()
        );
    }
}
