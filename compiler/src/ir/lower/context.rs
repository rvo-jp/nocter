use crate::diagnostics::Diagnostic;
use crate::ir::{BoolLocation, I32Location};

pub(super) struct LoweringContext {
    i32_parameters: Vec<String>,
    locals: Vec<LocalBinding>,
}

impl LoweringContext {
    pub(super) fn empty() -> Self {
        Self {
            i32_parameters: Vec::new(),
            locals: Vec::new(),
        }
    }

    pub(super) fn new(i32_parameters: Vec<String>) -> Self {
        Self {
            i32_parameters,
            locals: Vec::new(),
        }
    }

    pub(super) fn next_i32_local_location(&self) -> Result<I32Location, Vec<Diagnostic>> {
        self.next_local_index().map(I32Location::Local)
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
