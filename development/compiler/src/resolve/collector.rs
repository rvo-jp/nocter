use super::builtins::{is_builtin_type_name, is_reserved_type_declaration_name};
use super::conformance::interface_conformance;
use super::diagnostics::{
    builtin_name_reuse_diagnostic, duplicate_enum_variant_name_diagnostic,
    duplicate_enum_variant_payload_name_diagnostic, duplicate_generic_parameter_name_diagnostic,
    duplicate_interface_method_name_diagnostic, duplicate_parameter_name_diagnostic,
    duplicate_struct_field_name_diagnostic, duplicate_visible_name_diagnostic,
    invalid_associated_function_owner_diagnostic, prelude_name_collision_diagnostic,
    reserved_generic_parameter_name_reuse_diagnostic,
    reserved_type_declaration_name_reuse_diagnostic,
};
use super::signatures::{
    alias_type_symbol, associated_function_signature, declaration_target_type_name,
    destruct_signature, duplicate_destruct_diagnostics, duplicate_inherent_member_name_diagnostics,
    enum_type_symbol, function_signature, instance_method_signatures, interface_type_symbol,
    primitive_signature, struct_type_symbol, type_symbol_accepts_destructor,
    type_symbol_accepts_inherent_behavior,
};
use super::{Resolver, SymbolKind, TypeSymbol};
use crate::ast::{
    AstFile, ConformanceDecl, ConformanceMember, DestructDecl, EnumDecl, EnumVariant, FunctionDecl,
    GenericParamList, InstanceDecl, InterfaceDecl, Item, MethodReceiver, Parameter, PrimitiveDecl,
    StructDecl,
};
use crate::diagnostics::Diagnostic;
use crate::source::{ByteSpan, SourceMap};
use std::collections::HashMap;

