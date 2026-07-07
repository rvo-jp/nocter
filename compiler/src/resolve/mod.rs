//! Import resolution, visibility, and name lookup.

use crate::ast::{
    AstFile, Block, CallExpr, EnumVariant, Expr, FromImportItem, FunctionDecl, IdentifierExpr,
    ImplDecl, ImplMember, ImportItem, Item, MethodDecl, Parameter, PrimitiveDecl, Stmt,
    StructField, TypeExpr, UseItem, Visibility,
};
use crate::diagnostics::{Diagnostic, DiagnosticNote};
use crate::source::{ByteSpan, SourceId, SourceMap};
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

pub fn resolve(sources: &SourceMap, ast: &AstFile) -> ResolveOutput {
    resolve_compile_unit(
        sources,
        ast,
        std::slice::from_ref(ast),
        &ImportSourceMap::new(),
    )
}

pub fn resolve_compile_unit(
    sources: &SourceMap,
    root: &AstFile,
    files: &[AstFile],
    import_sources: &ImportSourceMap,
) -> ResolveOutput {
    let module_index = ModuleIndex::new(sources, files);
    let mut resolver = Resolver {
        sources,
        module_index,
        import_sources,
        output: ResolveOutput::new(),
    };

    resolver.collect_top_level_symbols(root);
    resolver.resolve_callable_bodies(root);
    resolver.output
}

struct Resolver<'a> {
    sources: &'a SourceMap,
    module_index: ModuleIndex<'a>,
    import_sources: &'a ImportSourceMap,
    output: ResolveOutput,
}

