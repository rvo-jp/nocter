//! Import resolution, visibility, and name lookup.

use crate::ast::{
    AstFile, Block, CallExpr, Expr, FromImportItem, FunctionDecl, IdentifierExpr, Item, Parameter,
    Stmt, TypeExpr,
};
use crate::diagnostics::{Diagnostic, DiagnosticNote};
use crate::source::{ByteSpan, SourceId, SourceMap};
use std::collections::HashMap;
use std::path::PathBuf;

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
    identifier_targets: HashMap<ByteSpan, SymbolId>,
    call_targets: HashMap<ByteSpan, SymbolId>,
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

    pub fn function_signature_for_call(&self, call: &CallExpr) -> Option<&FunctionSignature> {
        match self.symbol_for_call(call).map(|symbol| &symbol.kind) {
            Some(SymbolKind::Function(signature)) => Some(signature),
            Some(SymbolKind::Imported(_)) | None => None,
        }
    }

    fn new() -> Self {
        Self {
            symbols: SymbolTable::new(),
            diagnostics: Vec::new(),
            identifier_targets: HashMap::new(),
            call_targets: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolTable {
    symbols: Vec<Symbol>,
    by_name: HashMap<String, SymbolId>,
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

    fn define(
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
    Imported(ImportedSymbol),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSignature {
    pub parameters: Vec<ParameterSignature>,
    pub return_type: TypeExpr,
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

pub fn resolve(sources: &SourceMap, ast: &AstFile) -> ResolveOutput {
    resolve_compile_unit(sources, ast, std::slice::from_ref(ast))
}

pub fn resolve_compile_unit(
    sources: &SourceMap,
    root: &AstFile,
    files: &[AstFile],
) -> ResolveOutput {
    let module_index = ModuleIndex::new(sources, files);
    let mut resolver = Resolver {
        sources,
        module_index,
        output: ResolveOutput::new(),
    };

    resolver.collect_top_level_symbols(root);
    resolver.resolve_callable_bodies(root);
    resolver.output
}

struct Resolver<'a> {
    sources: &'a SourceMap,
    module_index: ModuleIndex<'a>,
    output: ResolveOutput,
}

impl Resolver<'_> {
    fn collect_top_level_symbols(&mut self, ast: &AstFile) {
        for item in &ast.items {
            match item {
                Item::FromImport(item) => self.collect_imported_symbols(item),
                Item::Function(function) => self.collect_function_symbol(function),
                Item::Use(_) | Item::Program(_) => {}
            }
        }
    }

    fn collect_imported_symbols(&mut self, item: &FromImportItem) {
        if is_relative_module_path(&item.path.value) {
            self.collect_relative_imported_symbols(item);
            return;
        }

        for name in &item.names {
            self.define_symbol(
                name.name.clone(),
                name.span,
                item.span,
                SymbolKind::Imported(ImportedSymbol {
                    path: item.path.value.clone(),
                }),
            );
        }
    }

    fn collect_relative_imported_symbols(&mut self, item: &FromImportItem) {
        let Some(imported_ast) = self.module_index.relative_import_ast(self.sources, item) else {
            for name in &item.names {
                self.output.diagnostics.push(unloaded_import_diagnostic(
                    self.sources,
                    &item.path.value,
                    name.span,
                ));
            }
            return;
        };

        for name in &item.names {
            match find_function(imported_ast, &name.name) {
                Some(function) => self.define_symbol(
                    name.name.clone(),
                    name.span,
                    function.name_span,
                    SymbolKind::Function(function_signature(function)),
                ),
                None => {
                    self.output.diagnostics.push(missing_import_diagnostic(
                        self.sources,
                        &name.name,
                        &item.path.value,
                        name.span,
                    ));
                }
            }
        }
    }

    fn collect_function_symbol(&mut self, function: &FunctionDecl) {
        self.define_symbol(
            function.name.clone(),
            function.name_span,
            function.name_span,
            SymbolKind::Function(function_signature(function)),
        );
    }

    fn define_symbol(
        &mut self,
        name: String,
        name_span: ByteSpan,
        declaration_span: ByteSpan,
        kind: SymbolKind,
    ) {
        if let Err(first_id) =
            self.output
                .symbols
                .define(name.clone(), name_span, declaration_span, kind)
            && let Some(first) = self.output.symbols.get(first_id)
        {
            self.output
                .diagnostics
                .push(duplicate_visible_name_diagnostic(
                    self.sources,
                    &name,
                    first.name_span,
                    name_span,
                ));
        }
    }

    fn resolve_callable_bodies(&mut self, ast: &AstFile) {
        for item in &ast.items {
            match item {
                Item::Program(program) => {
                    let mut scope = Scope::new();
                    self.resolve_block(&program.body, &mut scope);
                }
                Item::Function(function) => {
                    let mut scope = Scope::new();
                    self.define_parameters(&function.parameters.parameters, &mut scope);
                    self.resolve_block(&function.body, &mut scope);
                }
                Item::Use(_) | Item::FromImport(_) => {}
            }
        }
    }

    fn define_parameters(&mut self, parameters: &[Parameter], scope: &mut Scope) {
        for parameter in parameters {
            self.define_local_name(parameter.name.clone(), parameter.name_span, scope);
        }
    }

    fn resolve_block(&mut self, block: &Block, scope: &mut Scope) {
        for statement in &block.statements {
            self.resolve_statement(statement, scope);
        }
    }

    fn resolve_statement(&mut self, statement: &Stmt, scope: &mut Scope) {
        match statement {
            Stmt::Return(statement) => {
                if let Some(expression) = &statement.expression {
                    self.resolve_expression(expression, scope);
                }
            }
            Stmt::Binding(statement) => {
                self.resolve_expression(&statement.initializer, scope);
                if let Some(else_block) = &statement.else_block {
                    let mut else_scope = scope.clone();
                    self.resolve_block(else_block, &mut else_scope);
                }
                self.define_local_name(statement.name.clone(), statement.name_span, scope);
            }
            Stmt::Try(statement) => self.resolve_expression(&statement.expression, scope),
            Stmt::TryCatch(statement) => {
                self.resolve_expression(&statement.expression, scope);
                let mut catch_scope = scope.clone();
                self.define_local_name(
                    statement.error_name.clone(),
                    statement.error_span,
                    &mut catch_scope,
                );
                self.resolve_block(&statement.catch_block, &mut catch_scope);
            }
            Stmt::Expression(statement) => self.resolve_expression(&statement.expression, scope),
        }
    }

    fn resolve_expression(&mut self, expression: &Expr, scope: &mut Scope) {
        match expression {
            Expr::Identifier(expression) => self.resolve_identifier(expression, scope),
            Expr::Try(expression) => self.resolve_expression(&expression.expression, scope),
            Expr::TryCatch(expression) => {
                self.resolve_expression(&expression.expression, scope);
                let mut catch_scope = scope.clone();
                self.define_local_name(
                    expression.error_name.clone(),
                    expression.error_span,
                    &mut catch_scope,
                );
                self.resolve_block(&expression.catch_block, &mut catch_scope);
            }
            Expr::Call(expression) => {
                self.resolve_expression(&expression.callee, scope);
                if let Expr::Identifier(callee) = expression.callee.as_ref()
                    && let Some(symbol_id) = self.resolve_top_level_name(callee, scope)
                {
                    self.output.call_targets.insert(expression.span, symbol_id);
                }
                for argument in &expression.arguments {
                    self.resolve_expression(argument, scope);
                }
            }
            Expr::Member(expression) => self.resolve_expression(&expression.object, scope),
            Expr::Group(expression) => self.resolve_expression(&expression.expression, scope),
            Expr::OptionalDefault(expression) => {
                self.resolve_expression(&expression.value, scope);
                self.resolve_expression(&expression.default, scope);
            }
            Expr::IntegerLiteral(_) | Expr::StringLiteral(_) | Expr::NoneLiteral(_) => {}
        }
    }

    fn resolve_identifier(&mut self, identifier: &IdentifierExpr, scope: &Scope) {
        if let Some(symbol_id) = self.resolve_top_level_name(identifier, scope) {
            self.output
                .identifier_targets
                .insert(identifier.span, symbol_id);
        }
    }

    fn resolve_top_level_name(
        &self,
        identifier: &IdentifierExpr,
        scope: &Scope,
    ) -> Option<SymbolId> {
        if scope.contains(&identifier.name) {
            return None;
        }

        self.output
            .symbols
            .symbol_by_name(&identifier.name)
            .map(|symbol| symbol.id)
    }

    fn define_local_name(&mut self, name: String, span: ByteSpan, scope: &mut Scope) {
        if is_builtin_type_name(&name) {
            self.output
                .diagnostics
                .push(builtin_name_reuse_diagnostic(self.sources, &name, span));
        } else if let Some(first_span) = scope.get(&name) {
            self.output
                .diagnostics
                .push(duplicate_visible_name_diagnostic(
                    self.sources,
                    &name,
                    first_span,
                    span,
                ));
        } else if let Some(symbol) = self.output.symbols.symbol_by_name(&name) {
            self.output
                .diagnostics
                .push(duplicate_visible_name_diagnostic(
                    self.sources,
                    &name,
                    symbol.name_span,
                    span,
                ));
        }

        scope.define(name, span);
    }
}

struct ModuleIndex<'a> {
    by_absolute_path: HashMap<PathBuf, &'a AstFile>,
}

impl<'a> ModuleIndex<'a> {
    fn new(sources: &SourceMap, files: &'a [AstFile]) -> Self {
        let mut by_absolute_path = HashMap::new();

        for ast in files {
            if let Some(path) = source_absolute_path(sources, ast.span.source) {
                by_absolute_path.insert(path, ast);
            }
        }

        Self { by_absolute_path }
    }

    fn relative_import_ast(
        &self,
        sources: &SourceMap,
        item: &FromImportItem,
    ) -> Option<&'a AstFile> {
        let source_file = sources.get(item.path.span.source)?;
        let source_path = source_file.absolute_path()?;
        let source_dir = source_path.parent()?;
        let import_path = source_dir.join(format!("{}.nct", item.path.value));
        let canonical = import_path.canonicalize().ok()?;
        self.by_absolute_path.get(&canonical).copied()
    }
}

fn source_absolute_path(sources: &SourceMap, source: SourceId) -> Option<PathBuf> {
    sources.get(source)?.absolute_path().cloned()
}

fn is_relative_module_path(path: &str) -> bool {
    path.starts_with("./") || path.starts_with("../")
}

fn find_function<'a>(ast: &'a AstFile, name: &str) -> Option<&'a FunctionDecl> {
    ast.items.iter().find_map(|item| match item {
        Item::Function(function) if function.name == name => Some(function),
        _ => None,
    })
}

