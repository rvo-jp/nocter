use super::Formatter;
use crate::ast::{
    AssociatedTypeBinding, AssociatedTypeDecl, AstFile, CoerceDecl, CoercionEntry, ConformanceDecl,
    ConformanceMember, ConstructDecl, ConstructMember, ConstructMemberDecl, DestructDecl, EnumDecl,
    EnumVariant, FromImportItem, FunctionDecl, GenericParam, GenericParamList, ImportItem,
    ImportedName, InstanceDecl, InterfaceDecl, Item, LiteralDecl, LiteralShape, MethodDecl,
    MethodReceiver, PackageFile, Parameter, ParameterList, PrimitiveDecl, ResultProvenanceClause,
    StructDecl, StructField, TestDecl, TypeAliasDecl, TypeExpr, Visibility, WhereClause,
};

impl Formatter {
    pub(super) fn format_package_file(&mut self, file: &PackageFile) {
        for (index, directive) in file.manifest.directives.iter().enumerate() {
            if index > 0 {
                self.newline();
            }
            self.format_package_directive(directive);
        }
        self.newline();
    }

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
            Item::Test(item) => self.format_test_decl(item),
            Item::Primitive(item) => self.format_primitive_decl(item),
            Item::TypeAlias(item) => self.format_type_alias_decl(item),
            Item::Struct(item) => self.format_struct_decl(item),
            Item::Enum(item) => self.format_enum_decl(item),
            Item::Interface(item) => self.format_interface_decl(item),
            Item::Instance(item) => self.format_instance_decl(item),
            Item::Conformance(item) => self.format_conformance_decl(item),
            Item::Destruct(item) => self.format_destruct_decl(item),
            Item::Construct(item) => self.format_construct_decl(item),
            Item::Coerce(item) => self.format_coerce_decl(item),
        }
    }

    fn format_coerce_decl(&mut self, item: &CoerceDecl) {
        self.write("coerce ");
        self.format_type(&item.target);
        if item.entries.is_empty() {
            self.write(" {}");
            return;
        }
        self.write(" {");
        self.newline();
        self.indented(|formatter| {
            for (index, entry) in item.entries.iter().enumerate() {
                if index > 0 {
                    formatter.newline();
                }
                formatter.write_indent();
                formatter.format_coercion_entry(entry);
                formatter.newline();
            }
        });
        self.write_indent();
        self.write("}");
    }

    fn format_coercion_entry(&mut self, entry: &CoercionEntry) {
        self.format_visibility(entry.visibility);
        self.format_method_receiver(&entry.receiver);
        self.write(" as ");
        self.format_type(&entry.target);
        self.format_result_provenance(entry.result_provenance.as_ref());
        if let Some(body) = &entry.body {
            self.write(" ");
            self.format_block(body);
        }
    }

    fn format_test_decl(&mut self, item: &TestDecl) {
        self.write("test ");
        self.write(&item.name);
        self.write(" ");
        self.format_block(&item.body);
    }

    fn format_construct_decl(&mut self, item: &ConstructDecl) {
        self.write("construct ");
        self.format_type(&item.target);
        if item.members.is_empty() {
            self.write(" {}");
            return;
        }
        self.write(" {");
        self.newline();
        let owner_generic_count = match &item.target {
            TypeExpr::Generic(generic) => generic.arguments.len(),
            _ => 0,
        };
        self.indented(|formatter| {
            for (index, member) in item.members.iter().enumerate() {
                if index > 0 {
                    formatter.newline();
                }
                formatter.write_indent();
                formatter.format_construct_member(member, owner_generic_count);
                formatter.newline();
            }
        });
        self.write_indent();
        self.write("}");
    }

    fn format_construct_member(&mut self, member: &ConstructMember, owner_generic_count: usize) {
        self.format_visibility(match &member.declaration {
            ConstructMemberDecl::Function(function) => function.visibility,
            ConstructMemberDecl::Literal(literal) => literal.visibility,
        });
        if member.is_default() {
            self.write("default ");
        }
        match &member.declaration {
            ConstructMemberDecl::Function(function) => {
                self.write("func ");
                self.write(&function.member_name);
                self.format_generic_params(&function.generics.parameters[owner_generic_count..]);
                self.format_parameters(&function.parameters);
                self.write(": ");
                self.format_type(&function.return_type);
                self.format_result_provenance(function.result_provenance.as_ref());
                self.format_where_clause(function.requirements.as_ref());
                if let Some(body) = &function.body {
                    self.write(" ");
                    self.format_block(body);
                }
            }
            ConstructMemberDecl::Literal(literal) => self.format_construct_literal(literal),
        }
    }

    fn format_construct_literal(&mut self, item: &LiteralDecl) {
        self.write("literal ");
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
        self.format_result_provenance(item.result_provenance.as_ref());
        self.format_where_clause(item.requirements.as_ref());
        if let Some(body) = &item.body {
            self.write(" ");
            self.format_block(body);
        }
    }

    pub(super) fn format_import_item(&mut self, item: &ImportItem) {
        self.format_visibility(item.visibility);
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
        self.format_result_provenance(item.result_provenance.as_ref());
        self.format_where_clause(item.requirements.as_ref());
        if let Some(body) = &item.body {
            self.write(" ");
            self.format_block(body);
        }
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
        self.format_result_provenance(item.result_provenance.as_ref());
        self.format_where_clause(item.requirements.as_ref());
    }

    fn format_type_alias_decl(&mut self, item: &TypeAliasDecl) {
        self.format_target_directive(item.target_directive.as_ref());
        self.format_visibility(item.visibility);
        self.write("type ");
        self.write(&item.name);
        self.format_generics(&item.generics);
        self.write(" = ");
        self.format_type(&item.target);
        self.format_where_clause(item.requirements.as_ref());
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
        self.format_where_clause(item.requirements.as_ref());
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
        self.format_where_clause(item.requirements.as_ref());
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
        self.format_where_clause(item.requirements.as_ref());
        self.write(" ");

        if item.associated_types.is_empty() && item.methods.is_empty() {
            self.write("{}");
            return;
        }

        self.write("{");
        self.newline();
        self.indented(|formatter| {
            for associated_type in &item.associated_types {
                formatter.write_indent();
                formatter.format_associated_type_decl(associated_type);
                formatter.newline();
            }
            if !item.associated_types.is_empty() && !item.methods.is_empty() {
                formatter.newline();
            }
            for method in &item.methods {
                formatter.write_indent();
                formatter.format_method_decl(method);
                formatter.newline();
            }
        });
        self.write_indent();
        self.write("}");
    }

    fn format_associated_type_decl(&mut self, item: &AssociatedTypeDecl) {
        self.write("pub type ");
        self.write(&item.name);
        if !item.bounds.is_empty() {
            self.write(": ");
            for (index, bound) in item.bounds.iter().enumerate() {
                if index != 0 {
                    self.write(" + ");
                }
                self.format_type(bound);
            }
        }
    }

    fn format_instance_decl(&mut self, item: &InstanceDecl) {
        self.write("instance ");
        self.format_type(&item.target_ty);
        self.format_where_clause(item.requirements.as_ref());
        self.write(" ");

        if item.methods.is_empty() {
            self.write("{}");
            return;
        }

        self.write("{");
        self.newline();
        self.indented(|formatter| {
            for (index, method) in item.methods.iter().enumerate() {
                if index > 0 {
                    formatter.newline();
                }
                formatter.write_indent();
                formatter.format_method_decl(method);
                formatter.newline();
            }
        });
        self.write_indent();
        self.write("}");
    }

    fn format_conformance_decl(&mut self, item: &ConformanceDecl) {
        self.write("conform ");
        self.format_type(&item.interface_ty);
        self.write(" for ");
        self.format_type(&item.target_ty);
        self.format_where_clause(item.requirements.as_ref());
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
                match member {
                    ConformanceMember::AssociatedType(binding) => {
                        formatter.format_associated_type_binding(binding);
                    }
                    ConformanceMember::Method(method) => formatter.format_method_decl(method),
                }
                formatter.newline();
            }
        });
        self.write_indent();
        self.write("}");
    }

    fn format_associated_type_binding(&mut self, binding: &AssociatedTypeBinding) {
        self.write("type ");
        self.write(&binding.name);
        self.write(" = ");
        self.format_type(&binding.value);
    }

    fn format_destruct_decl(&mut self, item: &DestructDecl) {
        self.write("destruct ");
        self.format_type(&item.target_ty);
        self.write("(");
        self.format_self_receiver(&item.binding);
        self.write(") ");
        self.format_block(&item.body);
    }

    fn format_method_decl(&mut self, item: &MethodDecl) {
        self.format_visibility(item.visibility);
        self.write("method ");
        self.format_method_receiver(&item.receiver);
        self.write(".");
        self.write(&item.name);
        self.format_generics(&item.generics);
        self.format_parameters(&item.parameters);
        self.write(": ");
        self.format_type(&item.return_type);
        self.format_result_provenance(item.result_provenance.as_ref());
        self.format_where_clause(item.requirements.as_ref());

        if let Some(body) = &item.body {
            self.write(" ");
            self.format_block(body);
        }
    }

    fn format_visibility(&mut self, visibility: Visibility) {
        match visibility {
            Visibility::Private => {}
            Visibility::Public => self.write("pub "),
            Visibility::ModuleTree(_) | Visibility::Package => {
                self.write(&visibility.source_notation());
                self.write(" ");
            }
        }
    }

    fn format_result_provenance(&mut self, clause: Option<&ResultProvenanceClause>) {
        let Some(clause) = clause else {
            return;
        };
        self.write(" from ");
        for (index, origin) in clause.origins.iter().enumerate() {
            if index > 0 {
                self.write(" | ");
            }
            self.write(origin.kind.source_label());
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
        self.format_generic_params(&generics.parameters);
    }

    fn format_generic_params(&mut self, parameters: &[GenericParam]) {
        if parameters.is_empty() {
            return;
        }

        self.write("<");
        self.write_comma_separated(parameters, Self::format_generic_param);
        self.write(">");
    }

    fn format_generic_param(&mut self, parameter: &GenericParam) {
        self.write(&parameter.name);
    }

    fn format_where_clause(&mut self, clause: Option<&WhereClause>) {
        let Some(clause) = clause else {
            return;
        };
        self.write(" where ");
        for (index, predicate) in clause.predicates.iter().enumerate() {
            if index != 0 {
                self.write(", ");
            }
            match predicate {
                crate::ast::WherePredicate::Copy(requirement) => {
                    self.write("copy ");
                    self.write(&requirement.name);
                }
                crate::ast::WherePredicate::Generic(requirement) => {
                    self.write(&requirement.name);
                    self.write(": ");
                    for (index, bound) in requirement.bounds.iter().enumerate() {
                        if index != 0 {
                            self.write(" + ");
                        }
                        self.format_type(bound);
                    }
                }
                crate::ast::WherePredicate::Refinement(refinement) => {
                    self.write(&refinement.name);
                    self.write(" = ");
                    self.format_type(&refinement.value);
                }
                crate::ast::WherePredicate::Equality(equality) => {
                    self.format_type(&equality.left);
                    self.write(" = ");
                    self.format_type(&equality.right);
                }
            }
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
            self.write("#target: ");
            self.write_quoted_string_literal(&target.target);
            self.newline();
        }
    }

    pub(super) fn write_quoted_string_literal(&mut self, value: &str) {
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
