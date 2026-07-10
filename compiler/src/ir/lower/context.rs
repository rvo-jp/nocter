use crate::ast::CallExpr;
use crate::diagnostics::Diagnostic;
use crate::ir::{BoolLocation, CallTarget, I32Location, Type};
use crate::resolve::{ResolveOutput, SymbolKind};
use crate::source::SourceId;
use std::collections::HashMap;

pub(super) struct LoweringContext<'a> {
    function_name: String,
    return_type: Type,
    function_signatures: FunctionSignatures,
    call_resolution: Option<CallResolution<'a>>,
    i32_parameters: Vec<String>,
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
            i32_parameters: Vec::new(),
            locals: Vec::new(),
        }
    }

    pub(super) fn new(
        function_name: String,
        return_type: Type,
        function_signatures: FunctionSignatures,
        i32_parameters: Vec<String>,
    ) -> Self {
        Self {
            function_name,
            return_type,
            function_signatures,
            call_resolution: None,
            i32_parameters,
            locals: Vec::new(),
        }
    }

    pub(super) fn with_call_resolution(
        mut self,
        root_source: SourceId,
        resolved: &'a ResolveOutput,
    ) -> Self {
        self.call_resolution = Some(CallResolution {
            root_source,
            resolved,
        });
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

    pub(super) fn call_target(&self, call: &CallExpr, fallback_name: &str) -> CallTarget {
        let Some(resolution) = &self.call_resolution else {
            return CallTarget::same_file(fallback_name);
        };
        let Some(symbol) = resolution.resolved.symbol_for_call(call) else {
            return CallTarget::same_file(fallback_name);
        };

        match &symbol.kind {
            SymbolKind::Function(_) | SymbolKind::Type(_)
                if symbol.declaration_span.source != resolution.root_source =>
            {
                CallTarget::imported(symbol.declaration_span.source, symbol.name.clone())
            }
            SymbolKind::Function(_) | SymbolKind::Type(_) => {
                CallTarget::same_file(symbol.name.clone())
            }
            SymbolKind::Imported(_) => CallTarget::same_file(fallback_name),
        }
    }

    pub(super) fn next_i32_local_location(&self) -> Result<I32Location, Vec<Diagnostic>> {
        self.next_local_index().map(I32Location::Local)
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
                    .position(|parameter| parameter == name)
                    .map(I32Location::Parameter)
            })
    }

    pub(super) fn bool_location(&self, name: &str) -> Option<BoolLocation> {
        self.locals
            .iter()
            .find(|local| local.name == name && local.kind == LocalKind::Bool)
            .map(|local| BoolLocation::Local(local.index))
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
pub(super) struct FunctionSignatures {
    return_types: HashMap<CallTarget, Type>,
}

impl FunctionSignatures {
    #[cfg(test)]
    pub(super) fn new(return_types: HashMap<String, Type>) -> Self {
        Self {
            return_types: return_types
                .into_iter()
                .map(|(name, return_type)| (CallTarget::same_file(name), return_type))
                .collect(),
        }
    }

    pub(super) fn from_call_targets(return_types: HashMap<CallTarget, Type>) -> Self {
        Self { return_types }
    }

    pub(super) fn return_type(&self, target: &CallTarget) -> Option<&Type> {
        self.return_types.get(target)
    }
}

struct LocalBinding {
    name: String,
    kind: LocalKind,
    index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalKind {
    I32,
    Bool,
}

const MAX_SCALAR_LOCALS: usize = 7;
