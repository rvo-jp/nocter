//! The single semantic product of type checking.

use super::TypedHir;
use crate::diagnostics::Diagnostic;

/// Partial typed semantics and diagnostics produced by one checker invocation.
///
/// `TypedHir` deliberately survives diagnostics. Editor and lowering consumers
/// must use this result instead of walking the AST to infer successful semantic
/// decisions a second time.
#[derive(Debug)]
pub(crate) struct TypecheckOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) typed_hir: TypedHir,
}

impl TypecheckOutput {
    pub(crate) fn new(diagnostics: Vec<Diagnostic>, typed_hir: TypedHir) -> Self {
        Self {
            diagnostics,
            typed_hir,
        }
    }
}
