/// Native configuration record captured only while Network.framework creates TLS parameters.
///
/// The callback borrows both strings for the duration of the synchronous parameter-construction
/// call and records whether it obtained and configured the Security protocol options. No pointer
/// from this record is retained by Nocter after that call returns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinTlsConfigurationAbiSchema {
    server_name_offset: u64,
    application_protocol_offset: u64,
    trust_context_offset: u64,
    verify_queue_offset: u64,
    configured_offset: u64,
    size: u64,
    alignment: u64,
    minimum_protocol_version: u16,
}

impl DarwinTlsConfigurationAbiSchema {
    pub const ARM64_DARWIN: Self = Self {
        server_name_offset: 0,
        application_protocol_offset: 8,
        trust_context_offset: 16,
        verify_queue_offset: 24,
        configured_offset: 32,
        size: 40,
        alignment: 8,
        minimum_protocol_version: 0x0303,
    };

    #[must_use]
    pub const fn server_name_offset(self) -> u64 {
        self.server_name_offset
    }

    #[must_use]
    pub const fn application_protocol_offset(self) -> u64 {
        self.application_protocol_offset
    }

    #[must_use]
    pub const fn configured_offset(self) -> u64 {
        self.configured_offset
    }

    #[must_use]
    pub const fn trust_context_offset(self) -> u64 {
        self.trust_context_offset
    }

    #[must_use]
    pub const fn verify_queue_offset(self) -> u64 {
        self.verify_queue_offset
    }

    #[must_use]
    pub const fn size(self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn alignment(self) -> u64 {
        self.alignment
    }

    #[must_use]
    pub const fn minimum_protocol_version(self) -> u16 {
        self.minimum_protocol_version
    }
}

/// The compiler-owned heap record captured by an optional custom-trust callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinTlsTrustAnchorAbiSchema {
    length_offset: u64,
    bytes_offset: u64,
}

impl DarwinTlsTrustAnchorAbiSchema {
    pub const ARM64_DARWIN: Self = Self {
        length_offset: 0,
        bytes_offset: 8,
    };

    #[must_use]
    pub const fn length_offset(self) -> u64 {
        self.length_offset
    }

    #[must_use]
    pub const fn bytes_offset(self) -> u64 {
        self.bytes_offset
    }

    #[must_use]
    pub const fn allocation_size(self, der_length: u64) -> Option<u64> {
        if der_length == 0 {
            None
        } else {
            self.bytes_offset.checked_add(der_length)
        }
    }
}

/// Fixed Block roles admitted only by the Darwin TLS adapter.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DarwinTlsCallbackRole {
    ConfigureProtocol,
    VerifyTrust,
}

impl DarwinTlsCallbackRole {
    pub const ALL: &'static [Self] = &[Self::ConfigureProtocol, Self::VerifyTrust];

    /// Returns the canonical NUL-terminated Objective-C Block signature.
    #[must_use]
    pub const fn block_signature(self) -> &'static [u8] {
        match self {
            Self::ConfigureProtocol => b"v16@?0^{nw_protocol_options=}8\0",
            Self::VerifyTrust => b"v32@?0^{sec_protocol_metadata=}8^{__SecTrust=}16@?24\0",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DarwinTlsCallbackRole, DarwinTlsConfigurationAbiSchema, DarwinTlsTrustAnchorAbiSchema,
    };

    #[test]
    fn arm64_configuration_record_has_one_closed_layout() {
        let schema = DarwinTlsConfigurationAbiSchema::ARM64_DARWIN;
        assert_eq!(schema.server_name_offset(), 0);
        assert_eq!(schema.application_protocol_offset(), 8);
        assert_eq!(schema.trust_context_offset(), 16);
        assert_eq!(schema.verify_queue_offset(), 24);
        assert_eq!(schema.configured_offset(), 32);
        assert_eq!(schema.size(), 40);
        assert_eq!(schema.alignment(), 8);
        assert_eq!(schema.minimum_protocol_version(), 0x0303);
    }

    #[test]
    fn custom_anchor_record_is_checked_and_callback_roles_are_closed() {
        let schema = DarwinTlsTrustAnchorAbiSchema::ARM64_DARWIN;
        assert_eq!(schema.length_offset(), 0);
        assert_eq!(schema.bytes_offset(), 8);
        assert_eq!(schema.allocation_size(0), None);
        assert_eq!(schema.allocation_size(1), Some(9));
        assert_eq!(schema.allocation_size(u64::MAX), None);
        for role in DarwinTlsCallbackRole::ALL {
            assert!(role.block_signature().ends_with(&[0]));
        }
    }
}
