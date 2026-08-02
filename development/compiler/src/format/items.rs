use super::Formatter;
use crate::ast::{
    AstFile, DropDecl, EnumDecl, EnumVariant, FromImportItem, FunctionDecl, GenericParam,
    GenericParamList, ImplDecl, ImplMember, ImportItem, ImportedName, InterfaceDecl, Item,
    LiteralDecl, LiteralShape, MethodDecl, MethodReceiver, Parameter, ParameterList, PrimitiveDecl,
    StructDecl, StructField, TypeAliasDecl, Visibility,
};

impl Formatter {
    pub(super) fn format_file(&mut self, file: &AstFile) {
        for (index, item) in file.items.iter().enumerate() {
            if index > 0 {
                self.blank_line();
            }
            self.format_item(item);
        }
        self.newline();
    }

    fn format_item(&mut self, item: &Item) {
        match item {
            Item::Import(item) => self.format_import_item(item),
            Item::FromImport(item) => self.format_from_import_item(item),
            Item::Function(item) => self.format_function_decl(item),
            Item::Primitive(item) => self.format_primitive_decl(item),
            Item::TypeAlias(item) => self.format_type_alias_decl(item),
            Item::Struct(item) => self.format_struct_decl(item),
            Item::Enum(item) => self.format_enum_decl(item),
            Item::Interface(item) => self.format_interface_decl(item),
            Item::Impl(item) => self.format_impl_decl(item),
            Item::Literal(item) => self.format_literal_decl(item),
        }
    }

    fn format_literal_decl(&mut self, item: &LiteralDecl) {
        self.format_visibility(item.visibility);
        self.write("literal ");
        self.format_type(&item.target);
        self.write(" ");
        self.write(match item.shape {
            LiteralShape::Sequence => "[]",
            LiteralShape::String => "\"\"",
        });
        self.write("(");
        if let Some(capture) = &item.capture {
            self.write("...");
            self.write(&capture.name);
            self.write(": ");
            self.format_type(&capture.element_type);
        } else {
            self.write_comma_separated(&item.parameters.parameters, Self::format_parameter);
        }
        self.write("): ");
        self.format_type(&item.return_type);
        self.write(" ");
        self.format_block(&item.body);
    }

    pub(super) fn format_import_item(&mut self, item: &ImportItem) {
        self.write("use ");
        self.write(&item.path.value);
        if !item.alias_is_default {
            self.write(" as ");
            self.write(&item.alias.name);
        }
    }

    pub(super) fn format_from_import_item(&mut self, item: &FromImportItem) {
        self.format_visibility(item.visibility);
        self.write("use ");
        self.write(&item.path.value);
        self.write(".");
        if item.names.len() == 1 {
            self.format_imported_name(&item.names[0]);
        } else {
            self.write("{");
            self.write_comma_separated(&item.names, Self::format_imported_name);
            self.write("}");
        }
    }

    fn format_function_decl(&mut self, item: &FunctionDecl) {
        self.format_target_directive(item.target.as_ref());
        self.format_visibility(item.visibility);
        self.write("func ");
        self.write(&item.name);
        self.format_generics(&item.generics);
        self.format_parameters(&item.parameters);
        self.write(": ");
        self.format_type(&item.return_type);
        self.write(" ");
        self.format_block(&item.body);
    }

    fn format_primitive_decl(&mut self, item: &PrimitiveDecl) {
        self.format_target_directive(item.target.as_ref());
        self.format_visibility(item.visibility);
        self.write("primitive ");
        self.write(&item.name);
        self.format_generics(&item.generics);
        self.format_parameters(&item.parameters);
        self.write(": ");
        self.format_type(&item.return_type);
    }

    fn format_type_alias_decl(&mut self, item: &TypeAliasDecl) {
        self.format_target_directive(item.target_directive.as_ref());
        self.format_visibility(item.visibility);
        self.write("type ");
        self.write(&item.name);
        self.format_generics(&item.generics);
        self.write(" = ");
        self.format_type(&item.target);
    }

    fn format_struct_decl(&mut self, item: &StructDecl) {
        self.format_target_directive(item.target.as_ref());
        self.format_visibility(item.visibility);
        if item.is_copy {
            self.write("copy ");
        }
        self.write("struct ");
        self.write(&item.name);
        self.format_generics(&item.generics);
        self.write(" ");

        if item.fields.is_empty() {
            self.write("{}");
            return;
        }

        self.write("{");
        self.newline();
        self.indented(|formatter| {
            for field in &item.fields {
                formatter.write_indent();
                formatter.format_struct_field(field);
                formatter.write(",");
                formatter.newline();
            }
        });
        self.write_indent();
        self.write("}");
    }

    fn format_enum_decl(&mut self, item: &EnumDecl) {
        self.format_target_directive(item.target.as_ref());
        self.format_visibility(item.visibility);
        self.write("enum ");
        self.write(&item.name);
        self.format_generics(&item.generics);
        self.write(" ");

        if item.variants.is_empty() {
            self.write("{}");
            return;
        }

        self.write("{");
        self.newline();
        self.indented(|formatter| {
            for variant in &item.variants {
                formatter.write_indent();
                formatter.format_enum_variant(variant);
                formatter.write(",");
                formatter.newline();
            }
        });
        self.write_indent();
        self.write("}");
    }

