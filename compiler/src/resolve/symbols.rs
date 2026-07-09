use crate::ast::{BindingKind, CallExpr, Expr, IdentifierExpr, TypeExpr, Visibility};
use crate::diagnostics::Diagnostic;
use crate::source::{ByteSpan, SourceId};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(u32);

impl SymbolId {
    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveOutput {
    pub symbols: SymbolTable,
    pub diagnostics: Vec<Diagnostic>,
    pub(super) identifier_targets: HashMap<ByteSpan, SymbolId>,
    pub(super) call_targets: HashMap<ByteSpan, SymbolId>,
    pub(super) local_symbols: Vec<LocalSymbol>,
    pub(super) local_identifier_targets: HashMap<ByteSpan, LocalSymbolId>,
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

    pub fn function_signature_for_call(&self, call: &CallExpr) -> Option<&FunctionSignature> {
        match self.symbol_for_call(call).map(|symbol| &symbol.kind) {
            Some(SymbolKind::Function(signature)) => Some(signature),
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
            Some(SymbolKind::Function(_) | SymbolKind::Imported(_)) | None => None,
        }
    }

    pub fn type_symbol_by_canonical_name(&self, canonical_name: &str) -> Option<&TypeSymbol> {
        self.symbols
            .symbols
            .iter()
            .find_map(|symbol| match &symbol.kind {
                SymbolKind::Type(type_symbol) if type_symbol.canonical_name == canonical_name => {
                    Some(type_symbol)
                }
                SymbolKind::Function(_) | SymbolKind::Type(_) | SymbolKind::Imported(_) => None,
            })
    }

    pub(super) fn new() -> Self {
        Self {
            symbols: SymbolTable::new(),
            diagnostics: Vec::new(),
            identifier_targets: HashMap::new(),
            call_targets: HashMap::new(),
            local_symbols: Vec::new(),
            local_identifier_targets: HashMap::new(),
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
    PatternPayload,
    CatchError,
    ForRange,
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

    pub(super) fn define(
        &mut self,
        name: String,
        name_span: ByteSpan,
        declaration_span: ByteSpan,
        kind: SymbolKind,
    ) -> Result<SymbolId, SymbolId> {
        if let Some(existing) = self.by_name.get(&name) {
            return Err(*existing);
        }

        let id = SymbolId(self.symbols.len() as u32);
        self.symbols.push(Symbol {
            id,
            name: name.clone(),
            name_span,
            declaration_span,
            kind,
        });
        self.by_name.insert(name, id);
        Ok(id)
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
    pub kind: SymbolKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Function(FunctionSignature),
    Type(TypeSymbol),
    Imported(ImportedSymbol),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSignature {
    pub parameters: Vec<ParameterSignature>,
    pub return_type: TypeExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeSymbol {
    pub kind: TypeSymbolKind,
    pub canonical_name: String,
    pub alias_target: Option<TypeExpr>,
    pub fields: Vec<StructFieldSignature>,
    pub variants: Vec<EnumVariantSignature>,
    pub associated_functions: Vec<AssociatedFunctionSignature>,
    pub methods: Vec<MethodSignature>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeSymbolKind {
    Alias,
    Struct,
    Enum,
    Trait,
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
    pub receiver: ParameterSignature,
    pub signature: FunctionSignature,
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