impl Resolver<'_> {
    fn collect_top_level_symbols(&mut self, ast: &AstFile) {
        for item in &ast.items {
            match item {
                Item::Use(item) => self.collect_use_symbols(item),
                Item::Import(item) => self.collect_import_namespace_symbol(item),
                Item::FromImport(item) => self.collect_imported_symbols(item),
                Item::Function(function) => self.collect_function_symbol(function),
                Item::Primitive(primitive) => self.collect_primitive_symbol(primitive),
                Item::TypeAlias(alias) => self.collect_type_symbol(
                    alias.name.clone(),
                    alias.name_span,
                    alias.span,
                    alias_type_symbol(alias.name.clone(), alias.target.clone()),
                ),
                Item::Struct(struct_) => self.collect_type_symbol(
                    struct_.name.clone(),
                    struct_.name_span,
                    struct_.span,
                    struct_type_symbol(struct_.name.clone(), &struct_.fields),
                ),
                Item::Enum(enum_) => self.collect_type_symbol(
                    enum_.name.clone(),
                    enum_.name_span,
                    enum_.span,
                    enum_type_symbol(enum_.name.clone(), &enum_.variants),
                ),
                Item::Trait(trait_) => self.collect_type_symbol(
                    trait_.name.clone(),
                    trait_.name_span,
                    trait_.span,
                    nominal_type_symbol(trait_.name.clone(), TypeSymbolKind::Trait),
                ),
                Item::Impl(_) | Item::Program(_) => {}
            }
        }

        for item in &ast.items {
            if let Item::Impl(impl_) = item {
                self.collect_inherent_impl_members(impl_);
            }
        }
    }

    fn collect_use_symbols(&mut self, item: &UseItem) {
        let Some((imported_ast, import_source)) = self
            .module_index
            .import_ast_for_span(item.path.span, self.import_sources)
        else {
            return;
        };

        self.collect_public_exports(imported_ast, import_source.access, &item.path.value);
    }

    fn collect_import_namespace_symbol(&mut self, item: &ImportItem) {
        if self
            .module_index
            .import_ast_for_span(item.path.span, self.import_sources)
            .is_none()
            && is_relative_module_path(&item.path.value)
        {
            self.output.diagnostics.push(unloaded_import_diagnostic(
                self.sources,
                &item.path.value,
                item.alias.span,
            ));
            return;
        }

        self.define_symbol(
            item.alias.name.clone(),
            item.alias.span,
            item.path.span,
            SymbolKind::Imported(ImportedSymbol {
                path: item.path.value.clone(),
            }),
        );
    }

    fn collect_imported_symbols(&mut self, item: &FromImportItem) {
        if let Some((imported_ast, import_source)) =
            self.module_index.import_ast(item, self.import_sources)
        {
            self.collect_loaded_imported_symbols(item, imported_ast, import_source.access);
            return;
        }

        if is_relative_module_path(&item.path.value) {
            self.report_unloaded_imported_symbols(item);
            return;
        }

        for name in &item.names {
            self.define_symbol(
                name.local_name().to_string(),
                name.local_span(),
                item.span,
                SymbolKind::Imported(ImportedSymbol {
                    path: item.path.value.clone(),
                }),
            );
        }
    }

    fn collect_loaded_imported_symbols(
        &mut self,
        item: &FromImportItem,
        imported_ast: &AstFile,
        access: ImportAccess,
    ) {
        for name in &item.names {
            match self.find_importable_symbol(imported_ast, &name.name) {
                Some(imported) if imported.is_visible_to(access) => {
                    let imported = filter_importable_symbol_for_access(imported, access);
                    let imported = qualify_imported_symbol(imported, &item.path.value, &name.name);
                    self.define_symbol(
                        name.local_name().to_string(),
                        name.local_span(),
                        imported.declaration_span,
                        imported.kind,
                    );
                }
                Some(imported) => {
                    self.output.diagnostics.push(restricted_import_diagnostic(
                        self.sources,
                        &name.name,
                        &item.path.value,
                        imported.visibility,
                        name.name_span,
                        imported.declaration_span,
                    ));
                }
                None => {
                    self.output.diagnostics.push(missing_import_diagnostic(
                        self.sources,
                        &name.name,
                        &item.path.value,
                        name.name_span,
                    ));
                }
            }
        }
    }

    fn collect_public_exports(&mut self, ast: &AstFile, access: ImportAccess, module_path: &str) {
        for item in &ast.items {
            match item {
                Item::Function(function) => {
                    let imported = ImportableSymbol {
                        declaration_span: function.name_span,
                        visibility: function.visibility,
                        kind: SymbolKind::Function(function_signature(function)),
                    };
                    self.collect_public_export(
                        function.name.clone(),
                        function.name_span,
                        imported,
                        access,
                    );
                }
                Item::Primitive(primitive) => {
                    let imported = ImportableSymbol {
                        declaration_span: primitive.name_span,
                        visibility: primitive.visibility,
                        kind: SymbolKind::Function(primitive_signature(primitive)),
                    };
                    self.collect_public_export(
                        primitive.name.clone(),
                        primitive.name_span,
                        imported,
                        access,
                    );
                }
                Item::TypeAlias(alias) => {
                    let imported = type_importable_symbol(
                        alias.span,
                        alias.visibility,
                        alias_type_symbol(alias.name.clone(), alias.target.clone()),
                    );
                    let imported = qualify_imported_symbol(imported, module_path, &alias.name);
                    self.collect_public_export(
                        alias.name.clone(),
                        alias.name_span,
                        imported,
                        access,
                    );
                }
                Item::Struct(struct_) => {
                    let mut symbol = struct_type_symbol(struct_.name.clone(), &struct_.fields);
                    attach_inherent_impl_members_to_symbol(&mut symbol, ast, &struct_.name);
                    let imported = type_importable_symbol(struct_.span, struct_.visibility, symbol);
                    let imported = filter_importable_symbol_for_access(imported, access);
                    let imported = qualify_imported_symbol(imported, module_path, &struct_.name);
                    self.collect_public_export(
                        struct_.name.clone(),
                        struct_.name_span,
                        imported,
                        access,
                    );
                }
                Item::Enum(enum_) => {
                    let mut symbol = enum_type_symbol(enum_.name.clone(), &enum_.variants);
                    attach_inherent_impl_members_to_symbol(&mut symbol, ast, &enum_.name);
                    let imported = type_importable_symbol(enum_.span, enum_.visibility, symbol);
                    let imported = filter_importable_symbol_for_access(imported, access);
                    let imported = qualify_imported_symbol(imported, module_path, &enum_.name);
                    self.collect_public_export(
                        enum_.name.clone(),
                        enum_.name_span,
                        imported,
                        access,
                    );
                }
                Item::Trait(trait_) => {
                    let imported = type_importable_symbol(
                        trait_.span,
                        trait_.visibility,
                        nominal_type_symbol(trait_.name.clone(), TypeSymbolKind::Trait),
                    );
                    let imported = qualify_imported_symbol(imported, module_path, &trait_.name);
                    self.collect_public_export(
                        trait_.name.clone(),
                        trait_.name_span,
                        imported,
                        access,
                    );
                }
                Item::FromImport(item) if item.visibility == Visibility::Public => {
                    self.collect_public_reexports(item, access);
                }
                Item::Use(_)
                | Item::Import(_)
                | Item::FromImport(_)
                | Item::Impl(_)
                | Item::Program(_) => {}
            }
        }
    }

    fn collect_public_export(
        &mut self,
        name: String,
        name_span: ByteSpan,
        imported: ImportableSymbol,
        access: ImportAccess,
    ) {
        if imported.is_visible_to(access) {
            self.define_symbol(name, name_span, imported.declaration_span, imported.kind);
        }
    }

    fn collect_public_reexports(&mut self, item: &FromImportItem, access: ImportAccess) {
        let Some((imported_ast, _)) = self.module_index.import_ast(item, self.import_sources)
        else {
            return;
        };

        for name in &item.names {
            match self.find_importable_symbol(imported_ast, &name.name) {
                Some(imported)
                    if imported.visibility == Visibility::Public
                        && imported.is_visible_to(access) =>
                {
                    let imported = filter_importable_symbol_for_access(imported, access);
                    let imported = qualify_imported_symbol(imported, &item.path.value, &name.name);
                    self.define_symbol(
                        name.local_name().to_string(),
                        name.local_span(),
                        imported.declaration_span,
                        imported.kind,
                    );
                }
                Some(imported) => {
                    self.output.diagnostics.push(restricted_import_diagnostic(
                        self.sources,
                        &name.name,
                        &item.path.value,
                        imported.visibility,
                        name.name_span,
                        imported.declaration_span,
                    ));
                }
                None => {
                    self.output.diagnostics.push(missing_import_diagnostic(
                        self.sources,
                        &name.name,
                        &item.path.value,
                        name.name_span,
                    ));
                }
            }
        }
    }

    fn report_unloaded_imported_symbols(&mut self, item: &FromImportItem) {
        for name in &item.names {
            self.output.diagnostics.push(unloaded_import_diagnostic(
                self.sources,
                &item.path.value,
                name.local_span(),
            ));
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

    fn collect_primitive_symbol(&mut self, primitive: &PrimitiveDecl) {
        self.define_symbol(
            primitive.name.clone(),
            primitive.name_span,
            primitive.name_span,
            SymbolKind::Function(primitive_signature(primitive)),
        );
    }

    fn collect_type_symbol(
        &mut self,
        name: String,
        name_span: ByteSpan,
        declaration_span: ByteSpan,
        symbol: TypeSymbol,
    ) {
        if is_reserved_type_declaration_name(&name) {
            self.output
                .diagnostics
                .push(builtin_type_declaration_name_reuse_diagnostic(
                    self.sources,
                    &name,
                    name_span,
                ));
            return;
        }

        self.define_symbol(name, name_span, declaration_span, SymbolKind::Type(symbol));
    }

    fn collect_inherent_impl_members(&mut self, impl_: &ImplDecl) {
        if impl_.trait_ty.is_some() {
            return;
        }

        let Some(target_name) = impl_target_type_name(&impl_.target_ty) else {
            return;
        };
        let Some(symbol_id) = self.output.symbols.by_name.get(target_name).copied() else {
            return;
        };
        let Some(symbol) = self
            .output
            .symbols
            .symbols
            .get_mut(symbol_id.raw() as usize)
        else {
            return;
        };
        let diagnostics = {
            let SymbolKind::Type(type_symbol) = &mut symbol.kind else {
                return;
            };

            if !matches!(
                type_symbol.kind,
                TypeSymbolKind::Struct | TypeSymbolKind::Enum
            ) {
                return;
            }

            let mut associated_functions =
                associated_function_signatures(impl_).collect::<Vec<_>>();
            let mut methods = method_signatures(impl_).collect::<Vec<_>>();
            let diagnostics = duplicate_inherent_member_name_diagnostics(
                self.sources,
                target_name,
                type_symbol,
                impl_,
            );
            type_symbol
                .associated_functions
                .append(&mut associated_functions);
            type_symbol.methods.append(&mut methods);
            diagnostics
        };
        self.output.diagnostics.extend(diagnostics);
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
                Item::Impl(impl_) => self.resolve_impl_bodies(impl_),
                Item::Use(_)
                | Item::Import(_)
                | Item::FromImport(_)
                | Item::Primitive(_)
                | Item::TypeAlias(_)
                | Item::Struct(_)
                | Item::Enum(_)
                | Item::Trait(_) => {}
            }
        }
    }

    fn resolve_impl_bodies(&mut self, impl_: &ImplDecl) {
        for member in &impl_.members {
            match member {
                ImplMember::Function(function) => {
                    let mut scope = Scope::new();
                    self.define_parameters(&function.parameters.parameters, &mut scope);
                    self.resolve_block(&function.body, &mut scope);
                }
                ImplMember::Method(method) => {
                    let Some(body) = &method.body else {
                        continue;
                    };
                    let mut scope = Scope::new();
                    self.define_local_name(
                        method.receiver.name.clone(),
                        method.receiver.name_span,
                        &mut scope,
                    );
                    self.define_parameters(&method.parameters.parameters, &mut scope);
                    self.resolve_block(body, &mut scope);
                }
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
            Stmt::Fail(statement) => self.resolve_expression(&statement.expression, scope),
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
            Stmt::If(statement) => {
                self.resolve_expression(&statement.condition, scope);
                let mut then_scope = scope.clone();
                self.resolve_block(&statement.then_block, &mut then_scope);
                if let Some(else_block) = &statement.else_block {
                    let mut else_scope = scope.clone();
                    self.resolve_block(else_block, &mut else_scope);
                }
            }
            Stmt::IfIs(statement) => {
                self.resolve_expression(&statement.expression, scope);
                let mut then_scope = scope.clone();
                if let Some(payload) = &statement.payload {
                    self.define_local_name(payload.name.clone(), payload.span, &mut then_scope);
                }
                self.resolve_block(&statement.then_block, &mut then_scope);
                if let Some(else_block) = &statement.else_block {
                    let mut else_scope = scope.clone();
                    self.resolve_block(else_block, &mut else_scope);
                }
            }
            Stmt::IfLet(statement) => {
                self.resolve_expression(&statement.initializer, scope);
                let mut then_scope = scope.clone();
                self.define_local_name(
                    statement.name.clone(),
                    statement.name_span,
                    &mut then_scope,
                );
                self.resolve_block(&statement.then_block, &mut then_scope);
                if let Some(else_block) = &statement.else_block {
                    let mut else_scope = scope.clone();
                    self.resolve_block(else_block, &mut else_scope);
                }
            }
            Stmt::Switch(statement) => {
                self.resolve_expression(&statement.expression, scope);
                for arm in &statement.arms {
                    let mut arm_scope = scope.clone();
                    if let Some(payload) = &arm.payload {
                        self.define_local_name(payload.name.clone(), payload.span, &mut arm_scope);
                    }
                    self.resolve_block(&arm.body, &mut arm_scope);
                }
                if let Some(else_arm) = &statement.else_arm {
                    let mut else_scope = scope.clone();
                    self.resolve_block(&else_arm.body, &mut else_scope);
                }
            }
            Stmt::While(statement) => {
                self.resolve_expression(&statement.condition, scope);
                let mut body_scope = scope.clone();
                self.resolve_block(&statement.body, &mut body_scope);
            }
            Stmt::WhileLet(statement) => {
                self.resolve_expression(&statement.initializer, scope);
                let mut body_scope = scope.clone();
                self.define_local_name(
                    statement.name.clone(),
                    statement.name_span,
                    &mut body_scope,
                );
                self.resolve_block(&statement.body, &mut body_scope);
            }
            Stmt::ForRange(statement) => {
                self.resolve_expression(&statement.start, scope);
                self.resolve_expression(&statement.end, scope);
                let mut body_scope = scope.clone();
                self.define_local_name(
                    statement.name.clone(),
                    statement.name_span,
                    &mut body_scope,
                );
                self.resolve_block(&statement.body, &mut body_scope);
            }
            Stmt::Loop(statement) => {
                let mut body_scope = scope.clone();
                self.resolve_block(&statement.body, &mut body_scope);
            }
            Stmt::Break(_) | Stmt::Continue(_) => {}
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
            Expr::Binary(expression) => {
                self.resolve_expression(&expression.left, scope);
                self.resolve_expression(&expression.right, scope);
            }
            Expr::Unary(expression) => self.resolve_expression(&expression.operand, scope),
            Expr::TypeConversion(expression) => {
                self.resolve_expression(&expression.expression, scope)
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
            Expr::Index(expression) => {
                self.resolve_expression(&expression.object, scope);
                self.resolve_expression(&expression.index, scope);
            }
            Expr::ArrayLiteral(expression) => {
                for element in &expression.elements {
                    self.resolve_expression(element, scope);
                }
            }
            Expr::StructLiteral(expression) => {
                for field in &expression.fields {
                    self.resolve_expression(&field.value, scope);
                }
            }
            Expr::Group(expression) => self.resolve_expression(&expression.expression, scope),
            Expr::OptionalDefault(expression) => {
                self.resolve_expression(&expression.value, scope);
                self.resolve_expression(&expression.default, scope);
            }
            Expr::IntegerLiteral(_)
            | Expr::StringLiteral(_)
            | Expr::BoolLiteral(_)
            | Expr::NoneLiteral(_) => {}
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

    fn find_importable_symbol(&self, ast: &AstFile, name: &str) -> Option<ImportableSymbol> {
        direct_importable_symbol(ast, name).or_else(|| self.find_reexported_symbol(ast, name))
    }

    fn find_reexported_symbol(&self, ast: &AstFile, name: &str) -> Option<ImportableSymbol> {
        ast.items.iter().find_map(|item| {
            let Item::FromImport(item) = item else {
                return None;
            };
            if item.visibility != Visibility::Public {
                return None;
            }

            let reexport = item
                .names
                .iter()
                .find(|imported| imported.local_name() == name)?;
            let (imported_ast, _) = self.module_index.import_ast(item, self.import_sources)?;
            let imported = direct_importable_symbol(imported_ast, &reexport.name)?;
            (imported.visibility == Visibility::Public)
                .then(|| qualify_imported_symbol(imported, &item.path.value, &reexport.name))
        })
    }
}

struct ModuleIndex<'a> {
    by_source: HashMap<SourceId, &'a AstFile>,
}

impl<'a> ModuleIndex<'a> {
    fn new(_sources: &SourceMap, files: &'a [AstFile]) -> Self {
        let mut by_source = HashMap::new();

        for ast in files {
            by_source.insert(ast.span.source, ast);
        }

        Self { by_source }
    }

    fn import_ast(
        &self,
        item: &FromImportItem,
        import_sources: &ImportSourceMap,
    ) -> Option<(&'a AstFile, ImportSource)> {
        self.import_ast_for_span(item.path.span, import_sources)
    }

    fn import_ast_for_span(
        &self,
        path_span: ByteSpan,
        import_sources: &ImportSourceMap,
    ) -> Option<(&'a AstFile, ImportSource)> {
        let import_source = *import_sources.get(&path_span)?;
        self.by_source
            .get(&import_source.source)
            .copied()
            .map(|ast| (ast, import_source))
    }
}

fn is_relative_module_path(path: &str) -> bool {
    path.starts_with("./") || path.starts_with("../")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportableSymbol {
    declaration_span: ByteSpan,
    visibility: Visibility,
    kind: SymbolKind,
}

impl ImportableSymbol {
    fn is_visible_to(&self, access: ImportAccess) -> bool {
        match self.visibility {
            Visibility::Public => true,
            Visibility::Nocter => access == ImportAccess::Nocter,
            Visibility::Private => false,
        }
    }
}

fn direct_importable_symbol(ast: &AstFile, name: &str) -> Option<ImportableSymbol> {
    ast.items.iter().find_map(|item| match item {
        Item::Function(function) if function.name == name => Some(ImportableSymbol {
            declaration_span: function.name_span,
            visibility: function.visibility,
            kind: SymbolKind::Function(function_signature(function)),
        }),
        Item::Primitive(primitive) if primitive.name == name => Some(ImportableSymbol {
            declaration_span: primitive.name_span,
            visibility: primitive.visibility,
            kind: SymbolKind::Function(primitive_signature(primitive)),
        }),
        Item::TypeAlias(alias) if alias.name == name => Some(type_importable_symbol(
            alias.span,
            alias.visibility,
            alias_type_symbol(alias.name.clone(), alias.target.clone()),
        )),
        Item::Struct(struct_) if struct_.name == name => {
            let mut symbol = struct_type_symbol(struct_.name.clone(), &struct_.fields);
            attach_inherent_impl_members_to_symbol(&mut symbol, ast, &struct_.name);
            Some(type_importable_symbol(
                struct_.span,
                struct_.visibility,
                symbol,
            ))
        }
        Item::Enum(enum_) if enum_.name == name => {
            let mut symbol = enum_type_symbol(enum_.name.clone(), &enum_.variants);
            attach_inherent_impl_members_to_symbol(&mut symbol, ast, &enum_.name);
            Some(type_importable_symbol(enum_.span, enum_.visibility, symbol))
        }
        Item::Trait(trait_) if trait_.name == name => Some(type_importable_symbol(
            trait_.span,
            trait_.visibility,
            nominal_type_symbol(trait_.name.clone(), TypeSymbolKind::Trait),
        )),
        _ => None,
    })
}

fn attach_inherent_impl_members_to_symbol(symbol: &mut TypeSymbol, ast: &AstFile, type_name: &str) {
    if !matches!(symbol.kind, TypeSymbolKind::Struct | TypeSymbolKind::Enum) {
        return;
    }

    for item in &ast.items {
        let Item::Impl(impl_) = item else {
            continue;
        };
        if impl_.trait_ty.is_some() || impl_target_type_name(&impl_.target_ty) != Some(type_name) {
            continue;
        }

        symbol
            .associated_functions
            .extend(associated_function_signatures(impl_));
        symbol.methods.extend(method_signatures(impl_));
    }
}

fn associated_function_signatures(
    impl_: &ImplDecl,
) -> impl Iterator<Item = AssociatedFunctionSignature> + '_ {
    impl_.members.iter().filter_map(|member| match member {
        ImplMember::Function(function) => Some(AssociatedFunctionSignature {
            name: function.name.clone(),
            name_span: function.name_span,
            visibility: function.visibility,
            is_accessible: true,
            signature: function_signature(function),
        }),
        ImplMember::Method(_) => None,
    })
}

fn method_signatures(impl_: &ImplDecl) -> impl Iterator<Item = MethodSignature> + '_ {
    impl_.members.iter().filter_map(|member| match member {
        ImplMember::Method(method) => Some(method_signature(method)),
        ImplMember::Function(_) => None,
    })
}

fn method_signature(method: &MethodDecl) -> MethodSignature {
    MethodSignature {
        name: method.name.clone(),
        name_span: method.name_span,
        visibility: method.visibility,
        is_accessible: true,
        receiver: parameter_signature(&method.receiver),
        signature: callable_signature(&method.parameters.parameters, method.return_type.clone()),
    }
}

fn duplicate_inherent_member_name_diagnostics(
    sources: &SourceMap,
    target_name: &str,
    type_symbol: &TypeSymbol,
    impl_: &ImplDecl,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = HashMap::<&str, ByteSpan>::new();
    for function in &type_symbol.associated_functions {
        seen.entry(function.name.as_str())
            .or_insert(function.name_span);
    }
    for method in &type_symbol.methods {
        seen.entry(method.name.as_str()).or_insert(method.name_span);
    }

    for member in &impl_.members {
        let (name, span) = match member {
            ImplMember::Function(function) => (function.name.as_str(), function.name_span),
            ImplMember::Method(method) => (method.name.as_str(), method.name_span),
        };
        match seen.entry(name) {
            std::collections::hash_map::Entry::Occupied(first) => {
                diagnostics.push(duplicate_inherent_member_name_diagnostic(
                    sources,
                    target_name,
                    name,
                    *first.get(),
                    span,
                ));
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(span);
            }
        }
    }

    diagnostics
}

fn impl_target_type_name(ty: &TypeExpr) -> Option<&str> {
    let TypeExpr::Reference(reference) = ty else {
        return None;
    };

    Some(&reference.name)
}

fn function_signature(function: &FunctionDecl) -> FunctionSignature {
    callable_signature(
        &function.parameters.parameters,
        function.return_type.clone(),
    )
}

fn primitive_signature(primitive: &PrimitiveDecl) -> FunctionSignature {
    callable_signature(
        &primitive.parameters.parameters,
        primitive.return_type.clone(),
    )
}

fn callable_signature(parameters: &[Parameter], return_type: TypeExpr) -> FunctionSignature {
    FunctionSignature {
        parameters: parameters.iter().map(parameter_signature).collect(),
        return_type,
    }
}

fn type_importable_symbol(
    declaration_span: ByteSpan,
    visibility: Visibility,
    symbol: TypeSymbol,
) -> ImportableSymbol {
    ImportableSymbol {
        declaration_span,
        visibility,
        kind: SymbolKind::Type(symbol),
    }
}

fn filter_importable_symbol_for_access(
    mut imported: ImportableSymbol,
    access: ImportAccess,
) -> ImportableSymbol {
    if let SymbolKind::Type(symbol) = &mut imported.kind {
        for field in &mut symbol.fields {
            field.is_accessible =
                field.is_accessible && visibility_is_visible_to(field.visibility, access);
        }

        for function in &mut symbol.associated_functions {
            function.is_accessible =
                function.is_accessible && visibility_is_visible_to(function.visibility, access);
        }

        for method in &mut symbol.methods {
            method.is_accessible =
                method.is_accessible && visibility_is_visible_to(method.visibility, access);
        }
    }

    imported
}

fn visibility_is_visible_to(visibility: Visibility, access: ImportAccess) -> bool {
    match visibility {
        Visibility::Public => true,
        Visibility::Nocter => access == ImportAccess::Nocter,
        Visibility::Private => false,
    }
}

fn alias_type_symbol(canonical_name: String, alias_target: TypeExpr) -> TypeSymbol {
    TypeSymbol {
        kind: TypeSymbolKind::Alias,
        canonical_name,
        alias_target: Some(alias_target),
        fields: Vec::new(),
        variants: Vec::new(),
        associated_functions: Vec::new(),
        methods: Vec::new(),
    }
}

fn nominal_type_symbol(canonical_name: String, kind: TypeSymbolKind) -> TypeSymbol {
    TypeSymbol {
        kind,
        canonical_name,
        alias_target: None,
        fields: Vec::new(),
        variants: Vec::new(),
        associated_functions: Vec::new(),
        methods: Vec::new(),
    }
}

fn struct_type_symbol(canonical_name: String, fields: &[StructField]) -> TypeSymbol {
    TypeSymbol {
        kind: TypeSymbolKind::Struct,
        canonical_name,
        alias_target: None,
        fields: fields
            .iter()
            .map(|field| StructFieldSignature {
                name: field.name.clone(),
                name_span: field.name_span,
                visibility: field.visibility,
                is_accessible: true,
                ty: field.ty.clone(),
            })
            .collect(),
        variants: Vec::new(),
        associated_functions: Vec::new(),
        methods: Vec::new(),
    }
}

fn enum_type_symbol(canonical_name: String, variants: &[EnumVariant]) -> TypeSymbol {
    TypeSymbol {
        kind: TypeSymbolKind::Enum,
        canonical_name,
        alias_target: None,
        fields: Vec::new(),
        variants: variants
            .iter()
            .map(|variant| EnumVariantSignature {
                name: variant.name.clone(),
                name_span: variant.name_span,
                payload: variant.payload.iter().map(parameter_signature).collect(),
            })
            .collect(),
        associated_functions: Vec::new(),
        methods: Vec::new(),
    }
}

fn qualify_imported_symbol(
    mut imported: ImportableSymbol,
    import_path: &str,
    imported_name: &str,
) -> ImportableSymbol {
    if let SymbolKind::Type(symbol) = &mut imported.kind
        && symbol.canonical_name == imported_name
    {
        symbol.canonical_name = format!("{import_path}.{imported_name}");
    }

    imported
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
        "bool"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "usize"
            | "isize"
            | "str"
    )
}

