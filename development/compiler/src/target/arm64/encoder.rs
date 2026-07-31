#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Encoder {
    bytes: Vec<u8>,
}

mod arithmetic;
mod branches;
mod encoding;
mod immediates;
mod load_store;
mod registers;
#[cfg(test)]
mod tests;

pub(in crate::target::arm64::encoder) use encoding::*;
pub(crate) use registers::{BranchCondition, MoveWideShift, WReg, XReg};

impl Encoder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn position(&self) -> usize {
        self.bytes.len()
    }

    pub(crate) fn finish(self) -> Vec<u8> {
        self.bytes
    }

    pub(in crate::target::arm64::encoder) fn emit_word(&mut self, word: u32) {
        self.bytes.extend_from_slice(&word.to_le_bytes());
    }
}
