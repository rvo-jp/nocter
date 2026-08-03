use super::*;

impl TypecheckFactCollector<'_> {
    pub(in crate::typecheck::facts::collector) fn collect_item_signature_type_references(
        &mut self,
        item: &Item,
    ) {
        match item {
            Item::Import(_) | Item::FromImport(_) => {}
            Item::Function(function) => {
                self.facts.declaration_hover_labels.insert(
                    function.name_span,
                    function_declaration_hover_label(function, self.resolved),
                );
                if let Some(owner) = &function.owner {
                    self.record_type_reference(&owner.name, owner.name_span);
                }
                self.collect_generic_param_type_references(&function.generics);
                self.collect_parameter_type_references(&function.parameters.parameters);
                self.collect_type_expr_references(&function.return_type);
            }
            Item::Primitive(primitive) => {
                self.facts.declaration_hover_labels.insert(
                    primitive.name_span,
                    primitive_declaration_hover_label(primitive, self.resolved),
                );
                self.collect_generic_param_type_references(&primitive.generics);
                self.collect_parameter_type_references(&primitive.parameters.parameters);
                self.collect_type_expr_references(&primitive.return_type);
            }
            Item::TypeAlias(alias) => {
                self.facts.declaration_hover_labels.insert(
                    alias.name_span,
                    type_alias_declaration_hover_label(alias, self.resolved),
                );
                self.collect_generic_param_type_references(&alias.generics);
                self.collect_type_expr_references(&alias.target);
            }
            Item::Struct(struct_) => {
                self.facts.declaration_hover_labels.insert(
                    struct_.name_span,
                    struct_declaration_hover_label(struct_, self.resolved),
                );
                self.collect_generic_param_type_references(&struct_.generics);
                for field in &struct_.fields {
                    self.facts.declaration_hover_labels.insert(
                        field.name_span,
                        struct_field_declaration_hover_label(struct_, field, self.resolved),
                    );
                    self.collect_type_expr_references(&field.ty);
                }
            }
            Item::Enum(enum_) => {
                self.facts.declaration_hover_labels.insert(
                    enum_.name_span,
                    enum_declaration_hover_label(enum_, self.resolved),
                );
                self.collect_generic_param_type_references(&enum_.generics);
                for variant in &enum_.variants {
                    self.facts.declaration_hover_labels.insert(
                        variant.name_span,
                        enum_variant_declaration_hover_label(enum_, variant, self.resolved),
                    );
                    self.collect_parameter_type_references(&variant.payload);
                }
            }
            Item::Interface(interface) => {
                self.collect_generic_param_type_references(&interface.generics);
                for method in &interface.methods {
                    self.facts.declaration_hover_labels.insert(
                        method.name_span,
                        method_declaration_hover_label(method, self.resolved, None),
                    );
                    self.collect_method_signature_type_references(method);
                }
            }
            Item::Impl(impl_) => {
                let self_type = impl_self_type(impl_, self.resolved);
                if let Some(interface_ty) = &impl_.interface_ty {
                    self.collect_type_expr_references(interface_ty);
                }
                self.collect_type_expr_references(&impl_.target_ty);
                for member in &impl_.members {
                    match member {
                        ImplMember::Method(method) => {
                            self.facts.declaration_hover_labels.insert(
                                method.name_span,
                                method_declaration_hover_label(
                                    method,
                                    self.resolved,
                                    Some(&self_type),
                                ),
                            );
                            self.collect_method_signature_type_references(method);
                        }
                        ImplMember::Drop(drop_) => {
                            self.facts.declaration_hover_labels.insert(
                                drop_.binding.name_span,
                                drop_declaration_hover_label(drop_, self.resolved, &self_type),
                            );
                        }
                    }
                }
            }
            Item::Literal(literal) => {
                self.facts.declaration_hover_labels.insert(
                    literal.shape_span,
                    literal_declaration_hover_label(literal, self.resolved),
                );
                self.collect_type_expr_references(&literal.target);
                self.collect_parameter_type_references(&literal.parameters.parameters);
                if let Some(capture) = &literal.capture {
                    self.collect_type_expr_references(&capture.element_type);
                }
                self.collect_type_expr_references(&literal.return_type);
            }
        }
    }

    pub(in crate::typecheck::facts::collector) fn collect_method_signature_type_references(
        &mut self,
        method: &MethodDecl,
    ) {
        self.collect_parameter_type_references(&method.parameters.parameters);
        self.collect_type_expr_references(&method.return_type);
    }

    pub(in crate::typecheck::facts::collector) fn collect_generic_param_type_references(
        &mut self,
        generics: &GenericParamList,
    ) {
        for parameter in &generics.parameters {
            if let Some(bound) = &parameter.bound {
                self.collect_type_expr_references(bound);
            }
        }
    }

    pub(in crate::typecheck::facts::collector) fn collect_parameter_type_references(
        &mut self,
        parameters: &[Parameter],
    ) {
        for parameter in parameters {
            self.collect_type_expr_references(&parameter.ty);
        }
    }

    pub(in crate::typecheck::facts::collector) fn collect_type_expr_references(
        &mut self,
        ty: &TypeExpr,
    ) {
        match ty {
            TypeExpr::Reference(ty) => {
                self.record_type_reference(&ty.name, ty.span);
            }
            TypeExpr::Generic(ty) => {
                self.record_type_reference(&ty.name, ty.name_span);
                for argument in &ty.arguments {
                    self.collect_type_expr_references(argument);
                }
            }
            TypeExpr::Pointer(ty) => self.collect_type_expr_references(&ty.inner),
            TypeExpr::Borrow(ty) => self.collect_type_expr_references(&ty.inner),
            TypeExpr::View(ty) => self.collect_type_expr_references(&ty.element),
            TypeExpr::Array(ty) => self.collect_type_expr_references(&ty.element),
            TypeExpr::Optional(ty) => self.collect_type_expr_references(&ty.inner),
            TypeExpr::Fallible(ty) => {
                self.collect_type_expr_references(&ty.success);
                self.collect_type_expr_references(&ty.error);
            }
        }
    }
}
