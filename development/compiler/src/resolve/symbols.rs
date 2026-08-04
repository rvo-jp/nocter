use crate::ast::{
    BindingKind, CallExpr, ClosureCaptureMode, Expr, IdentifierExpr, LiteralShape, MethodReceiver,
    ResultProvenanceClause, TypeExpr, Visibility,
};
use crate::diagnostics::Diagnostic;
use crate::semantics::TrustedDeclarationFacts;
use crate::source::{ByteSpan, SourceId};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(u32);

impl SymbolId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveOutput {
    pub symbols: SymbolTable,
    pub access: ImportAccess,
    pub diagnostics: Vec<Diagnostic>,
    pub(super) identifier_targets: HashMap<ByteSpan, SymbolId>,
    pub(super) call_targets: HashMap<ByteSpan, SymbolId>,
    pub(super) typed_literal_targets: HashMap<ByteSpan, LiteralResolution>,
    pub(super) local_symbols: Vec<LocalSymbol>,
    pub(super) local_identifier_targets: HashMap<ByteSpan, LocalSymbolId>,
    pub(crate) trusted_declarations: TrustedDeclarationFacts,
}

impl ResolveOutput {
    pub fn symbol_for_identifier(&self, identifier: &IdentifierExpr) -> Option<&Symbol> {
        self.identifier_targets
            .get(&identifier.span)
            .and_then(|id| self.symbols.get(*id))
    }

    pub fn symbol_for_call(&self, call: &CallExpr) -> Option<&Symbol> {
        self.call_targets
            .get(&call.span)
            .and_then(|id| self.symbols.get(*id))
    }

    pub fn local_symbol_for_identifier(&self, identifier: &IdentifierExpr) -> Option<&LocalSymbol> {
        self.local_identifier_targets
            .get(&identifier.span)
            .and_then(|id| self.local_symbol(*id))
    }

    pub fn local_symbol(&self, id: LocalSymbolId) -> Option<&LocalSymbol> {
        self.local_symbols.get(id.raw() as usize)
    }

    pub fn local_symbols(&self) -> impl Iterator<Item = &LocalSymbol> {
        self.local_symbols.iter()
    }

