/// Native socket-address bytes admitted at the Network.framework boundary on Darwin.
///
/// The target adapter copies one complete supported record into caller-owned storage before it
/// releases the provider endpoint. Source code therefore observes neither provider ownership nor
/// a borrowed native pointer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinNetworkSocketAddressAbiSchema {
    maximum_size: u64,
    ipv4_size: u64,
    ipv6_size: u64,
    ipv4_family: u64,
    ipv6_family: u64,
    length_offset: u64,
    family_offset: u64,
}

impl DarwinNetworkSocketAddressAbiSchema {
    pub const ARM64_DARWIN: Self = Self {
        maximum_size: 28,
        ipv4_size: 16,
        ipv6_size: 28,
        ipv4_family: 2,
        ipv6_family: 30,
        length_offset: 0,
        family_offset: 1,
    };

    #[must_use]
    pub const fn maximum_size(self) -> u64 {
        self.maximum_size
    }

    #[must_use]
    pub const fn length_offset(self) -> u64 {
        self.length_offset
    }

    #[must_use]
    pub const fn family_offset(self) -> u64 {
        self.family_offset
    }

    #[must_use]
    pub const fn ipv4(self) -> (u64, u64) {
        (self.ipv4_size, self.ipv4_family)
    }

    #[must_use]
    pub const fn ipv6(self) -> (u64, u64) {
        (self.ipv6_size, self.ipv6_family)
    }

    #[must_use]
    pub const fn supports(self, size: u64, family: u64) -> bool {
        (size == self.ipv4_size && family == self.ipv4_family)
            || (size == self.ipv6_size && family == self.ipv6_family)
    }
}

#[cfg(test)]
mod tests {
    use super::DarwinNetworkSocketAddressAbiSchema;

    #[test]
    fn schema_admits_only_complete_ipv4_and_ipv6_records() {
        let schema = DarwinNetworkSocketAddressAbiSchema::ARM64_DARWIN;
        assert!(schema.supports(16, 2));
        assert!(schema.supports(28, 30));
        assert!(!schema.supports(16, 30));
        assert!(!schema.supports(28, 2));
        assert!(!schema.supports(24, 2));
        assert_eq!(schema.maximum_size(), 28);
        assert_eq!(schema.length_offset(), 0);
        assert_eq!(schema.family_offset(), 1);
    }
}
