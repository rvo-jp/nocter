use std::fmt;

use nocter_arm64::{Arm64LoweringError, Arm64Program};
use nocter_machine::{MachineProgram, MachineProgramError};
use nocter_macho::{MachOError, MachOImage};
use nocter_mir::{MirLoweringError, lower_executable};

use crate::{
    CompiledNativeImage, ExecutableCompileRequest, ExecutableSessionError, compile_executable,
};

/// Compiles one selected executable through the complete native image boundary.
///
/// # Errors
///
/// Returns the exact semantic, MIR, machine, target, or writer boundary that rejected the program.
pub fn compile_native_image(
    request: ExecutableCompileRequest<'_>,
) -> Result<CompiledNativeImage, NativeSessionError> {
    let executable = compile_executable(request)?;
    let (executable, source_index) = executable.into_parts();
    let mir = lower_executable(executable)?;
    let machine = MachineProgram::lower(&mir)?;
    let arm64 = Arm64Program::lower_machine(&machine)?;
    let image = MachOImage::build(&arm64)?;
    Ok(CompiledNativeImage::new(image, source_index))
}

#[derive(Debug)]
pub enum NativeSessionError {
    Executable(ExecutableSessionError),
    Mir(MirLoweringError),
    Machine(MachineProgramError),
    Arm64(Arm64LoweringError),
    Image(MachOError),
}

impl fmt::Display for NativeSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Executable(error) => write!(formatter, "executable compilation failed: {error}"),
            Self::Mir(error) => write!(formatter, "MIR lowering failed: {error}"),
            Self::Machine(error) => write!(formatter, "machine lowering failed: {error}"),
            Self::Arm64(error) => write!(formatter, "ARM64 lowering failed: {error}"),
            Self::Image(error) => write!(formatter, "Mach-O construction failed: {error}"),
        }
    }
}

impl std::error::Error for NativeSessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Executable(error) => Some(error),
            Self::Mir(error) => Some(error),
            Self::Machine(error) => Some(error),
            Self::Arm64(error) => Some(error),
            Self::Image(error) => Some(error),
        }
    }
}

impl From<ExecutableSessionError> for NativeSessionError {
    fn from(error: ExecutableSessionError) -> Self {
        Self::Executable(error)
    }
}

impl From<MirLoweringError> for NativeSessionError {
    fn from(error: MirLoweringError) -> Self {
        Self::Mir(error)
    }
}

impl From<MachineProgramError> for NativeSessionError {
    fn from(error: MachineProgramError) -> Self {
        Self::Machine(error)
    }
}

impl From<Arm64LoweringError> for NativeSessionError {
    fn from(error: Arm64LoweringError) -> Self {
        Self::Arm64(error)
    }
}

impl From<MachOError> for NativeSessionError {
    fn from(error: MachOError) -> Self {
        Self::Image(error)
    }
}
