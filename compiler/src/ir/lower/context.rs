use crate::ast::CallExpr;
use crate::diagnostics::Diagnostic;
use crate::ir::{BoolLocation, CallTarget, I32Location, StrLocation, Type, UsizeLocation};
use crate::resolve::{ResolveOutput, SymbolKind};
use crate::source::{ByteSpan, SourceId};
use std::collections::HashMap;

pub(super) struct LoweringContext<'a> {
    function_name: String,
    return_type: Type,
    function_signatures: FunctionSignatures,
    call_resolution: Option<CallResolution<'a>>,
    function_names: FunctionNames,
    i32_parameters: Vec<Option<String>>,
    usize_parameters: Vec<Option<String>>,
    bool_parameters: Vec<Option<String>>,
    str_parameters: Vec<Option<String>>,
    locals: Vec<LocalBinding>,
}

impl<'a> LoweringContext<'a> {
    pub(super) fn empty(
        function_name: String,
        return_type: Type,
        function_signatures: FunctionSignatures,
    ) -> Self {
        Self {
            function_name,
            return_type,
            function_signatures,
            call_resolution: None,
            function_names: FunctionNames::default(),
            i32_parameters: Vec::new(),
            usize_parameters: Vec::new(),
            bool_parameters: Vec::new(),
            str_parameters: Vec::new(),
            locals: Vec::new(),
        }
    }

    pub(super) fn new(
        function_name: String,
        return_type: Type,
        function_signatures: FunctionSignatures,
        i32_parameters: Vec<Option<String>>,
        usize_parameters: Vec<Option<String>>,
        bool_parameters: Vec<Option<String>>,
        str_parameters: Vec<Option<String>>,
    ) -> Self {
        Self {
            function_name,
            return_type,
            function_signatures,
            call_resolution: None,
            function_names: FunctionNames::default(),
            i32_parameters,
            usize_parameters,
            bool_parameters,
            str_parameters,
            locals: Vec::new(),
        }
    }

    pub(super) fn with_call_resolution(
        mut self,
        root_source: SourceId,
        resolved: &'a ResolveOutput,
        function_names: FunctionNames,
    ) -> Self {
        self.call_resolution = Some(CallResolution {
            root_source,
            resolved,
        });
        self.function_names = function_names;
        self
    }

    pub(super) fn function_name(&self) -> &str {
        &self.function_name
    }

    pub(super) fn return_type(&self) -> &Type {
        &self.return_type
    }

    pub(super) fn call_return_type(&self, target: &CallTarget) -> Option<&Type> {
        self.function_signatures.return_type(target)
    }

    pub(super) fn call_parameter_types(&self, target: &CallTarget) -> Option<&[Type]> {
        self.function_signatures.parameter_types(target)
    }

    pub(super) fn call_target(&self, call: &CallExpr, fallback_name: &str) -> CallTarget {
        let Some(resolution) = &self.call_resolution else {
            return CallTarget::same_file(fallback_name);
        };
        let Some(symbol) = resolution.resolved.symbol_for_call(call) else {
            return CallTarget::same_file(fallback_name);
        };

        match &symbol.kind {
            SymbolKind::Function(_) | SymbolKind::Primitive(_) | SymbolKind::Type(_)
                if symbol.declaration_span.source != resolution.root_source =>
            {
                let target_name = self
                    .function_names
                    .name_for_declaration(symbol.declaration_span)
                    .unwrap_or(&symbol.name);
                CallTarget::imported(symbol.declaration_span.source, target_name.clone())
            }
            SymbolKind::Function(_) | SymbolKind::Primitive(_) | SymbolKind::Type(_) => {
                CallTarget::same_file(symbol.name.clone())
            }
            SymbolKind::Imported(_) => CallTarget::same_file(fallback_name),
        }
    }

    pub(super) fn primitive_name_for_call(&self, call: &CallExpr) -> Option<&str> {
        let resolution = self.call_resolution.as_ref()?;
        let symbol = resolution.resolved.symbol_for_call(call)?;
        match &symbol.kind {
            SymbolKind::Primitive(_) => Some(symbol.name.as_str()),
            SymbolKind::Function(_) | SymbolKind::Type(_) | SymbolKind::Imported(_) => None,
        }
    }

    pub(super) fn next_i32_local_location(&self) -> Result<I32Location, Vec<Diagnostic>> {
        self.next_local_index().map(I32Location::Local)
    }

    pub(super) fn next_usize_local_location(&self) -> Result<UsizeLocation, Vec<Diagnostic>> {
        self.next_local_index().map(UsizeLocation::Local)
    }