fn function_signature(function: &FunctionDecl) -> FunctionSignature {
    FunctionSignature {
        parameters: function
            .parameters
            .parameters
            .iter()
            .map(parameter_signature)
            .collect(),
        return_type: function.return_type.clone(),
    }
}

#[derive(Debug, Clone, Default)]
struct Scope {
    locals: HashMap<String, ByteSpan>,
}

impl Scope {
    fn new() -> Self {
        Self::default()
    }

    fn contains(&self, name: &str) -> bool {
        self.locals.contains_key(name)
    }

    fn get(&self, name: &str) -> Option<ByteSpan> {
        self.locals.get(name).copied()
    }

    fn define(&mut self, name: String, span: ByteSpan) {
        self.locals.entry(name).or_insert(span);
    }
}

fn parameter_signature(parameter: &Parameter) -> ParameterSignature {
    ParameterSignature {
        name: parameter.name.clone(),
        name_span: parameter.name_span,
        ty: parameter.ty.clone(),
    }
}

fn is_builtin_type_name(name: &str) -> bool {
    matches!(
        name,
        "bool" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "usize" | "isize"
    )
}

fn duplicate_visible_name_diagnostic(
    sources: &SourceMap,
    name: &str,
    first_span: ByteSpan,
    duplicate_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error("E0400", format!("name `{name}` is already visible"));
    diagnostic.primary_span = sources.span_to_json(duplicate_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first visible declaration is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help =
        Some("choose a distinct name; Nocter v0 does not allow shadowing".to_string());
    diagnostic
}

fn builtin_name_reuse_diagnostic(sources: &SourceMap, name: &str, span: ByteSpan) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0401",
        format!("built-in type name `{name}` cannot be reused as a binding"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some("choose a binding name that is not a built-in type name".to_string());
    diagnostic
}

fn unloaded_import_diagnostic(
    sources: &SourceMap,
    import_path: &str,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0411",
        format!("relative import `{import_path}` was not loaded before name resolution"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some("load relative imports before running name resolution".to_string());
    diagnostic
}

fn missing_import_diagnostic(
    sources: &SourceMap,
    name: &str,
    import_path: &str,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0411",
        format!("import `{import_path}` does not export function `{name}` in v0"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "define a top-level `func` with that name in the imported file, or import an existing function"
            .to_string(),
    );
    diagnostic
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;

    fn resolve_text(text: &str) -> ResolveOutput {
        let mut sources = SourceMap::new();
        let source = sources.add_source("app.nct", None, text);
        let lexed = lex(&sources, source);
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        let parsed = parse(&sources, source, &lexed.tokens);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        resolve(&sources, &parsed.ast.unwrap())
    }

    #[test]
    fn collects_function_symbols() {
        let output = resolve_text(
            r#"program(): i32 {
    return answer()
}

func answer(): i32 {
    return 1
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let symbol = output.symbols.symbol_by_name("answer").unwrap();
        assert_eq!(symbol.name, "answer");
        assert!(matches!(symbol.kind, SymbolKind::Function(_)));
    }

    #[test]
    fn diagnoses_duplicate_function_names() {
        let output = resolve_text(
            r#"program(): i32 {
    return 0
}

func answer(): i32 {
    return 1
}

func answer(): i32 {
    return 2
}
"#,
        );

        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, "E0400");
    }

    #[test]
    fn resolves_direct_function_calls() {
        let output = resolve_text(
            r#"program(): i32 {
    return answer()
}

func answer(): i32 {
    return 1
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        assert_eq!(output.call_targets.len(), 1);
        let symbol = output
            .call_targets
            .values()
            .next()
            .and_then(|id| output.symbols.get(*id))
            .unwrap();
        assert_eq!(symbol.name, "answer");
    }

    #[test]
    fn imported_calls_are_not_function_signatures_yet() {
        let output = resolve_text(
            r#"from std/io import print

program(): i32 {
    try print("hello") catch error {
        return 1
    }

    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let symbol = output
            .call_targets
            .values()
            .next()
            .and_then(|id| output.symbols.get(*id))
            .unwrap();
        assert_eq!(symbol.name, "print");
        assert!(matches!(symbol.kind, SymbolKind::Imported(_)));
    }

    #[test]
    fn diagnoses_local_shadowing_top_level_function() {
        let output = resolve_text(
            r#"program(): i32 {
    let answer = 0
    return answer
}

func answer(): i32 {
    return 1
}
"#,
        );

        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, "E0400");
    }
}
