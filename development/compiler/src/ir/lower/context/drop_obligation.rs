use crate::ir::UsizeLocation;

use super::AggregateDrop;

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
