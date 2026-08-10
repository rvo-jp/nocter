use super::support::ParsedIdentifier;
use super::{ParseResult, Parser};
use crate::ast::{
    AssociatedTypeBinding, AssociatedTypeDecl, AstFile, BorrowType, ConformanceDecl,
    ConformanceMember, DestructDecl, EnumDecl, EnumVariant, FromImportItem, FunctionDecl,
    FunctionOwner, ImportAlias, ImportItem, ImportedName, InstanceDecl, InterfaceDecl, Item,
    MethodDecl, MethodReceiver, MethodReceiverMode, ModulePath, Parameter, ParameterList,
    PrimitiveDecl, ResultProvenanceClause, ResultProvenanceOrigin, ResultProvenanceOriginKind,
    StructDecl, StructField, TargetDirective, TypeAliasDecl, TypeExpr, TypeReference, Visibility,
};
use crate::lexer::Keyword;
use crate::literals::decode_string_literal_bytes;
use crate::source::ByteSpan;

mod aggregates;
mod coercions;
mod conformances;
mod destructors;
mod entry;
mod functions;
mod imports;
mod instances;
mod interfaces;
mod methods;
mod package_directives;
mod parameters;
mod provenance;
mod tests;
pub(super) mod type_owners;
mod type_patterns;
