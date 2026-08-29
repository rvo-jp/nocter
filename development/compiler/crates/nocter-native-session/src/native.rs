use nocter_arm64::{Arm64LoweringError, Arm64Program};
use nocter_machine::{MachineProgram, MachineProgramError};
use nocter_macho::{MachOError, MachOImage};
use nocter_mir::{MirLoweringError, lower_executable};
use std::fmt;

use nocter_session::{
    CompiledTarget, ExecutableCompileRequest, ExecutableIdentity, ExecutableSessionError,
    close_root_executables, compile_executable,
};

use crate::output::{CompiledNativeImage, CompiledNativeImageSet, NativeImage, NativeImageEntry};

/// Compiles one selected executable through the complete native image boundary.
///
/// # Errors
///
/// Returns the exact semantic, MIR, machine, target, or writer boundary that rejected the program.
pub fn compile_native_image(
    request: ExecutableCompileRequest,
) -> Result<CompiledNativeImage, NativeSessionError> {
    let executable = compile_executable(request)?;
    let identity = executable.identity().clone();
    let (executable, source_index) = executable.into_parts();
    let image = lower_native_image(executable)?;
    Ok(CompiledNativeImage::new(identity, image, source_index))
}

/// Closed request to compile every executable owned by the command-root packages.
#[derive(Debug)]
pub struct NativeImageSetCompileRequest {
    target: CompiledTarget,
}

impl NativeImageSetCompileRequest {
    #[must_use]
    pub const fn all(target: CompiledTarget) -> Self {
        Self { target }
    }
}

/// Compiles every root executable through one shared target program.
///
/// Entries preserve canonical package-target declaration order. The function returns no partial
/// set when one executable fails specialization or backend lowering.
///
/// # Errors
///
/// Rejects a root set with no executable or identifies the exact executable whose closure or
/// native lowering failed.
pub fn compile_native_images(
    request: NativeImageSetCompileRequest,
) -> Result<CompiledNativeImageSet, NativeImageSetError> {
    let NativeImageSetCompileRequest { target } = request;
    let (target, source_index) = target.into_parts();
    let executables = close_root_executables(target).map_err(|error| {
        let (executable, error) = error.into_parts();
        NativeImageSetError::Image {
            executable,
            error: NativeImageError::Executable(error),
        }
    })?;
    if executables.is_empty() {
        return Err(NativeImageSetError::NoExecutable);
    }
    let mut entries = Vec::with_capacity(executables.len());
    for executable in executables {
        let (identity, executable) = executable.into_parts();
        let image = lower_native_image(executable).map_err(|error| NativeImageSetError::Image {
            executable: identity.clone(),
            error,
        })?;
        entries.push(NativeImageEntry::new(identity, image));
    }
    Ok(CompiledNativeImageSet::new(entries, source_index))
}

fn lower_native_image(
    executable: nocter_target_program::ExecutableProgram,
) -> Result<NativeImage, NativeImageError> {
    let mir = lower_executable(executable)?;
    let machine = MachineProgram::lower(&mir)?;
    let arm64 = Arm64Program::lower_machine(&machine)?;
    Ok(NativeImage::from_macho(MachOImage::build(&arm64)?))
}

#[derive(Debug)]
pub enum NativeImageError {
    Executable(nocter_target_program::ExecutableProgramError),
    Mir(MirLoweringError),
    Machine(MachineProgramError),
    Arm64(Arm64LoweringError),
    Image(MachOError),
    Integrity(Box<str>),
}

impl fmt::Display for NativeImageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Executable(error) => write!(formatter, "executable closure failed: {error}"),
            Self::Mir(error) => write!(formatter, "MIR lowering failed: {error}"),
            Self::Machine(error) => write!(formatter, "machine lowering failed: {error}"),
            Self::Arm64(error) => write!(formatter, "ARM64 lowering failed: {error}"),
            Self::Image(error) => write!(formatter, "Mach-O construction failed: {error}"),
            Self::Integrity(message) => {
                write!(formatter, "native image integrity failed: {message}")
            }
        }
    }
}

impl std::error::Error for NativeImageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Executable(error) => Some(error),
            Self::Mir(error) => Some(error),
            Self::Machine(error) => Some(error),
            Self::Arm64(error) => Some(error),
            Self::Image(error) => Some(error),
            Self::Integrity(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum NativeImageSetError {
    NoExecutable,
    Image {
        executable: ExecutableIdentity,
        error: NativeImageError,
    },
}

impl NativeImageSetError {
    #[must_use]
    pub const fn diagnostic_code(&self) -> Option<&'static str> {
        match self {
            Self::NoExecutable => Some("E0800"),
            Self::Image { .. } => None,
        }
    }
}

impl fmt::Display for NativeImageSetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoExecutable => formatter.write_str("compile roots declare no executable"),
            Self::Image { executable, error } => write!(
                formatter,
                "native compilation of {} ({}) failed: {error}",
                executable.name(),
                executable.package().as_str()
            ),
        }
    }
}

impl std::error::Error for NativeImageSetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NoExecutable => None,
            Self::Image { error, .. } => Some(error),
        }
    }
}

#[derive(Debug)]
pub enum NativeSessionError {
    Executable(ExecutableSessionError),
    Image(NativeImageError),
}

impl NativeSessionError {
    #[must_use]
    pub const fn diagnostic_code(&self) -> Option<&'static str> {
        match self {
            Self::Executable(error) => error.diagnostic_code(),
            Self::Image(_) => None,
        }
    }
}

impl fmt::Display for NativeSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Executable(error) => write!(formatter, "executable compilation failed: {error}"),
            Self::Image(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for NativeSessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Executable(error) => Some(error),
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
        Self::Image(NativeImageError::Mir(error))
    }
}

impl From<MachineProgramError> for NativeSessionError {
    fn from(error: MachineProgramError) -> Self {
        Self::Image(NativeImageError::Machine(error))
    }
}

impl From<Arm64LoweringError> for NativeSessionError {
    fn from(error: Arm64LoweringError) -> Self {
        Self::Image(NativeImageError::Arm64(error))
    }
}

impl From<MachOError> for NativeSessionError {
    fn from(error: MachOError) -> Self {
        Self::Image(NativeImageError::Image(error))
    }
}

impl From<NativeImageError> for NativeSessionError {
    fn from(error: NativeImageError) -> Self {
        Self::Image(error)
    }
}

impl From<MirLoweringError> for NativeImageError {
    fn from(error: MirLoweringError) -> Self {
        Self::Mir(error)
    }
}

impl From<MachineProgramError> for NativeImageError {
    fn from(error: MachineProgramError) -> Self {
        Self::Machine(error)
    }
}

impl From<Arm64LoweringError> for NativeImageError {
    fn from(error: Arm64LoweringError) -> Self {
        Self::Arm64(error)
    }
}

impl From<MachOError> for NativeImageError {
    fn from(error: MachOError) -> Self {
        Self::Image(error)
    }
}