    pub fn symbol_identifier_references(&self) -> impl Iterator<Item = (ByteSpan, &Symbol)> + '_ {
        self.identifier_targets
            .iter()
            .filter_map(|(span, id)| self.symbols.get(*id).map(|symbol| (*span, symbol)))
    }

    pub fn local_symbol_identifier_references(
        &self,
    ) -> impl Iterator<Item = (ByteSpan, &LocalSymbol)> + '_ {
        self.local_identifier_targets
            .iter()
            .filter_map(|(span, id)| self.local_symbol(*id).map(|symbol| (*span, symbol)))
    }

    pub fn symbol_reference_at_offset(&self, offset: usize) -> Option<(ByteSpan, &Symbol)> {
        self.identifier_targets
            .iter()
            .filter(|(span, _)| span_contains(**span, offset))
            .min_by_key(|(span, _)| (span.len(), span.start))
            .and_then(|(span, id)| self.symbols.get(*id).map(|symbol| (*span, symbol)))
    }

    pub fn local_symbol_reference_at_offset(
        &self,
        offset: usize,
    ) -> Option<(ByteSpan, &LocalSymbol)> {
        self.local_identifier_targets
            .iter()
            .filter(|(span, _)| span_contains(**span, offset))
            .min_by_key(|(span, _)| (span.len(), span.start))
            .and_then(|(span, id)| self.local_symbol(*id).map(|symbol| (*span, symbol)))
    }

    pub fn function_signature_for_call(&self, call: &CallExpr) -> Option<&FunctionSignature> {
        match self.symbol_for_call(call).map(|symbol| &symbol.kind) {
            Some(SymbolKind::Function(signature) | SymbolKind::Primitive(signature)) => {
                Some(signature)
            }
            Some(SymbolKind::Type(_) | SymbolKind::Imported(_)) | None => None,
        }
    }

    pub fn associated_function_signature_for_call(
        &self,
        call: &CallExpr,
    ) -> Option<&FunctionSignature> {
        self.associated_function_for_call(call)
            .map(|(_, function)| &function.signature)
    }

    pub fn associated_function_for_call(
        &self,
        call: &CallExpr,
    ) -> Option<(&TypeSymbol, &AssociatedFunctionSignature)> {
        let Expr::Member(member) = call.callee.as_ref() else {
            return None;
        };
        let Expr::Identifier(type_name) = member.object.as_ref() else {
            return None;
        };

        let Some(SymbolKind::Type(type_symbol)) = self
            .symbol_for_identifier(type_name)
            .map(|symbol| &symbol.kind)
        else {
            return None;
        };

        let function = type_symbol
            .associated_functions
            .iter()
            .find(|function| function.is_accessible && function.name == member.member)?;
        Some((type_symbol, function))
    }

    pub fn call_signature_for_call(&self, call: &CallExpr) -> Option<&FunctionSignature> {
        self.function_signature_for_call(call)
            .or_else(|| self.associated_function_signature_for_call(call))
    }

    pub fn literal_resolution(&self, span: ByteSpan) -> Option<&LiteralResolution> {
        self.typed_literal_targets.get(&span)
    }

    pub fn literal_signature(&self, resolution: &LiteralResolution) -> Option<&LiteralSignature> {
        let SymbolKind::Type(symbol) = &self.symbols.get(resolution.type_symbol)?.kind else {
            return None;
        };
        symbol
            .literals
            .iter()
            .find(|literal| literal.declaration_span == resolution.literal_declaration_span)
    }

    pub fn method_signature_by_name_span(&self, name_span: ByteSpan) -> Option<&MethodSignature> {
        self.symbols
            .symbols()
            .find_map(|symbol| match &symbol.kind {
                SymbolKind::Type(type_symbol) => type_symbol
                    .methods
                    .iter()
                    .chain(
                        type_symbol
                            .interface_conformances
                            .iter()
                            .flat_map(|conformance| &conformance.methods),
                    )
                    .find(|method| method.name_span == name_span),
                SymbolKind::Function(_) | SymbolKind::Primitive(_) | SymbolKind::Imported(_) => {
                    None
                }
            })
    }

    pub fn call_name_for_diagnostic(&self, call: &CallExpr) -> String {
        if let Some(symbol) = self.symbol_for_call(call) {
            return symbol.name.clone();
        }

        if let Expr::Member(member) = call.callee.as_ref()
            && let Expr::Identifier(type_name) = member.object.as_ref()
        {
            return format!("{}.{}", type_name.name, member.member);
        }

        "<unknown>".to_string()
    }

    pub fn type_symbol_by_name(&self, name: &str) -> Option<&TypeSymbol> {
        match self.symbols.symbol_by_name(name).map(|symbol| &symbol.kind) {
            Some(SymbolKind::Type(symbol)) => Some(symbol),
            Some(SymbolKind::Function(_) | SymbolKind::Primitive(_) | SymbolKind::Imported(_))
            | None => None,
        }
    }

    pub fn type_symbol_by_reference_name(&self, name: &str) -> Option<&TypeSymbol> {
        self.type_symbol_by_name(name)
            .or_else(|| self.type_symbol_by_canonical_name(name))
    }

    pub fn type_symbol_definition_by_name(&self, name: &str) -> Option<(&Symbol, &TypeSymbol)> {
        let symbol = self.symbols.symbol_by_name(name)?;
        match &symbol.kind {
            SymbolKind::Type(type_symbol) => Some((symbol, type_symbol)),
            SymbolKind::Function(_) | SymbolKind::Primitive(_) | SymbolKind::Imported(_) => None,
        }
    }

    pub fn type_symbol_definition_by_reference_name(
        &self,
        name: &str,
    ) -> Option<(&Symbol, &TypeSymbol)> {
        self.type_symbol_definition_by_name(name)
            .or_else(|| self.type_symbol_definition_by_canonical_name(name))
    }

    pub fn type_symbol_by_canonical_name(&self, canonical_name: &str) -> Option<&TypeSymbol> {
        self.symbols
            .symbols
            .iter()
            .find_map(|symbol| match &symbol.kind {
                SymbolKind::Type(type_symbol) if type_symbol.canonical_name == canonical_name => {
                    Some(type_symbol)
                }
                SymbolKind::Function(_)
                | SymbolKind::Primitive(_)
                | SymbolKind::Type(_)
                | SymbolKind::Imported(_) => None,
            })
    }

    pub(crate) fn type_symbol_definition_by_canonical_name(
        &self,
        canonical_name: &str,
    ) -> Option<(&Symbol, &TypeSymbol)> {
        self.symbols
            .symbols
            .iter()
            .find_map(|symbol| match &symbol.kind {
                SymbolKind::Type(type_symbol) if type_symbol.canonical_name == canonical_name => {
                    Some((symbol, type_symbol))
                }
                SymbolKind::Function(_)
                | SymbolKind::Primitive(_)
                | SymbolKind::Type(_)
                | SymbolKind::Imported(_) => None,
            })
    }

    pub(super) fn new(access: ImportAccess) -> Self {
        Self {
            symbols: SymbolTable::new(),
            access,
            diagnostics: Vec::new(),
            identifier_targets: HashMap::new(),
            call_targets: HashMap::new(),
            typed_literal_targets: HashMap::new(),
            local_symbols: Vec::new(),
            local_identifier_targets: HashMap::new(),
            trusted_declarations: TrustedDeclarationFacts::default(),
        }
    }

    pub(super) fn define_local_symbol(
        &mut self,
        name: String,
        name_span: ByteSpan,
        kind: LocalSymbolKind,
    ) -> LocalSymbolId {
        let id = LocalSymbolId(self.local_symbols.len() as u32);
        self.local_symbols.push(LocalSymbol {
            id,
            name,
            name_span,
            kind,
        });
        id
    }
}

