use super::support::ParsedIdentifier;
use super::{ParseResult, Parser};
use crate::ast::{
    AstFile, BorrowType, DropDecl, EnumDecl, EnumVariant, FromImportItem, FunctionDecl,
    FunctionOwner, ImplDecl, ImplMember, ImportAlias, ImportItem, ImportedName, InterfaceDecl,
    Item, MethodDecl, MethodReceiver, MethodReceiverMode, ModulePath, Parameter, ParameterList,
    PrimitiveDecl, ResultAllocationModifier, ResultProvenanceClause, ResultProvenanceOrigin,
    ResultProvenanceOriginKind, StructDecl, StructField, TargetDirective, TypeAliasDecl, TypeExpr,
    TypeReference, Visibility,
};
use crate::lexer::Keyword;
use crate::literals::decode_string_literal_bytes;
use crate::source::ByteSpan;

mod aggregates;
mod entry;
mod functions;
mod implementations;
mod imports;
mod package_directives;
mod parameters;
mod provenance;
mod tests;
