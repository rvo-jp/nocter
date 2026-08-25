//! Native compilation orchestration for semantically closed Nocter executables.
//!
//! This crate owns MIR, machine, architecture, and executable-image lowering. Semantic clients
//! depend on `nocter-session` without inheriting these backend layers.

mod native;
mod output;
mod test;

pub use native::{
    NativeImageError, NativeImageSetCompileRequest, NativeImageSetError, NativeSessionError,
    compile_native_image, compile_native_images,
};
pub use output::{CompiledNativeImage, CompiledNativeImageSet, NativeImage, NativeImageEntry};
pub use test::{
    CompiledNativeTestSet, NativeTestCompileRequest, NativeTestImage, NativeTestSessionError,
    NativeTestTargetCompilation, NativeTestTargetOutcome, TestCaseIdentity, TestTargetIdentity,
    TestTargetSelectionError, compile_native_tests,
};

use nocter_session::{close_executable, root_executables};

#[cfg(test)]
mod tests;