impl Resolver<'_> {
    pub(super) fn collect_top_level_symbols(&mut self, ast: &AstFile) {
        self.collect_synthetic_prelude_symbols(ast);

        for item in &ast.items {
            match item {
                Item::Import(item) => self.collect_import_namespace_symbol(item),
                Item::FromImport(item) => self.collect_imported_symbols(item),
                Item::Function(function) => {
                    self.output
                        .diagnostics
                        .extend(generic_parameter_name_diagnostics(
                            self.sources,
                            &format!("function `{}`", function.name),
                            &function.generics,
                        ));
                    self.output
                        .diagnostics
                        .extend(duplicate_parameter_name_diagnostics(
                            self.sources,
                            &format!("function `{}`", function.name),
                            &function.parameters.parameters,
                        ));
                    if function.owner.is_none() {
                        self.collect_function_symbol(function);
                    }
                }
                Item::Test(_) => {}
                Item::Primitive(primitive) => {
                    self.output
                        .diagnostics
                        .extend(generic_parameter_name_diagnostics(
                            self.sources,
                            &format!("primitive `{}`", primitive.name),
                            &primitive.generics,
                        ));
                    self.output
                        .diagnostics
                        .extend(duplicate_parameter_name_diagnostics(
                            self.sources,
                            &format!("primitive `{}`", primitive.name),
                            &primitive.parameters.parameters,
                        ));
                    self.collect_primitive_symbol(primitive);
                }
                Item::TypeAlias(alias) => {
                    self.output
                        .diagnostics
                        .extend(generic_parameter_name_diagnostics(
                            self.sources,
                            &format!("type alias `{}`", alias.name),
                            &alias.generics,
                        ));
                    self.collect_type_symbol(
                        alias.name.clone(),
                        alias.name_span,
                        alias.span,
                        alias_type_symbol(alias),
                    );
                }
                Item::Struct(struct_) => {
                    self.output
                        .diagnostics
                        .extend(duplicate_struct_field_name_diagnostics(
                            self.sources,
                            struct_,
                        ));
                    self.output
                        .diagnostics
                        .extend(generic_parameter_name_diagnostics(
                            self.sources,
                            &format!("struct `{}`", struct_.name),
                            &struct_.generics,
                        ));
                    self.collect_type_symbol(
                        struct_.name.clone(),
                        struct_.name_span,
                        struct_.span,
                        struct_type_symbol(struct_, struct_.is_copy, &struct_.fields),
                    );
                }
                Item::Enum(enum_) => {
                    self.output
                        .diagnostics
                        .extend(duplicate_enum_decl_diagnostics(self.sources, enum_));
                    self.output
                        .diagnostics
                        .extend(generic_parameter_name_diagnostics(
                            self.sources,
                            &format!("enum `{}`", enum_.name),
                            &enum_.generics,
                        ));
                    self.collect_type_symbol(
                        enum_.name.clone(),
                        enum_.name_span,
                        enum_.span,
                        enum_type_symbol(enum_),
                    );
                }
                Item::Interface(interface) => {
                    self.output
                        .diagnostics
                        .extend(generic_parameter_name_diagnostics(
                            self.sources,
                            &format!("interface `{}`", interface.name),
                            &interface.generics,
                        ));
                    self.output
                        .diagnostics
                        .extend(duplicate_interface_method_name_diagnostics(
                            self.sources,
                            interface,
                        ));
                    for method in &interface.methods {
                        let subject =
                            format!("interface method `{}.{}`", interface.name, method.name);
                        self.output
                            .diagnostics
                            .extend(method_generic_parameter_name_diagnostics(
                                self.sources,
                                &subject,
                                &interface.generics,
                                &method.generics,
                            ));
                        self.output.diagnostics.extend(
                            duplicate_method_parameter_name_diagnostics(
                                self.sources,
                                &subject,
                                &method.receiver,
                                &method.parameters.parameters,
                            ),
                        );
                    }
                    self.collect_type_symbol(
                        interface.name.clone(),
                        interface.name_span,
                        interface.span,
                        interface_type_symbol(interface),
                    );
                }
                Item::Instance(instance) => {
                    self.output
                        .diagnostics
                        .extend(generic_parameter_name_diagnostics(
                            self.sources,
                            "instance block",
                            &instance.generics,
                        ));
                    for method in &instance.methods {
                        let subject = format!("method `{}`", method.name);
                        self.output
                            .diagnostics
                            .extend(method_generic_parameter_name_diagnostics(
                                self.sources,
                                &subject,
                                &instance.generics,
                                &method.generics,
                            ));
                        self.output.diagnostics.extend(
                            duplicate_method_parameter_name_diagnostics(
                                self.sources,
                                &subject,
                                &method.receiver,
                                &method.parameters.parameters,
                            ),
                        );
                    }
                }
                Item::Destruct(destruct) => {
                    self.output
                        .diagnostics
                        .extend(generic_parameter_name_diagnostics(
                            self.sources,
                            "destruct declaration",
                            &destruct.generics,
                        ));
                }
                Item::Conformance(conformance) => {
                    self.output
                        .diagnostics
                        .extend(generic_parameter_name_diagnostics(
                            self.sources,
                            "conform block",
                            &conformance.generics,
                        ));
                    for member in &conformance.members {
                        if let ConformanceMember::Method(method) = member {
                            let subject = format!("conformance method `{}`", method.name);
                            self.output.diagnostics.extend(
                                method_generic_parameter_name_diagnostics(
                                    self.sources,
                                    &subject,
                                    &conformance.generics,
                                    &method.generics,
                                ),
                            );
                            self.output.diagnostics.extend(
                                duplicate_method_parameter_name_diagnostics(
                                    self.sources,
                                    &subject,
                                    &method.receiver,
                                    &method.parameters.parameters,
                                ),
                            );
                        }
                    }
                }
                Item::Construct(construct) => {
                    for (_, function) in construct.functions() {
                        self.output
                            .diagnostics
                            .extend(generic_parameter_name_diagnostics(
                                self.sources,
                                &format!("construction function `{}`", function.name),
                                &function.generics,
                            ));
                        self.output
                            .diagnostics
                            .extend(duplicate_parameter_name_diagnostics(
                                self.sources,
                                &format!("construction function `{}`", function.name),
                                &function.parameters.parameters,
                            ));
                    }
                }
                Item::Coerce(_) => {}
            }
        }

        for item in &ast.items {
            match item {
                Item::Function(function) if function.owner.is_some() => {
                    self.collect_top_level_associated_function(function);
                }
                Item::Construct(construct) => {
                    for (_, function) in construct.functions() {
                        self.collect_top_level_associated_function(function);
                    }
                }
                Item::Coerce(_) => {}
                _ => {}
            }
        }

        for item in &ast.items {
            match item {
                Item::Instance(instance) => self.collect_instance_members(instance),
                Item::Destruct(destruct) => self.collect_destruct(destruct),
                Item::Conformance(conformance) => self.collect_conformance(conformance),
                _ => {}
            }
        }

        self.collect_literal_definitions(ast);
        self.collect_construction_surfaces(ast);
        self.collect_coercion_surfaces(ast);
    }

    pub(super) fn define_symbol(
        &mut self,
        name: String,
        name_span: ByteSpan,
        declaration_span: ByteSpan,
        kind: SymbolKind,
    ) {
        if symbol_kind_introduces_value_name(&kind) && is_builtin_type_name(&name) {
            self.output.diagnostics.push(builtin_name_reuse_diagnostic(
                self.sources,
                &name,
                name_span,
            ));
            return;
        }

        match self
            .output
            .symbols
            .define(name.clone(), name_span, declaration_span, kind)
        {
            Ok(id) => {
                if self.collecting_synthetic_prelude
                    && let Some(symbol) = self.output.symbols.get(id)
                {
                    self.synthetic_prelude_symbol_spans.insert(symbol.name_span);
                }
            }
            Err(first_id) => {
                if let Some(first) = self.output.symbols.get(first_id) {
                    let diagnostic =
                        self.duplicate_visible_symbol_diagnostic(&name, first.name_span, name_span);
                    self.output.diagnostics.push(diagnostic);
                }
            }
        }
    }

    pub(super) fn duplicate_visible_symbol_diagnostic(
        &self,
        name: &str,
        first_span: ByteSpan,
        duplicate_span: ByteSpan,
    ) -> Diagnostic {
        if self.synthetic_prelude_symbol_spans.contains(&first_span) {
            prelude_name_collision_diagnostic(self.sources, name, first_span, duplicate_span)
        } else {
            duplicate_visible_name_diagnostic(self.sources, name, first_span, duplicate_span)
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
            SymbolKind::Primitive(primitive_signature(primitive)),
        );
    }

    pub(super) fn collect_top_level_associated_function(&mut self, function: &FunctionDecl) {
        let Some(owner) = &function.owner else {
            return;
        };
        let Some(symbol_id) = self.output.symbols.by_name.get(&owner.name).copied() else {
            self.output
                .diagnostics
                .push(invalid_associated_function_owner_diagnostic(
                    self.sources,
                    &owner.name,
                    owner.name_span,
                    "must name a type declared in this module",
                    None,
                ));
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

        if symbol.declaration_span.source != function.name_span.source {
            self.output
                .diagnostics
                .push(invalid_associated_function_owner_diagnostic(
                    self.sources,
                    &owner.name,
                    owner.name_span,
                    "must be defined in the same module as the associated function",
                    Some(symbol.declaration_span),
                ));
            return;
        }

        let symbol_declaration_span = symbol.declaration_span;
        let SymbolKind::Type(type_symbol) = &mut symbol.kind else {
            self.output
                .diagnostics
                .push(invalid_associated_function_owner_diagnostic(
                    self.sources,
                    &owner.name,
                    owner.name_span,
                    "must name a type declared in this module",
                    Some(symbol_declaration_span),
                ));
            return;
        };

        if !type_symbol_accepts_inherent_behavior(type_symbol) {
            self.output
                .diagnostics
                .push(invalid_associated_function_owner_diagnostic(
                    self.sources,
                    &owner.name,
                    owner.name_span,
                    "must name a nominal type that accepts inherent members",
                    Some(symbol_declaration_span),
                ));
            return;
        }

        let diagnostics = duplicate_associated_function_name_diagnostics(
            self.sources,
            &owner.name,
            type_symbol,
            function,
        );
        type_symbol
            .associated_functions
            .push(associated_function_signature(function));
        self.output.diagnostics.extend(diagnostics);
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
                .push(reserved_type_declaration_name_reuse_diagnostic(
                    self.sources,
                    &name,
                    name_span,
                ));
            return;
        }

        self.define_symbol(name, name_span, declaration_span, SymbolKind::Type(symbol));
    }

    fn collect_instance_members(&mut self, instance: &InstanceDecl) {
        let Some(target_name) = declaration_target_type_name(&instance.target_ty) else {
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

            if !type_symbol_accepts_inherent_behavior(type_symbol) {
                return;
            }

            let mut methods = instance_method_signatures(instance).collect::<Vec<_>>();
            let diagnostics = duplicate_inherent_member_name_diagnostics(
                self.sources,
                target_name,
                type_symbol,
                instance,
            );
            type_symbol.methods.append(&mut methods);
            diagnostics
        };
        self.output.diagnostics.extend(diagnostics);
    }

    fn collect_destruct(&mut self, destruct: &DestructDecl) {
        let Some(target_name) = declaration_target_type_name(&destruct.target_ty) else {
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
        let SymbolKind::Type(type_symbol) = &mut symbol.kind else {
            return;
        };
        if !type_symbol_accepts_destructor(type_symbol) {
            return;
        }
        let diagnostics =
            duplicate_destruct_diagnostics(self.sources, target_name, type_symbol, destruct);
        if type_symbol.destructor.is_none() {
            type_symbol.destructor = destruct_signature(destruct);
        }
        self.output.diagnostics.extend(diagnostics);
    }

    fn collect_conformance(&mut self, conformance: &ConformanceDecl) {
        let Some(target_name) = declaration_target_type_name(&conformance.target_ty) else {
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
        let SymbolKind::Type(type_symbol) = &mut symbol.kind else {
            return;
        };
        if type_symbol_accepts_inherent_behavior(type_symbol) {
            type_symbol
                .interface_conformances
                .push(interface_conformance(conformance));
        }
    }
}

fn duplicate_struct_field_name_diagnostics(
    sources: &SourceMap,
    struct_: &StructDecl,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = HashMap::new();

    for field in &struct_.fields {
        if let Some(first_span) = seen.get(field.name.as_str()).copied() {
            diagnostics.push(duplicate_struct_field_name_diagnostic(
                sources,
                &struct_.name,
                &field.name,
                first_span,
                field.name_span,
            ));
        } else {
            seen.insert(field.name.as_str(), field.name_span);
        }
    }

    diagnostics
}

fn generic_parameter_name_diagnostics(
    sources: &SourceMap,
    subject: &str,
    generics: &GenericParamList,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = HashMap::new();

    for parameter in &generics.parameters {
        if is_reserved_type_declaration_name(&parameter.name) {
            diagnostics.push(reserved_generic_parameter_name_reuse_diagnostic(
                sources,
                &parameter.name,
                parameter.name_span,
            ));
        } else if let Some(first_span) = seen.get(parameter.name.as_str()).copied() {
            diagnostics.push(duplicate_generic_parameter_name_diagnostic(
                sources,
                subject,
                &parameter.name,
                first_span,
                parameter.name_span,
            ));
        } else {
            seen.insert(parameter.name.as_str(), parameter.name_span);
        }
    }

    diagnostics
}

fn method_generic_parameter_name_diagnostics(
    sources: &SourceMap,
    subject: &str,
    owner_generics: &GenericParamList,
    method_generics: &GenericParamList,
) -> Vec<Diagnostic> {
    let mut diagnostics = generic_parameter_name_diagnostics(sources, subject, method_generics);
    let owner_parameters = owner_generics
        .parameters
        .iter()
        .map(|parameter| (parameter.name.as_str(), parameter.name_span))
        .collect::<HashMap<_, _>>();

    for parameter in &method_generics.parameters {
        if let Some(first_span) = owner_parameters.get(parameter.name.as_str()).copied() {
            diagnostics.push(duplicate_generic_parameter_name_diagnostic(
                sources,
                subject,
                &parameter.name,
                first_span,
                parameter.name_span,
            ));
        }
    }

    diagnostics
}

fn symbol_kind_introduces_value_name(kind: &SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Function(_) | SymbolKind::Primitive(_) | SymbolKind::Imported(_)
    )
}

