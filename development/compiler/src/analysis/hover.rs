//! Hover information derived from compile-unit analysis.

use super::{CompileUnitAnalysis, FileAnalysis};
use crate::comments::attach_documentation;
use crate::resolve::{Symbol, SymbolKind};
use crate::source::{ByteSpan, SourceMap};
use crate::typecheck::{enum_variant_member_label, field_member_label};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HoverInfo {
    pub(crate) span: ByteSpan,
    pub(crate) label: String,
    pub(crate) documentation: Option<String>,
}

mod contents;
mod entry;
mod module_paths;
mod targets;

pub(in crate::analysis::hover) use contents::*;
pub(crate) use entry::{hover_for_file_analysis, hover_for_text};
pub(in crate::analysis::hover) use module_paths::*;
pub(crate) use targets::*;

#[cfg(test)]
mod tests;
