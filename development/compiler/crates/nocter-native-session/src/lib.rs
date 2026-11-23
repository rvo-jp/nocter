//! Native compilation orchestration for semantically closed Nocter executables.
//!
//! This crate owns MIR, machine, architecture, and executable-image lowering. Semantic clients
//! depend on `nocter-session` without inheriting these backend layers.

mod native;
mod output;
mod test;
mod value_access;
pub use value_access::{
    ValueAccessLoweringError, compile_value_access_image, compile_value_access_images,
};

pub use native::{
    NativeImageError, NativeImageSetCompileRequest, NativeImageSetError, NativeSessionError,
    compile_native_image, compile_native_images,
};
pub use output::{CompiledNativeImageSet, NativeImage, NativeImageEntry};
pub use test::{
    CompiledNativeTestSet, NativeTestCaseCompilation, NativeTestCompileRequest,
    NativeTestSessionError, NativeTestTargetCompilation, NativeTestTargetOutcome, TestCaseIdentity,
    TestDeclaration, TestTargetIdentity, TestTargetSelectionError, compile_native_tests,
    compile_value_access_tests,
};

#[cfg(test)]
mod tests;