fn duplicate_parameter_name_diagnostics(
    sources: &SourceMap,
    subject: &str,
    parameters: &[Parameter],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = HashMap::new();

    for parameter in parameters {
        if let Some(first_span) = seen.get(parameter.name.as_str()).copied() {
            diagnostics.push(duplicate_parameter_name_diagnostic(
                sources,
                subject,
                &parameter.name,
                first_span,
                parameter.name_span,
            ));
        } else {
            seen.insert(parameter.name.as_str(), parameter.name_span);
        }
    }

    diagnostics
}

fn duplicate_interface_method_name_diagnostics(
    sources: &SourceMap,
    interface: &InterfaceDecl,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = HashMap::new();

    for method in &interface.methods {
        if let Some(first_span) = seen.get(method.name.as_str()).copied() {
            diagnostics.push(duplicate_interface_method_name_diagnostic(
                sources,
                &interface.name,
                &method.name,
                first_span,
                method.name_span,
            ));
        } else {
            seen.insert(method.name.as_str(), method.name_span);
        }
    }

    diagnostics
}

fn duplicate_method_parameter_name_diagnostics(
    sources: &SourceMap,
    subject: &str,
    receiver: &MethodReceiver,
    parameters: &[Parameter],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = HashMap::from([(receiver.name.as_str(), receiver.name_span)]);

    for parameter in parameters {
        if let Some(first_span) = seen.get(parameter.name.as_str()).copied() {
            diagnostics.push(duplicate_parameter_name_diagnostic(
                sources,
                subject,
                &parameter.name,
                first_span,
                parameter.name_span,
            ));
        } else {
            seen.insert(parameter.name.as_str(), parameter.name_span);
        }
    }

    diagnostics
}