    pub(super) fn first_temporary_local_index(&self) -> Result<usize, Vec<Diagnostic>> {
        self.next_local_index()
    }

    pub(super) fn next_bool_local_location(&self) -> Result<BoolLocation, Vec<Diagnostic>> {
        self.next_local_index().map(BoolLocation::Local)
    }

    pub(super) fn define_i32_local(&mut self, name: String) {
        self.define_local(name, LocalKind::I32);
    }

    pub(super) fn define_usize_local(&mut self, name: String) {
        self.define_local(name, LocalKind::Usize);
    }

    pub(super) fn define_bool_local(&mut self, name: String) {
        self.define_local(name, LocalKind::Bool);
    }

    pub(super) fn i32_location(&self, name: &str) -> Option<I32Location> {
        self.locals
            .iter()
            .find(|local| local.name == name && local.kind == LocalKind::I32)
            .map(|local| I32Location::Local(local.index))
            .or_else(|| {
                self.i32_parameters
                    .iter()
                    .position(|parameter| parameter.as_deref() == Some(name))
                    .map(I32Location::Parameter)
            })
    }

    pub(super) fn usize_location(&self, name: &str) -> Option<UsizeLocation> {
        self.locals
            .iter()
            .find(|local| local.name == name && local.kind == LocalKind::Usize)
            .map(|local| UsizeLocation::Local(local.index))
            .or_else(|| {
                self.usize_parameters
                    .iter()
                    .position(|parameter| parameter.as_deref() == Some(name))
                    .map(UsizeLocation::Parameter)
            })
    }

    pub(super) fn bool_location(&self, name: &str) -> Option<BoolLocation> {
        self.locals
            .iter()
            .find(|local| local.name == name && local.kind == LocalKind::Bool)
            .map(|local| BoolLocation::Local(local.index))
            .or_else(|| {
                self.bool_parameters
                    .iter()
                    .position(|parameter| parameter.as_deref() == Some(name))
                    .map(BoolLocation::Parameter)
            })
    }

    pub(super) fn str_location(&self, name: &str) -> Option<StrLocation> {
        self.str_parameters
            .iter()
            .position(|parameter| parameter.as_deref() == Some(name))
            .map(StrLocation::Parameter)
    }

    fn next_local_index(&self) -> Result<usize, Vec<Diagnostic>> {
        if self.locals.len() >= MAX_SCALAR_LOCALS {
            return Err(vec![Diagnostic::error(
                "E8008",
                format!("IR v0 can only lower up to {MAX_SCALAR_LOCALS} local scalar bindings"),
            )]);
        }

        Ok(self.locals.len())
    }

    fn define_local(&mut self, name: String, kind: LocalKind) {
        self.locals.push(LocalBinding {
            name,
            kind,
            index: self.locals.len(),
        });
    }
}

struct CallResolution<'a> {
    root_source: SourceId,
    resolved: &'a ResolveOutput,
}

#[derive(Debug, Clone, Default)]
pub(super) struct FunctionNames {
    by_declaration_span: HashMap<ByteSpan, String>,
}

impl FunctionNames {
    pub(super) fn from_declarations(functions: Vec<(ByteSpan, String)>) -> Self {
        Self {
            by_declaration_span: functions.into_iter().collect(),
        }
    }

    fn name_for_declaration(&self, span: ByteSpan) -> Option<&String> {
        self.by_declaration_span.get(&span)
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct FunctionSignatures {
    signatures: HashMap<CallTarget, FunctionSignature>,
}

impl FunctionSignatures {
    #[cfg(test)]
    pub(super) fn new(return_types: HashMap<String, Type>) -> Self {
        Self {
            signatures: return_types
                .into_iter()
                .map(|(name, return_type)| {
                    (
                        CallTarget::same_file(name),
                        FunctionSignature {
                            return_type,
                            parameter_types: None,
                        },
                    )
                })
                .collect(),
        }
    }

    pub(super) fn from_call_targets(signatures: HashMap<CallTarget, FunctionSignature>) -> Self {
        Self { signatures }
    }

    pub(super) fn return_type(&self, target: &CallTarget) -> Option<&Type> {
        self.signatures
            .get(target)
            .map(|signature| &signature.return_type)
    }

    pub(super) fn parameter_types(&self, target: &CallTarget) -> Option<&[Type]> {
        self.signatures
            .get(target)
            .and_then(|signature| signature.parameter_types.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FunctionSignature {
    pub(super) return_type: Type,
    pub(super) parameter_types: Option<Vec<Type>>,
}

struct LocalBinding {
    name: String,
    kind: LocalKind,
    index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalKind {
    I32,
    Usize,
    Bool,
}

const MAX_SCALAR_LOCALS: usize = 7;
