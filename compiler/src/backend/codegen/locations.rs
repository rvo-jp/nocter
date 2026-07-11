use super::EntryEmitter;
use crate::diagnostics::Diagnostic;
use crate::ir::{BoolLocation, I32Location, SliceLocation, StrLocation, U8Location, UsizeLocation};
use crate::target::arm64::{WReg, XReg};

impl EntryEmitter {
    pub(super) fn i32_location_register(
        &self,
        location: I32Location,
    ) -> Result<WReg, Vec<Diagnostic>> {
        match location {
            I32Location::Return => Ok(WReg::W0),
            I32Location::Parameter(index) => WReg::argument(index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9003",
                    format!("codegen supports at most 8 i32 parameters, got parameter {index}"),
                )]
            }),
            I32Location::Local(index) => WReg::local(index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9004",
                    format!("codegen supports at most 7 i32 locals, got local {index}"),
                )]
            }),
        }
    }

    pub(super) fn usize_location_register(
        &self,
        location: UsizeLocation,
    ) -> Result<XReg, Vec<Diagnostic>> {
        match location {
            UsizeLocation::Return => Ok(XReg::X0),
            UsizeLocation::Parameter(index) => XReg::argument(index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9003",
                    format!("codegen supports at most 8 usize parameters, got parameter {index}"),
                )]
            }),
            UsizeLocation::Local(index) => XReg::local(index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9004",
                    format!("codegen supports at most 7 usize locals, got local {index}"),
                )]
            }),
        }
    }

    pub(super) fn u8_location_register(
        &self,
        location: U8Location,
    ) -> Result<WReg, Vec<Diagnostic>> {
        match location {
            U8Location::Return => Ok(WReg::W0),
            U8Location::Parameter(index) => WReg::argument(index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9003",
                    format!("codegen supports at most 8 u8 parameters, got parameter {index}"),
                )]
            }),
            U8Location::Local(index) => WReg::local(index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9004",
                    format!("codegen supports at most 7 u8 locals, got local {index}"),
                )]
            }),
        }
    }

    pub(super) fn bool_location_register(
        &self,
        location: BoolLocation,
    ) -> Result<WReg, Vec<Diagnostic>> {
        match location {
            BoolLocation::Return => Ok(WReg::W0),
            BoolLocation::Parameter(index) => WReg::argument(index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9003",
                    format!("codegen supports at most 8 bool parameters, got parameter {index}"),
                )]
            }),
            BoolLocation::Local(index) => WReg::local(index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9003",
                    format!("codegen supports at most 7 local scalar bindings, got local {index}"),
                )]
            }),
        }
    }

    pub(super) fn str_location_registers(
        &self,
        location: StrLocation,
    ) -> Result<(XReg, XReg), Vec<Diagnostic>> {
        match location {
            StrLocation::Return => Ok((XReg::X0, XReg::X1)),
            StrLocation::Parameter(index) => {
                let ptr = XReg::argument(index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9003",
                        format!("codegen supports at most 8 ABI parameter words, got parameter word {index}"),
                    )]
                })?;
                let len_index = index + 1;
                let len = XReg::argument(len_index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9003",
                        format!("codegen supports at most 8 ABI parameter words, got parameter word {len_index}"),
                    )]
                })?;
                Ok((ptr, len))
            }
            StrLocation::Local(index) => {
                let ptr = XReg::local(index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9004",
                        format!(
                            "codegen supports at most 7 local ABI words, got local word {index}"
                        ),
                    )]
                })?;
                let len_index = index + 1;
                let len = XReg::local(len_index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9004",
                        format!(
                            "codegen supports at most 7 local ABI words, got local word {len_index}"
                        ),
                    )]
                })?;
                Ok((ptr, len))
            }
        }
    }

    pub(super) fn slice_location_registers(
        &self,
        location: SliceLocation,
    ) -> Result<(XReg, XReg), Vec<Diagnostic>> {
        match location {
            SliceLocation::Return => Ok((XReg::X0, XReg::X1)),
            SliceLocation::Parameter(index) => {
                let ptr = XReg::argument(index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9003",
                        format!("codegen supports at most 8 ABI parameter words, got parameter word {index}"),
                    )]
                })?;
                let len_index = index + 1;
                let len = XReg::argument(len_index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9003",
                        format!("codegen supports at most 8 ABI parameter words, got parameter word {len_index}"),
                    )]
                })?;
                Ok((ptr, len))
            }
            SliceLocation::Local(index) => {
                let ptr = XReg::local(index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9004",
                        format!(
                            "codegen supports at most 7 local ABI words, got local word {index}"
                        ),
                    )]
                })?;
                let len_index = index + 1;
                let len = XReg::local(len_index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9004",
                        format!(
                            "codegen supports at most 7 local ABI words, got local word {len_index}"
                        ),
                    )]
                })?;
                Ok((ptr, len))
            }
        }
    }
}