fn duplicate_enum_decl_diagnostics(sources: &SourceMap, enum_: &EnumDecl) -> Vec<Diagnostic> {
    let mut diagnostics = duplicate_enum_variant_name_diagnostics(sources, enum_);
    for variant in &enum_.variants {
        diagnostics.extend(duplicate_enum_variant_payload_name_diagnostics(
            sources, enum_, variant,
        ));
    }
    diagnostics
}

fn duplicate_enum_variant_name_diagnostics(
    sources: &SourceMap,
    enum_: &EnumDecl,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = HashMap::new();

    for variant in &enum_.variants {
        if let Some(first_span) = seen.get(variant.name.as_str()).copied() {
            diagnostics.push(duplicate_enum_variant_name_diagnostic(
                sources,
                &enum_.name,
                &variant.name,
                first_span,
                variant.name_span,
            ));
        } else {
            seen.insert(variant.name.as_str(), variant.name_span);
        }
    }

    diagnostics
}

fn duplicate_enum_variant_payload_name_diagnostics(
    sources: &SourceMap,
    enum_: &EnumDecl,
    variant: &EnumVariant,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = HashMap::new();

    for payload in &variant.payload {
        if let Some(first_span) = seen.get(payload.name.as_str()).copied() {
            diagnostics.push(duplicate_enum_variant_payload_name_diagnostic(
                sources,
                &enum_.name,
                &variant.name,
                &payload.name,
                first_span,
                payload.name_span,
            ));
        } else {
            seen.insert(payload.name.as_str(), payload.name_span);
        }
    }

    diagnostics
}

fn duplicate_associated_function_name_diagnostics(
    sources: &crate::source::SourceMap,
    target_name: &str,
    type_symbol: &TypeSymbol,
    function: &FunctionDecl,
) -> Vec<crate::diagnostics::Diagnostic> {
    let mut diagnostics = Vec::new();
    let name = function.member_name.as_str();
    if let Some(first_span) = type_symbol
        .variants
        .iter()
        .find(|variant| variant.name == name)
        .map(|variant| variant.name_span)
        .or_else(|| {
            type_symbol
                .associated_functions
                .iter()
                .find(|associated| associated.name == name)
                .map(|associated| associated.name_span)
        })
        .or_else(|| {
            type_symbol
                .methods
                .iter()
                .find(|method| method.name == name)
                .map(|method| method.name_span)
        })
    {
        diagnostics.push(
            super::diagnostics::duplicate_inherent_member_name_diagnostic(
                sources,
                target_name,
                name,
                first_span,
                function.member_name_span,
            ),
        );
    }
    diagnostics
}