fn span_contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalSymbolId(u32);

impl LocalSymbolId {
    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSymbol {
    pub id: LocalSymbolId,
    pub name: String,
    pub name_span: ByteSpan,
    pub kind: LocalSymbolKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalSymbolKind {
    Parameter,
    Binding(BindingKind),
    Region,
    LiteralCapture,
    ClosureCapture(ClosureCaptureMode),
    PatternPayload,
    CatchError,
    ForRange,
    CollectionFor,
    LiteralPackFor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolTable {
    pub(super) symbols: Vec<Symbol>,
    pub(super) by_name: HashMap<String, SymbolId>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            symbols: Vec::new(),
            by_name: HashMap::new(),
        }
    }

    pub fn get(&self, id: SymbolId) -> Option<&Symbol> {
        self.symbols.get(id.raw() as usize)
    }

    pub fn symbol_by_name(&self, name: &str) -> Option<&Symbol> {
        self.by_name.get(name).and_then(|id| self.get(*id))
    }

    pub fn symbols(&self) -> impl Iterator<Item = &Symbol> {
        self.symbols.iter()
    }

    pub(super) fn get_mut(&mut self, id: SymbolId) -> Option<&mut Symbol> {
        self.symbols.get_mut(id.raw() as usize)
    }

    pub(super) fn id_by_name(&self, name: &str) -> Option<SymbolId> {
        self.by_name.get(name).copied()
    }

    pub(super) fn define(
        &mut self,
        name: String,
        name_span: ByteSpan,
        declaration_span: ByteSpan,
        kind: SymbolKind,
    ) -> Result<SymbolId, SymbolId> {
        self.define_indexed(name, name_span, declaration_span, kind, false)
    }

    pub(super) fn ensure_hidden_resolvable(
        &mut self,
        name: String,
        name_span: ByteSpan,
        declaration_span: ByteSpan,
        kind: SymbolKind,
    ) {
        if self.by_name.contains_key(&name) {
            return;
        }

        let id = self.push_symbol(name.clone(), name_span, declaration_span, kind, true);
        self.by_name.insert(name, id);
    }

    fn define_indexed(
        &mut self,
        name: String,
        name_span: ByteSpan,
        declaration_span: ByteSpan,
        kind: SymbolKind,
        is_hidden: bool,
    ) -> Result<SymbolId, SymbolId> {
        if let Some(existing) = self.by_name.get(&name) {
            return Err(*existing);
        }

        let id = self.push_symbol(name.clone(), name_span, declaration_span, kind, is_hidden);
        self.by_name.insert(name, id);
        Ok(id)
    }

    pub(super) fn define_hidden(
        &mut self,
        name: String,
        name_span: ByteSpan,
        declaration_span: ByteSpan,
        kind: SymbolKind,
    ) -> SymbolId {
        self.push_symbol(name, name_span, declaration_span, kind, true)
    }

    fn push_symbol(
        &mut self,
        name: String,
        name_span: ByteSpan,
        declaration_span: ByteSpan,
        kind: SymbolKind,
        is_hidden: bool,
    ) -> SymbolId {
        let id = SymbolId(self.symbols.len() as u32);
        self.symbols.push(Symbol {
            id,
            name,
            name_span,
            declaration_span,
            is_hidden,
            kind,
        });
        id
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub name_span: ByteSpan,
    pub declaration_span: ByteSpan,
    pub is_hidden: bool,
    pub kind: SymbolKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Function(FunctionSignature),
    Primitive(FunctionSignature),
    Type(TypeSymbol),
    Imported(ImportedSymbol),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSignature {
    pub generic_parameters: Vec<String>,
    pub generic_parameter_bounds: Vec<Vec<TypeExpr>>,
    pub parameters: Vec<ParameterSignature>,
    pub return_type: TypeExpr,
    pub result_provenance: Option<ResultProvenanceClause>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeSymbol {
    pub kind: TypeSymbolKind,
    pub canonical_name: String,
    pub generic_parameters: Vec<String>,
    pub generic_parameter_bounds: Vec<Vec<TypeExpr>>,
    pub generic_arity: usize,
    pub is_copy: bool,
    pub alias_target: Option<TypeExpr>,
    pub fields: Vec<StructFieldSignature>,
    pub variants: Vec<EnumVariantSignature>,
    pub associated_functions: Vec<AssociatedFunctionSignature>,
    pub methods: Vec<MethodSignature>,
    pub interface_conformances: Vec<InterfaceConformance>,
    pub drop_member: Option<DropSignature>,
    pub literals: Vec<LiteralSignature>,
    pub construction: ConstructionSurface,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConstructionSurface {
    pub declaration_span: Option<ByteSpan>,
    pub entries: Vec<ConstructionEntry>,
    pub default_entry: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructionEntry {
    pub kind: ConstructionEntryKind,
    pub declaration_span: ByteSpan,
    pub focus_span: ByteSpan,
    pub is_accessible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConstructionEntryKind {
    Structural,
    Function(String),
    Literal(LiteralShape),
    Variant(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceConformance {
    pub declaration_span: ByteSpan,
    pub generic_parameters: Vec<String>,
    pub generic_parameter_bounds: Vec<Vec<TypeExpr>>,
    pub interface_ty: TypeExpr,
    pub target_ty: TypeExpr,
    pub methods: Vec<MethodSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralSignature {
    pub shape: LiteralShape,
    pub visibility: Visibility,
    pub is_accessible: bool,
    pub declaration_span: ByteSpan,
    pub shape_span: ByteSpan,
    pub capture: Option<LiteralCaptureSignature>,
    pub parameters: Vec<ParameterSignature>,
    pub return_type: TypeExpr,
    pub result_provenance: Option<ResultProvenanceClause>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralCaptureSignature {
    pub name: String,
    pub name_span: ByteSpan,
    pub element_type: TypeExpr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteralResolution {
    pub type_symbol: SymbolId,
    pub literal_declaration_span: ByteSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeSymbolKind {
    Alias,
    Struct,
    Enum,
    Interface,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariantSignature {
    pub name: String,
    pub name_span: ByteSpan,
    pub payload: Vec<ParameterSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructFieldSignature {
    pub name: String,
    pub name_span: ByteSpan,
    pub visibility: Visibility,
    pub is_accessible: bool,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociatedFunctionSignature {
    pub name: String,
    pub target_name: String,
    pub name_span: ByteSpan,
    pub visibility: Visibility,
    pub is_accessible: bool,
    pub signature: FunctionSignature,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodSignature {
    pub name: String,
    pub name_span: ByteSpan,
    pub visibility: Visibility,
    pub is_accessible: bool,
    pub impl_target_ty: Option<TypeExpr>,
    pub has_default_body: bool,
    pub receiver: MethodReceiver,
    pub signature: FunctionSignature,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropSignature {
    pub name_span: ByteSpan,
    pub target_name: String,
    pub binding: ParameterSignature,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterSignature {
    pub name: String,
    pub name_span: ByteSpan,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedSymbol {
    pub path: String,
    pub source: Option<SourceId>,
    pub access: Option<ImportAccess>,
    pub kind: ImportedSymbolKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportedSymbolKind {
    Namespace,
    UnloadedName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportSource {
    pub source: SourceId,
    pub access: ImportAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportAccess {
    Public,
    Nocter,
}

pub type ImportSourceMap = HashMap<ByteSpan, ImportSource>;
pub type PreludeSourceMap = HashMap<SourceId, ImportSource>;