    fn format_interface_decl(&mut self, item: &InterfaceDecl) {
        self.format_target_directive(item.target.as_ref());
        self.format_visibility(item.visibility);
        self.write("interface ");
        self.write(&item.name);
        self.format_generics(&item.generics);
        self.write(" ");

        if item.methods.is_empty() {
            self.write("{}");
            return;
        }

        self.write("{");
        self.newline();
        self.indented(|formatter| {
            for method in &item.methods {
                formatter.write_indent();
                formatter.format_method_decl(method);
                formatter.newline();
            }
        });
        self.write_indent();
        self.write("}");
    }

    fn format_impl_decl(&mut self, item: &ImplDecl) {
        self.write("impl ");
        if let Some(interface_ty) = &item.interface_ty {
            self.format_type(interface_ty);
            self.write(" for ");
        }
        self.format_type(&item.target_ty);
        self.write(" ");

        if item.members.is_empty() {
            self.write("{}");
            return;
        }

        self.write("{");
        self.newline();
        self.indented(|formatter| {
            for (index, member) in item.members.iter().enumerate() {
                if index > 0 {
                    formatter.newline();
                }
                formatter.write_indent();
                formatter.format_impl_member(member);
                formatter.newline();
            }
        });
        self.write_indent();
        self.write("}");
    }

    fn format_impl_member(&mut self, member: &ImplMember) {
        match member {
            ImplMember::Method(method) => self.format_method_decl(method),
            ImplMember::Drop(drop_) => self.format_drop_decl(drop_),
        }
    }

    fn format_drop_decl(&mut self, item: &DropDecl) {
        self.write("drop ");
        self.format_self_receiver(&item.binding);
        self.write(" ");
        self.format_block(&item.body);
    }

    fn format_method_decl(&mut self, item: &MethodDecl) {
        self.format_visibility(item.visibility);
        self.write("method ");
        self.format_method_receiver(&item.receiver);
        self.write(".");
        self.write(&item.name);
        self.format_parameters(&item.parameters);
        self.write(": ");
        self.format_type(&item.return_type);

        if let Some(body) = &item.body {
            self.write(" ");
            self.format_block(body);
        }
    }

    fn format_visibility(&mut self, visibility: Visibility) {
        match visibility {
            Visibility::Private => {}
            Visibility::Public => self.write("pub "),
            Visibility::Nocter => self.write("pub(nocter) "),
        }
    }

    fn format_struct_field(&mut self, field: &StructField) {
        self.format_visibility(field.visibility);
        self.write(&field.name);
        self.write(": ");
        self.format_type(&field.ty);
    }

    fn format_enum_variant(&mut self, variant: &EnumVariant) {
        self.write(&variant.name);
        if !variant.payload.is_empty() {
            self.write("(");
            self.write_comma_separated(&variant.payload, Self::format_parameter);
            self.write(")");
        }
    }

    fn format_imported_name(&mut self, name: &ImportedName) {
        self.write(&name.name);
        if let Some(alias) = &name.alias {
            self.write(" as ");
            self.write(&alias.name);
        }
    }

    fn format_generics(&mut self, generics: &GenericParamList) {
        if generics.parameters.is_empty() {
            return;
        }

        self.write("<");
        self.write_comma_separated(&generics.parameters, Self::format_generic_param);
        self.write(">");
    }

    fn format_generic_param(&mut self, parameter: &GenericParam) {
        self.write(&parameter.name);
        if let Some(bound) = &parameter.bound {
            self.write(": ");
            self.format_type(bound);
        }
    }

    fn format_parameters(&mut self, parameters: &ParameterList) {
        self.write("(");
        self.write_comma_separated(&parameters.parameters, Self::format_parameter);
        self.write(")");
    }

    fn format_method_receiver(&mut self, receiver: &MethodReceiver) {
        self.write(receiver.mode.source_prefix());
        self.write(&receiver.name);
    }

    fn format_self_receiver(&mut self, receiver: &Parameter) {
        if let Some(prefix) = self_receiver_prefix(receiver) {
            self.write(prefix);
            self.write(&receiver.name);
            return;
        }

        self.format_parameter(receiver);
    }

    fn format_parameter(&mut self, parameter: &Parameter) {
        self.write(&parameter.name);
        self.write(": ");
        self.format_type(&parameter.ty);
    }
}

fn self_receiver_prefix(receiver: &Parameter) -> Option<&'static str> {
    match &receiver.ty {
        crate::ast::TypeExpr::Reference(reference) if reference.name == "Self" => Some(""),
        crate::ast::TypeExpr::Borrow(borrow) => match borrow.inner.as_ref() {
            crate::ast::TypeExpr::Reference(reference) if reference.name == "Self" => {
                Some(if borrow.is_readwrite { "&+" } else { "&" })
            }
            _ => None,
        },
        _ => None,
    }
}

impl Formatter {
    fn format_target_directive(&mut self, target: Option<&crate::ast::TargetDirective>) {
        if let Some(target) = target {
            self.write("#target(");
            self.write_quoted_string_literal(&target.target);
            self.write(")");
            self.newline();
        }
    }

    fn write_quoted_string_literal(&mut self, value: &str) {
        self.write("\"");
        for character in value.chars() {
            match character {
                '\\' => self.write("\\\\"),
                '"' => self.write("\\\""),
                '\n' => self.write("\\n"),
                '\r' => self.write("\\r"),
                '\t' => self.write("\\t"),
                character => self.write(&character.to_string()),
            }
        }
        self.write("\"");
    }
}
