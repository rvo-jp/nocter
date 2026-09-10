/// Native configuration record captured only while Network.framework creates TLS parameters.
///
/// The callback borrows both strings for the duration of the synchronous parameter-construction
/// call and records whether it obtained and configured the Security protocol options. No pointer
/// from this record is retained by Nocter after that call returns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DarwinTlsConfigurationAbiSchema {
    server_name_offset: u64,
    application_protocol_offset: u64,
    configured_offset: u64,
    size: u64,
    alignment: u64,
    minimum_protocol_version: u16,
}

impl DarwinTlsConfigurationAbiSchema {
    pub const ARM64_DARWIN: Self = Self {
        server_name_offset: 0,
        application_protocol_offset: 8,
        configured_offset: 16,
        size: 24,
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

#[cfg(test)]
mod tests {
    use super::DarwinTlsConfigurationAbiSchema;

    #[test]
    fn arm64_configuration_record_has_one_closed_layout() {
        let schema = DarwinTlsConfigurationAbiSchema::ARM64_DARWIN;
        assert_eq!(schema.server_name_offset(), 0);
        assert_eq!(schema.application_protocol_offset(), 8);
        assert_eq!(schema.configured_offset(), 16);
        assert_eq!(schema.size(), 24);
        assert_eq!(schema.alignment(), 8);
        assert_eq!(schema.minimum_protocol_version(), 0x0303);
    }
}
