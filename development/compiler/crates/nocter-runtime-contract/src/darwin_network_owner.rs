/// Fields in the fixed compiler-owned Darwin network owner record.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinNetworkOwnerField {
    NativeObject,
    SerialQueue,
    EventReader,
    EventWriter,
    Lifecycle,
}

impl DarwinNetworkOwnerField {
    pub const ALL: &'static [Self] = &[
        Self::NativeObject,
        Self::SerialQueue,
        Self::EventReader,
        Self::EventWriter,
        Self::Lifecycle,
    ];

    /// Identifies the runtime family that owns terminal cleanup of this field.
    #[must_use]
    pub const fn resource_family(self) -> DarwinNetworkOwnerResourceFamily {
        match self {
            Self::NativeObject => DarwinNetworkOwnerResourceFamily::NetworkObject,
            Self::SerialQueue => DarwinNetworkOwnerResourceFamily::DispatchObject,
            Self::EventReader | Self::EventWriter => {
                DarwinNetworkOwnerResourceFamily::FileDescriptor
            }
            Self::Lifecycle => DarwinNetworkOwnerResourceFamily::Value,
        }
    }
}

/// Runtime cleanup families retained in one native owner.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinNetworkOwnerResourceFamily {
    NetworkObject,
    DispatchObject,
    FileDescriptor,
    Value,
}

/// Fixed native record shared by connection and listener adapter owners on ARM64 Darwin.
///
/// Temporary endpoint, parameter, Block, dispatch-data, path, and error objects are deliberately
/// absent. The operation that creates or consumes them must also dispose of them, so an owner has
/// one bounded terminal release plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinNetworkOwnerAbiSchema {
    offsets: [u64; 5],
    size: u64,
    alignment: u64,
}

impl DarwinNetworkOwnerAbiSchema {
    pub const ARM64_DARWIN: Self = Self {
        offsets: [0, 8, 16, 24, 32],
        size: 40,
        alignment: 8,
    };

    /// Terminal cleanup order after the checked lifecycle reaches `quiesced`.
    ///
    /// The native object is released first, then the serial queue, then the writer and reader ends
    /// of the callback channel. Lifecycle is an inline value and requires no cleanup.
    pub const RELEASE_ORDER: &'static [DarwinNetworkOwnerField] = &[
        DarwinNetworkOwnerField::NativeObject,
        DarwinNetworkOwnerField::SerialQueue,
        DarwinNetworkOwnerField::EventWriter,
        DarwinNetworkOwnerField::EventReader,
    ];

    #[must_use]
    pub const fn offset(self, field: DarwinNetworkOwnerField) -> u64 {
        self.offsets[field as usize]
    }

    #[must_use]
    pub const fn size(self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn alignment(self) -> u64 {
        self.alignment
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DarwinNetworkOwnerAbiSchema, DarwinNetworkOwnerField, DarwinNetworkOwnerResourceFamily,
    };

    #[test]
    fn owner_layout_is_dense_and_each_resource_has_one_cleanup_family() {
        let schema = DarwinNetworkOwnerAbiSchema::ARM64_DARWIN;
        for (index, field) in DarwinNetworkOwnerField::ALL.iter().copied().enumerate() {
            assert_eq!(schema.offset(field), (index as u64) * 8);
        }
        assert_eq!(schema.size(), 40);
        assert_eq!(schema.alignment(), 8);
        assert_eq!(
            DarwinNetworkOwnerAbiSchema::RELEASE_ORDER
                .iter()
                .copied()
                .map(DarwinNetworkOwnerField::resource_family)
                .collect::<Vec<_>>(),
            vec![
                DarwinNetworkOwnerResourceFamily::NetworkObject,
                DarwinNetworkOwnerResourceFamily::DispatchObject,
                DarwinNetworkOwnerResourceFamily::FileDescriptor,
                DarwinNetworkOwnerResourceFamily::FileDescriptor,
            ]
        );
        assert_eq!(
            DarwinNetworkOwnerField::Lifecycle.resource_family(),
            DarwinNetworkOwnerResourceFamily::Value
        );
    }
}