fn is_reserved_type_declaration_name(name: &str) -> bool {
    is_builtin_type_name(name) || name == "error"
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

fn builtin_type_declaration_name_reuse_diagnostic(
    sources: &SourceMap,
    name: &str,
    span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0401",
        format!("built-in type name `{name}` cannot be reused as a type declaration"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some("choose a type name that is not a built-in type name".to_string());
    diagnostic
}

fn duplicate_inherent_member_name_diagnostic(
    sources: &SourceMap,
    target_name: &str,
    member_name: &str,
    first_span: ByteSpan,
    duplicate_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0413",
        format!("type `{target_name}` already has an inherent member named `{member_name}`"),
    );
    diagnostic.primary_span = sources.span_to_json(duplicate_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(first_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "first inherent member with this name is here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help =
        Some("choose a distinct member name; Nocter v0 does not support overloads".to_string());
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
        format!("import `{import_path}` does not export `{name}` in v0"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "define a public top-level `func`, `primitive`, `type`, `struct`, or `enum` with that name in the imported file"
            .to_string(),
    );
    diagnostic
}

fn restricted_import_diagnostic(
    sources: &SourceMap,
    name: &str,
    import_path: &str,
    visibility: Visibility,
    import_span: ByteSpan,
    declaration_span: ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0412",
        format!(
            "import `{import_path}` cannot access {visibility} name `{name}`",
            visibility = visibility_description(visibility),
        ),
    );
    diagnostic.primary_span = sources.span_to_json(import_span).ok().map(Box::new);
    if let Ok(span) = sources.span_to_json(declaration_span) {
        diagnostic.notes.push(DiagnosticNote {
            message: "name is declared here".to_string(),
            span: Some(span),
        });
    }
    diagnostic.help = Some(
        match visibility {
            Visibility::Private => "mark the declaration `pub` if it is part of the module API",
            Visibility::Nocter => {
                "`pub(nocter)` names are importable only from files inside the active Nocter home"
            }
            Visibility::Public => {
                "public names should be importable; this diagnostic is unexpected"
            }
        }
        .to_string(),
    );
    diagnostic
}

fn visibility_description(visibility: Visibility) -> &'static str {
    match visibility {
        Visibility::Private => "private",
        Visibility::Public => "public",
        Visibility::Nocter => "`pub(nocter)`",
    }
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
    fn collects_primitive_and_type_symbols() {
        let output = resolve_text(
            r#"pub primitive addr<T>(pointer: *T): usize

	pub type Bytes = [u8]

pub struct File {
    pub fd: i32
    name: str
}

pub enum IOError {
    denied
}

program(): i32 {
    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        assert!(matches!(
            &output.symbols.symbol_by_name("addr").unwrap().kind,
            SymbolKind::Function(_)
        ));
        assert!(matches!(
            &output.symbols.symbol_by_name("Bytes").unwrap().kind,
            SymbolKind::Type(TypeSymbol {
                kind: TypeSymbolKind::Alias,
                ..
            })
        ));
        let file_symbol = output.symbols.symbol_by_name("File").unwrap();
        let SymbolKind::Type(TypeSymbol {
            kind: TypeSymbolKind::Struct,
            fields,
            ..
        }) = &file_symbol.kind
        else {
            panic!("expected struct symbol");
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "fd");
        assert_eq!(fields[0].visibility, Visibility::Public);
        assert_eq!(fields[1].name, "name");
        assert_eq!(fields[1].visibility, Visibility::Private);
        assert!(matches!(
            &output.symbols.symbol_by_name("IOError").unwrap().kind,
            SymbolKind::Type(TypeSymbol {
                kind: TypeSymbolKind::Enum,
                ..
            })
        ));
    }

    #[test]
    fn collects_associated_function_symbols() {
        let output = resolve_text(
            r#"struct Point {
    x: i32
}

impl Point {
    pub func origin(): Point {
        return Point{ x: 0 }
    }

    method (point: Self).x_value(): i32 {
        return point.x
    }
}

program(): i32 {
    return Point.origin().x
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let point_symbol = output.symbols.symbol_by_name("Point").unwrap();
        let SymbolKind::Type(TypeSymbol {
            associated_functions,
            methods,
            ..
        }) = &point_symbol.kind
        else {
            panic!("expected type symbol");
        };

        assert_eq!(associated_functions.len(), 1);
        assert_eq!(associated_functions[0].name, "origin");
        assert_eq!(associated_functions[0].visibility, Visibility::Public);
        assert_eq!(associated_functions[0].signature.parameters.len(), 0);
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "x_value");
        assert_eq!(methods[0].receiver.name, "point");
        assert_eq!(methods[0].signature.parameters.len(), 0);
    }

    #[test]
    fn diagnoses_builtin_error_type_name_reuse() {
        let output = resolve_text(
            r#"type error = i32

program(): i32 {
    return 0
}
"#,
        );

        assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
        assert_eq!(output.diagnostics[0].code, "E0401");
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
    fn diagnoses_duplicate_inherent_associated_function_names_in_same_impl() {
        let output = resolve_text(
            r#"struct File {
    fd: i32
}

impl File {
    func open(): i32 {
        return 0
    }

    func open(path: str): i32 {
        return 1
    }
}

program(): i32 {
    return 0
}
"#,
        );

        assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
        assert_eq!(output.diagnostics[0].code, "E0413");
    }

    #[test]
    fn diagnoses_duplicate_inherent_method_names_across_impls() {
        let output = resolve_text(
            r#"struct File {
    fd: i32
}

impl File {
    method (file: Self).name(): i32 {
        return file.fd
    }
}

impl File {
    method (file: Self).name(value: i32): i32 {
        return value
    }
}

program(): i32 {
    return 0
}
"#,
        );

        assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
        assert_eq!(output.diagnostics[0].code, "E0413");
    }

    #[test]
    fn diagnoses_duplicate_inherent_function_and_method_name() {
        let output = resolve_text(
            r#"struct File {
    fd: i32
}

impl File {
    func open(): i32 {
        return 0
    }

    method (file: Self).open(): i32 {
        return file.fd
    }
}

program(): i32 {
    return 0
}
"#,
        );

        assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
        assert_eq!(output.diagnostics[0].code, "E0413");
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
    print("hello") catch error {
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
    fn imports_from_alias_under_local_name() {
        let output = resolve_text(
            r#"from std/io import print as write

program(): i32 {
    write("hello") catch error {
        return 1
    }

    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        assert!(output.symbols.symbol_by_name("print").is_none());
        let symbol = output.symbols.symbol_by_name("write").unwrap();
        assert_eq!(symbol.name, "write");
        assert!(matches!(symbol.kind, SymbolKind::Imported(_)));
    }

    #[test]
    fn imports_namespace_alias_as_visible_name() {
        let output = resolve_text(
            r#"import std/io as io

program(): i32 {
    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let symbol = output.symbols.symbol_by_name("io").unwrap();
        assert_eq!(symbol.name, "io");
        assert!(matches!(symbol.kind, SymbolKind::Imported(_)));
    }

    #[test]
    fn collects_trait_symbols() {
        let output = resolve_text(
            r#"pub trait Writer {
    method (writer: &+Self).write(text: str): void!
}

program(): i32 {
    return 0
}
"#,
        );

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        let symbol = output.symbols.symbol_by_name("Writer").unwrap();
        assert!(matches!(
            &symbol.kind,
            SymbolKind::Type(TypeSymbol {
                kind: TypeSymbolKind::Trait,
                ..
            })
        ));
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
