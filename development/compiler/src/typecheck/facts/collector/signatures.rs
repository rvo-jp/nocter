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
            Item::Instance(item) => Some(&item.generics),
            Item::Destruct(item) => Some(&item.generics),
            Item::Conformance(item) => Some(&item.generics),
            Item::Coerce(item) => Some(&item.generics),
            Item::Import(_) | Item::FromImport(_) | Item::Construct(_) | Item::Test(_) => None,
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
            Item::Import(_) | Item::FromImport(_) | Item::Test(_) => {}
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
                self.collect_where_clause_type_references(function.requirements.as_ref());
                self.collect_parameter_type_references(&function.parameters.parameters);
                self.collect_type_expr_references(&function.return_type);
            }
            Item::Primitive(primitive) => {
                self.collect_generic_param_type_references(&primitive.generics);
                self.collect_where_clause_type_references(primitive.requirements.as_ref());
                self.collect_parameter_type_references(&primitive.parameters.parameters);
                self.collect_type_expr_references(&primitive.return_type);
            }
            Item::TypeAlias(alias) => {
                self.collect_generic_param_type_references(&alias.generics);
                self.collect_type_expr_references(&alias.target);
                self.collect_where_clause_type_references(alias.requirements.as_ref());
            }
            Item::Struct(struct_) => {
                self.collect_generic_param_type_references(&struct_.generics);
                self.collect_where_clause_type_references(struct_.requirements.as_ref());
                for field in &struct_.fields {
                    self.collect_type_expr_references(&field.ty);
                }
            }
            Item::Enum(enum_) => {
                self.collect_generic_param_type_references(&enum_.generics);
                self.collect_where_clause_type_references(enum_.requirements.as_ref());
                for variant in &enum_.variants {
                    self.collect_parameter_type_references(&variant.payload);
                }
            }
            Item::Interface(interface) => {
                self.collect_generic_param_type_references(&interface.generics);
                self.collect_where_clause_type_references(interface.requirements.as_ref());
                for associated in &interface.associated_types {
                    for bound in &associated.bounds {
                        self.collect_type_expr_references(bound);
                    }
                }
                self.with_associated_type_scope(
                    interface
                        .associated_types
                        .iter()
                        .map(|associated| (associated.name.clone(), associated.name_span)),
                    |collector| {
                        for method in &interface.methods {
                            collector.with_generic_scope(&method.generics, |collector| {
                                collector.collect_method_signature_type_references(method);
                            });
                        }
                    },
                );
            }
            Item::Instance(instance) => {
                self.collect_generic_param_type_references(&instance.generics);
                self.collect_type_expr_references(&instance.target_ty);
                self.collect_where_clause_type_references(instance.requirements.as_ref());
                for method in &instance.methods {
                    self.with_generic_scope(&method.generics, |collector| {
                        collector.collect_method_signature_type_references(method)
                    });
                }
            }
            Item::Destruct(destruct) => {
                self.collect_generic_param_type_references(&destruct.generics);
                self.collect_type_expr_references(&destruct.target_ty);
            }
            Item::Conformance(conformance) => {
                self.collect_generic_param_type_references(&conformance.generics);
                self.collect_declaration_pattern_type_references(
                    &conformance.interface_ty,
                    conformance.requirements.as_ref(),
                );
                self.collect_type_expr_references(&conformance.target_ty);
                self.collect_where_clause_type_references(conformance.requirements.as_ref());
                let associated_types = match &conformance.interface_ty {
                    TypeExpr::Reference(reference) => {
                        self.resolved.type_symbol_by_reference_name(&reference.name)
                    }
                    TypeExpr::Generic(generic) => {
                        self.resolved.type_symbol_by_reference_name(&generic.name)
                    }
                    _ => None,
                }
                .map(|interface| {
                    interface
                        .associated_types
                        .iter()
                        .map(|associated| (associated.name.clone(), associated.name_span))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
                self.with_associated_type_scope(associated_types, |collector| {
                    for member in &conformance.members {
                        match member {
                            ConformanceMember::AssociatedType(binding) => {
                                if let Some(target) = collector
                                    .associated_type_declaration(&binding.name)
                                    .map(TypeOccurrenceTarget::Member)
                                {
                                    collector.facts.type_occurrences.push(TypeOccurrenceFact {
                                        focus_span: binding.name_span,
                                        contextual_type: binding.value.clone(),
                                        target: Some(target),
                                    });
                                }
                                collector.collect_type_expr_references(&binding.value);
                            }
                            ConformanceMember::Method(method) => {
                                collector.with_generic_scope(&method.generics, |collector| {
                                    collector.collect_method_signature_type_references(method);
                                });
                            }
                        }
                    }
                });
            }
            Item::Construct(construct) => {
                self.collect_type_expr_references(&construct.target);
                for (_, function) in construct.functions() {
                    self.with_generic_scope(&function.generics, |collector| {
                        collector.collect_generic_param_type_references(&function.generics);
                        collector
                            .collect_where_clause_type_references(function.requirements.as_ref());
                        collector
                            .collect_parameter_type_references(&function.parameters.parameters);
                        collector.collect_type_expr_references(&function.return_type);
                    });
                }
                for (_, literal) in construct.literals() {
                    self.collect_where_clause_type_references(literal.requirements.as_ref());
                    self.collect_parameter_type_references(&literal.parameters.parameters);
                    if let Some(capture) = &literal.capture {
                        self.collect_type_expr_references(&capture.element_type);
                    }
                    self.collect_type_expr_references(&literal.return_type);
                }
            }
            Item::Coerce(coerce) => {
                self.collect_generic_param_type_references(&coerce.generics);
                self.collect_type_expr_references(&coerce.target);
                for entry in &coerce.entries {
                    self.collect_type_expr_references(&entry.target);
                }
            }
        }
    }

    pub(in crate::typecheck::facts::collector) fn collect_method_signature_type_references(
        &mut self,
        method: &MethodDecl,
    ) {
        self.collect_generic_param_type_references(&method.generics);
        self.collect_where_clause_type_references(method.requirements.as_ref());
        self.collect_parameter_type_references(&method.parameters.parameters);
        self.collect_type_expr_references(&method.return_type);
    }

    fn collect_where_clause_type_references(&mut self, clause: Option<&crate::ast::WhereClause>) {
        let Some(clause) = clause else {
            return;
        };
        for requirement in clause.copy_requirements() {
            if let Some(declaration) = self.generic_parameter_declaration(&requirement.name)
                && let Some(parameter) = self
                    .facts
                    .generic_parameter_declarations
                    .iter_mut()
                    .find(|parameter| parameter.span == declaration)
            {
                parameter.is_copy = true;
            }
            self.record_type_reference(
                &requirement.name,
                requirement.name_span,
                TypeExpr::Reference(TypeReference {
                    span: requirement.name_span,
                    name: requirement.name.clone(),
                }),
            );
        }
        for requirement in clause.generic_requirements() {
            if let Some(declaration) = self.generic_parameter_declaration(&requirement.name)
                && let Some(parameter) = self
                    .facts
                    .generic_parameter_declarations
                    .iter_mut()
                    .find(|parameter| parameter.span == declaration)
            {
                for bound in &requirement.bounds {
                    if !parameter.bounds.contains(bound) {
                        parameter.bounds.push(bound.clone());
                    }
                }
            }
            self.record_type_reference(
                &requirement.name,
                requirement.name_span,
                TypeExpr::Reference(TypeReference {
                    span: requirement.name_span,
                    name: requirement.name.clone(),
                }),
            );
            for bound in &requirement.bounds {
                self.collect_type_expr_references(bound);
            }
        }
        for equality in clause.equalities() {
            self.collect_type_expr_references(&equality.left);
            self.collect_type_expr_references(&equality.right);
        }
        for refinement in clause.refinements() {
            self.record_type_reference(
                &refinement.name,
                refinement.name_span,
                TypeExpr::Reference(TypeReference {
                    span: refinement.name_span,
                    name: refinement.name.clone(),
                }),
            );
            self.collect_type_expr_references(&refinement.value);
        }
    }

    fn collect_declaration_pattern_type_references(
        &mut self,
        ty: &TypeExpr,
        clause: Option<&crate::ast::WhereClause>,
    ) {
        let substitutions = clause
            .into_iter()
            .flat_map(crate::ast::WhereClause::refinements)
            .map(|refinement| (refinement.name.clone(), refinement.value.clone()))
            .collect();
        match ty {
            TypeExpr::Generic(generic) => {
                let contextual = crate::ast::substitute_type_expr_parameters(ty, &substitutions);
                self.record_type_reference(&generic.name, generic.name_span, contextual);
                for argument in &generic.arguments {
                    self.collect_type_expr_references(argument);
                }
            }
            _ => self.collect_type_expr_references(ty),
        }
    }

    pub(in crate::typecheck::facts::collector) fn collect_generic_param_type_references(
        &mut self,
        _generics: &GenericParamList,
    ) {
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
            TypeExpr::Opaque(opaque) => {
                self.collect_type_expr_references(&opaque.interface);
                for binding in &opaque.associated_bindings {
                    self.collect_type_expr_references(&binding.value);
                }
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
            TypeExpr::Projection(ty) => {
                self.collect_type_expr_references(&ty.base);
                self.record_associated_type_reference(ty);
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
