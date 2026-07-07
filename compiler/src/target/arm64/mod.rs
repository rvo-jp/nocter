//! ARM64 instruction selection and binary instruction encoding.

mod encoder;

pub(crate) use encoder::{Encoder, MoveWideShift, WReg, XReg};
