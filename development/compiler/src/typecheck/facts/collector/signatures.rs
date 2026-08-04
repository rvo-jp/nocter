use super::*;

impl TypecheckFactCollector<'_> {
    pub(in crate::typecheck::facts::collector) fn collect_item_signature_type_references(
        &mut self,
        item: &Item,
    ) {
        let generics = match item {
            Item::Function(item) => Some(&item.generics),
            Item::Primitive(item) => Some(&item.generics),
            Item::TypeAlias(item) => Some(&item.generics),
            Item::Struct(item) => Some(&item.generics),
            Item::Enum(item) => Some(&item.generics),
            Item::Interface(item) => Some(&item.generics),
            Item::Impl(item) => Some(&item.generics),
            Item::Import(_) | Item::FromImport(_) | Item::Literal(_) | Item::Construct(_) => None,
        };
        if let Some(generics) = generics {
            self.with_generic_scope(generics, |collector| {
                collector.collect_item_signature_type_references_in_scope(item)
            });
        } else {
            self.collect_item_signature_type_references_in_scope(item);
        }
    }

    fn collect_item_signature_type_references_in_scope(&mut self, item: &Item) {
        match item {
            Item::Import(_) | Item::FromImport(_) => {}
            Item::Function(function) => {
                if let Some(owner) = &function.owner {
                    self.record_type_reference(
                        &owner.name,
                        owner.name_span,
                        TypeExpr::Reference(TypeReference {
                            span: owner.name_span,
                            name: owner.name.clone(),
                        }),
                    );
                }
                self.collect_generic_param_type_references(&function.generics);
                self.collect_parameter_type_references(&function.parameters.parameters);
                self.collect_type_expr_references(&function.return_type);
            }
            Item::Primitive(primitive) => {
                self.collect_generic_param_type_references(&primitive.generics);
                self.collect_parameter_type_references(&primitive.parameters.parameters);
                self.collect_type_expr_references(&primitive.return_type);
            }
            Item::TypeAlias(alias) => {
                self.collect_generic_param_type_references(&alias.generics);
                self.collect_type_expr_references(&alias.target);
            }
            Item::Struct(struct_) => {
                self.collect_generic_param_type_references(&struct_.generics);
                for field in &struct_.fields {
                    self.collect_type_expr_references(&field.ty);
                }
            }
            Item::Enum(enum_) => {
                self.collect_generic_param_type_references(&enum_.generics);
                for variant in &enum_.variants {
                    self.collect_parameter_type_references(&variant.payload);
                }
            }
            Item::Interface(interface) => {
                self.collect_generic_param_type_references(&interface.generics);
                for method in &interface.methods {
                    self.with_generic_scope(&method.generics, |collector| {
                        collector.collect_method_signature_type_references(method);
                    });
                }
            }
            Item::Impl(impl_) => {
                self.collect_generic_param_type_references(&impl_.generics);
                if let Some(interface_ty) = &impl_.interface_ty {
                    self.collect_type_expr_references(interface_ty);
                }
                self.collect_type_expr_references(&impl_.target_ty);
                for member in &impl_.members {
                    match member {
                        ImplMember::Method(method) => {
                            self.with_generic_scope(&method.generics, |collector| {
                                collector.collect_method_signature_type_references(method);
                            });
                        }
                        ImplMember::Drop(_) => {}
                    }
                }
            }
            Item::Literal(literal) => {
                self.collect_type_expr_references(&literal.target);
                self.collect_parameter_type_references(&literal.parameters.parameters);
                if let Some(capture) = &literal.capture {
                    self.collect_type_expr_references(&capture.element_type);
                }
                self.collect_type_expr_references(&literal.return_type);
            }
            Item::Construct(construct) => {
                self.collect_type_expr_references(&construct.target);
                for (_, function) in construct.functions() {
                    self.with_generic_scope(&function.generics, |collector| {
                        collector.collect_generic_param_type_references(&function.generics);
                        collector
                            .collect_parameter_type_references(&function.parameters.parameters);
                        collector.collect_type_expr_references(&function.return_type);
                    });
                }
                for (_, literal) in construct.literals() {
                    self.collect_parameter_type_references(&literal.parameters.parameters);
                    if let Some(capture) = &literal.capture {
                        self.collect_type_expr_references(&capture.element_type);
                    }
                    self.collect_type_expr_references(&literal.return_type);
                }
            }
        }
    }

    pub(in crate::typecheck::facts::collector) fn collect_method_signature_type_references(
        &mut self,
        method: &MethodDecl,
    ) {
        self.collect_generic_param_type_references(&method.generics);
        self.collect_parameter_type_references(&method.parameters.parameters);
        self.collect_type_expr_references(&method.return_type);
    }

    pub(in crate::typecheck::facts::collector) fn collect_generic_param_type_references(
        &mut self,
        generics: &GenericParamList,
    ) {
        for parameter in &generics.parameters {
            for bound in &parameter.bounds {
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
            TypeExpr::Callable(callable) => {
                for parameter in &callable.parameters {
                    self.collect_type_expr_references(&parameter.ty);
                }
                self.collect_type_expr_references(&callable.return_type);
            }
            TypeExpr::Closure(closure) => {
                for capture in &closure.captures {
                    self.collect_type_expr_references(&capture.ty);
                }
                for parameter in &closure.parameters {
                    self.collect_type_expr_references(parameter);
                }
                self.collect_type_expr_references(&closure.return_type);
            }
            TypeExpr::Reference(ty) => {
                self.record_type_reference(&ty.name, ty.span, TypeExpr::Reference(ty.clone()));
            }
            TypeExpr::Generic(ty) => {
                self.record_type_reference(&ty.name, ty.name_span, TypeExpr::Generic(ty.clone()));
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
