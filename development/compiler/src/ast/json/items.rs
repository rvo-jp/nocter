use super::*;

impl Item {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        match self {
            Item::Import(item) => JsonAstNode::with_value(
                if item.visibility.is_private() {
                    "use_namespace_item"
                } else {
                    "pub_use_namespace_item"
                },
                item.path.value.clone(),
                json_span(sources, item.span),
                vec![
                    visibility_json(item.visibility),
                    item.path.to_json(sources),
                    item.alias.to_json(sources),
                ],
            ),
            Item::FromImport(item) => {
                let mut children =
                    vec![visibility_json(item.visibility), item.path.to_json(sources)];
                children.extend(item.names.iter().map(|name| name.to_json(sources)));
                JsonAstNode::with_value(
                    if item.visibility.is_private() {
                        "use_names_item"
                    } else {
                        "pub_use_names_item"
                    },
                    item.path.value.clone(),
                    json_span(sources, item.span),
                    children,
                )
            }
            Item::Function(item) => {
                let mut children = target_directive_json(item.target.as_ref(), sources);
                children.extend([
                    visibility_json(item.visibility),
                    item.generics.to_json(sources),
                    item.parameters.to_json(sources),
                    item.return_type.to_json(sources),
                ]);
                if let Some(provenance) = &item.result_provenance {
                    children.push(provenance.to_json(sources));
                }
                if let Some(requirements) = &item.requirements {
                    children.push(requirements.to_json(sources));
                }
                if let Some(body) = &item.body {
                    children.push(body.to_json(sources));
                }
                JsonAstNode::with_value(
                    "function_decl",
                    item.name.clone(),
                    json_span(sources, item.span),
                    children,
                )
            }
            Item::Test(item) => JsonAstNode::with_value(
                "test_decl",
                item.name.clone(),
                json_span(sources, item.span),
                vec![item.body.to_json(sources)],
            ),
            Item::Primitive(item) => {
                let mut children = target_directive_json(item.target.as_ref(), sources);
                children.extend([
                    visibility_json(item.visibility),
                    item.generics.to_json(sources),
                    item.parameters.to_json(sources),
                    item.return_type.to_json(sources),
                ]);
                if let Some(provenance) = &item.result_provenance {
                    children.push(provenance.to_json(sources));
                }
                if let Some(requirements) = &item.requirements {
                    children.push(requirements.to_json(sources));
                }
                JsonAstNode::with_value(
                    "primitive_decl",
                    item.name.clone(),
                    json_span(sources, item.span),
                    children,
                )
            }
            Item::TypeAlias(item) => {
                let mut children = target_directive_json(item.target_directive.as_ref(), sources);
                children.extend([
                    visibility_json(item.visibility),
                    item.generics.to_json(sources),
                    item.target.to_json(sources),
                ]);
                if let Some(requirements) = &item.requirements {
                    children.push(requirements.to_json(sources));
                }
                JsonAstNode::with_value(
                    "type_alias_decl",
                    item.name.clone(),
                    json_span(sources, item.span),
                    children,
                )
            }
            Item::Struct(item) => {
                let mut children = target_directive_json(item.target.as_ref(), sources);
                children.push(visibility_json(item.visibility));
                if item.is_copy {
                    children.push(JsonAstNode::new("copy_modifier", None, Vec::new()));
                }
                children.push(item.generics.to_json(sources));
                if let Some(requirements) = &item.requirements {
                    children.push(requirements.to_json(sources));
                }
                children.extend(item.fields.iter().map(|field| field.to_json(sources)));
                JsonAstNode::with_value(
                    "struct_decl",
                    item.name.clone(),
                    json_span(sources, item.span),
                    children,
                )
            }
            Item::Enum(item) => {
                let mut children = target_directive_json(item.target.as_ref(), sources);
                children.extend([
                    visibility_json(item.visibility),
                    item.generics.to_json(sources),
                ]);
                if let Some(requirements) = &item.requirements {
                    children.push(requirements.to_json(sources));
                }
                children.extend(item.variants.iter().map(|variant| variant.to_json(sources)));
                JsonAstNode::with_value(
                    "enum_decl",
                    item.name.clone(),
                    json_span(sources, item.span),
                    children,
                )
            }
            Item::Interface(item) => {
                let mut children = target_directive_json(item.target.as_ref(), sources);
                children.extend([
                    visibility_json(item.visibility),
                    item.generics.to_json(sources),
                ]);
                if let Some(requirements) = &item.requirements {
                    children.push(requirements.to_json(sources));
                }
                children.extend(item.associated_types.iter().map(|associated_type| {
                    JsonAstNode::with_value(
                        "associated_type_decl",
                        associated_type.name.clone(),
                        json_span(sources, associated_type.span),
                        std::iter::once(visibility_json(Visibility::Public))
                            .chain(
                                associated_type
                                    .bounds
                                    .iter()
                                    .map(|bound| bound.to_json(sources)),
                            )
                            .collect(),
                    )
                }));
                children.extend(item.methods.iter().map(|method| method.to_json(sources)));
                JsonAstNode::with_value(
                    "interface_decl",
                    item.name.clone(),
                    json_span(sources, item.span),
                    children,
                )
            }
            Item::Instance(item) => {
                let mut children = vec![item.generics.to_json(sources)];
                children.push(JsonAstNode::new(
                    "instance_target_type",
                    json_span(sources, item.target_ty.span()),
                    vec![item.target_ty.to_json(sources)],
                ));
                if let Some(clause) = &item.requirements {
                    children.push(clause.to_json(sources));
                }
                children.extend(item.methods.iter().map(|method| method.to_json(sources)));
                children.extend(
                    item.operators
                        .iter()
                        .map(|operator| operator.to_json(sources)),
                );
                children.extend(item.coercions.iter().map(|entry| {
                    let callable = entry.callable_method();
                    let mut entry_children = vec![
                        visibility_json(callable.visibility),
                        callable.receiver.to_json(sources),
                        JsonAstNode::new(
                            "coerce_target_type",
                            json_span(sources, entry.target().span()),
                            vec![entry.target().to_json(sources)],
                        ),
                    ];
                    if let Some(provenance) = &callable.result_provenance {
                        entry_children.push(provenance.to_json(sources));
                    }
                    if let Some(body) = &callable.body {
                        entry_children.push(body.to_json(sources));
                    }
                    JsonAstNode::new(
                        "coercion_entry",
                        json_span(sources, entry.span),
                        entry_children,
                    )
                }));
                JsonAstNode::new("instance_decl", json_span(sources, item.span), children)
            }
            Item::Conformance(item) => {
                let mut children = vec![item.generics.to_json(sources)];
                children.push(JsonAstNode::new(
                    "interface_type",
                    json_span(sources, item.interface_ty.span()),
                    vec![item.interface_ty.to_json(sources)],
                ));
                children.push(JsonAstNode::new(
                    "conformance_target_type",
                    json_span(sources, item.target_ty.span()),
                    vec![item.target_ty.to_json(sources)],
                ));
                if let Some(clause) = &item.requirements {
                    children.push(clause.to_json(sources));
                }
                children.extend(item.members.iter().map(|member| member.to_json(sources)));
                JsonAstNode::new("conformance_decl", json_span(sources, item.span), children)
            }
            Item::Destruct(item) => JsonAstNode::new(
                "destruct_decl",
                json_span(sources, item.span),
                vec![
                    item.generics.to_json(sources),
                    JsonAstNode::new(
                        "destruct_target_type",
                        json_span(sources, item.target_ty.span()),
                        vec![item.target_ty.to_json(sources)],
                    ),
                    item.binding.to_json(sources),
                    item.body.to_json(sources),
                ],
            ),
            Item::Construct(item) => {
                let mut children = vec![item.target.to_json(sources)];
                children.extend(item.members.iter().map(|member| {
                    let mut member_children = Vec::new();
                    if member.is_default() {
                        member_children.push(JsonAstNode::new(
                            "default_modifier",
                            member
                                .default_span
                                .and_then(|span| json_span(sources, span)),
                            Vec::new(),
                        ));
                    }
                    let (kind, value, declaration_span) = match &member.declaration {
                        ConstructMemberDecl::Function(function) => {
                            member_children.extend([
                                function.generics.to_json(sources),
                                function.parameters.to_json(sources),
                                function.return_type.to_json(sources),
                            ]);
                            if let Some(provenance) = &function.result_provenance {
                                member_children.push(provenance.to_json(sources));
                            }
                            if let Some(requirements) = &function.requirements {
                                member_children.push(requirements.to_json(sources));
                            }
                            if let Some(body) = &function.body {
                                member_children.push(body.to_json(sources));
                            }
                            (
                                "construct_function_member",
                                function.member_name.clone(),
                                function.span,
                            )
                        }
                        ConstructMemberDecl::Literal(literal) => {
                            let mut parameters = literal
                                .parameters
                                .parameters
                                .iter()
                                .map(|parameter| parameter.to_json(sources))
                                .collect::<Vec<_>>();
                            if let Some(capture) = &literal.capture {
                                parameters.push(JsonAstNode::with_value(
                                    "literal_capture",
                                    capture.name.clone(),
                                    json_span(sources, capture.span),
                                    vec![capture.element_type.to_json(sources)],
                                ));
                            }
                            member_children.extend([
                                JsonAstNode::new(
                                    "literal_parameter_list",
                                    json_span(sources, literal.parameters.span),
                                    parameters,
                                ),
                                literal.return_type.to_json(sources),
                            ]);
                            if let Some(provenance) = &literal.result_provenance {
                                member_children.push(provenance.to_json(sources));
                            }
                            if let Some(requirements) = &literal.requirements {
                                member_children.push(requirements.to_json(sources));
                            }
                            if let Some(body) = &literal.body {
                                member_children.push(body.to_json(sources));
                            }
                            (
                                "construct_literal_member",
                                match literal.shape {
                                    LiteralShape::Sequence => "sequence".to_string(),
                                    LiteralShape::String => "string".to_string(),
                                },
                                literal.span,
                            )
                        }
                    };
                    JsonAstNode::with_value(
                        kind,
                        value,
                        json_span(sources, declaration_span),
                        member_children,
                    )
                }));
                JsonAstNode::new("construct_decl", json_span(sources, item.span), children)
            }
        }
    }
}

fn target_directive_json(
    target: Option<&TargetDirective>,
    sources: &SourceMap,
) -> Vec<JsonAstNode> {
    target
        .map(|target| vec![target.to_json(sources)])
        .unwrap_or_default()
}

impl TargetDirective {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::with_value(
            "target_directive",
            self.target.clone(),
            json_span(sources, self.span),
            Vec::new(),
        )
    }
}

impl ModulePath {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::with_value(
            "module_path",
            self.value.clone(),
            json_span(sources, self.span),
            Vec::new(),
        )
    }
}

impl ImportAlias {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::with_value(
            "import_alias",
            self.name.clone(),
            json_span(sources, self.span),
            Vec::new(),
        )
    }
}

impl ImportedName {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        let children = self
            .alias
            .as_ref()
            .map(|alias| vec![alias.to_json(sources)])
            .unwrap_or_default();
        JsonAstNode::with_value(
            "imported_name",
            self.name.clone(),
            json_span(sources, self.span),
            children,
        )
    }
}

impl ParameterList {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::new(
            "parameter_list",
            json_span(sources, self.span),
            self.parameters
                .iter()
                .map(|parameter| parameter.to_json(sources))
                .collect(),
        )
    }
}

impl GenericParamList {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::new(
            "generic_param_list",
            self.span.and_then(|span| json_span(sources, span)),
            self.parameters
                .iter()
                .map(|parameter| parameter.to_json(sources))
                .collect(),
        )
    }
}

impl GenericParam {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::with_value(
            "generic_param",
            self.name.clone(),
            json_span(sources, self.span),
            Vec::new(),
        )
    }
}

impl WhereClause {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::new(
            "where_clause",
            json_span(sources, self.span),
            self.predicates
                .iter()
                .map(|predicate| match predicate {
                    crate::ast::WherePredicate::Copy(requirement) => JsonAstNode::with_value(
                        "copy_requirement",
                        requirement.name.clone(),
                        json_span(sources, requirement.span),
                        Vec::new(),
                    ),
                    crate::ast::WherePredicate::Generic(requirement) => {
                        let children = requirement
                            .bounds
                            .iter()
                            .map(|bound| bound.to_json(sources))
                            .collect::<Vec<_>>();
                        JsonAstNode::with_value(
                            "generic_requirement",
                            requirement.name.clone(),
                            json_span(sources, requirement.span),
                            children,
                        )
                    }
                    crate::ast::WherePredicate::Refinement(refinement) => JsonAstNode::with_value(
                        "binder_refinement_predicate",
                        refinement.name.clone(),
                        json_span(sources, refinement.span),
                        vec![refinement.value.to_json(sources)],
                    ),
                    crate::ast::WherePredicate::Equality(equality) => JsonAstNode::new(
                        "type_equality_predicate",
                        json_span(sources, equality.span),
                        vec![
                            equality.left.to_json(sources),
                            equality.right.to_json(sources),
                        ],
                    ),
                    crate::ast::WherePredicate::Operator(requirement) => JsonAstNode::new(
                        "operator_requirement",
                        json_span(sources, requirement.span),
                        {
                            let mut children = match &requirement.shape {
                                crate::ast::OperatorRequirementShape::Comparison {
                                    kind,
                                    left,
                                    right,
                                    ..
                                } => vec![JsonAstNode::with_value(
                                    "comparison_operator_shape",
                                    kind.source_token().to_string(),
                                    json_span(sources, requirement.span),
                                    vec![left.to_json(sources), right.to_json(sources)],
                                )],
                                crate::ast::OperatorRequirementShape::Index {
                                    target,
                                    index,
                                    ..
                                } => vec![JsonAstNode::new(
                                    "index_operator_shape",
                                    json_span(sources, requirement.span),
                                    vec![target.to_json(sources), index.to_json(sources)],
                                )],
                                crate::ast::OperatorRequirementShape::Expansion {
                                    source, ..
                                } => vec![JsonAstNode::new(
                                    "expansion_operator_shape",
                                    json_span(sources, requirement.span),
                                    vec![source.to_json(sources)],
                                )],
                            };
                            children.push(requirement.result.to_json(sources));
                            children
                        },
                    ),
                    crate::ast::WherePredicate::Coercion(requirement) => JsonAstNode::new(
                        "coercion_requirement",
                        json_span(sources, requirement.span),
                        vec![
                            requirement.source.to_json(sources),
                            requirement.target.to_json(sources),
                        ],
                    ),
                })
                .collect(),
        )
    }
}

impl ConformanceMember {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        match self {
            ConformanceMember::AssociatedType(binding) => JsonAstNode::with_value(
                "associated_type_binding",
                binding.name.clone(),
                json_span(sources, binding.span),
                vec![binding.value.to_json(sources)],
            ),
            ConformanceMember::Method(method) => method.to_json(sources),
        }
    }
}

impl MethodDecl {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        let mut children = vec![
            visibility_json(self.visibility),
            self.receiver.to_json(sources),
            self.generics.to_json(sources),
            self.parameters.to_json(sources),
            self.return_type.to_json(sources),
        ];
        if let Some(provenance) = &self.result_provenance {
            children.push(provenance.to_json(sources));
        }
        if let Some(requirements) = &self.requirements {
            children.push(requirements.to_json(sources));
        }
        if let Some(body) = &self.body {
            children.push(body.to_json(sources));
        }

        JsonAstNode::with_value(
            if self.body.is_some() {
                "method_decl"
            } else {
                "method_signature"
            },
            self.name.clone(),
            json_span(sources, self.span),
            children,
        )
    }
}

impl crate::ast::ComparisonOperatorDecl {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        let callable = self.callable_method();
        JsonAstNode::with_value(
            "operator_decl",
            self.kind.source_token().to_string(),
            json_span(sources, self.span),
            vec![
                visibility_json(callable.visibility),
                callable.receiver.to_json(sources),
                callable.parameters.parameters[0].to_json(sources),
                callable.return_type.to_json(sources),
                callable
                    .body
                    .as_ref()
                    .expect("operator body")
                    .to_json(sources),
            ],
        )
    }
}

impl crate::ast::IndexOperatorDecl {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        let callable = self.callable_method();
        let mut children = vec![
            visibility_json(callable.visibility),
            callable.receiver.to_json(sources),
            callable.parameters.parameters[0].to_json(sources),
            callable.return_type.to_json(sources),
        ];
        if let Some(provenance) = &callable.result_provenance {
            children.push(provenance.to_json(sources));
        }
        children.push(
            callable
                .body
                .as_ref()
                .expect("operator body")
                .to_json(sources),
        );
        JsonAstNode::with_value(
            "operator_decl",
            "[]".to_string(),
            json_span(sources, self.span),
            children,
        )
    }
}

impl crate::ast::ExpansionOperatorDecl {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        let callable = self.callable_method();
        let mut children = vec![
            visibility_json(callable.visibility),
            callable.receiver.to_json(sources),
            callable.return_type.to_json(sources),
        ];
        if let Some(provenance) = &callable.result_provenance {
            children.push(provenance.to_json(sources));
        }
        children.push(
            callable
                .body
                .as_ref()
                .expect("operator body")
                .to_json(sources),
        );
        JsonAstNode::with_value(
            "operator_decl",
            "...".to_string(),
            json_span(sources, self.span),
            children,
        )
    }
}

impl crate::ast::OperatorDecl {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        match self {
            Self::Comparison(operator) => operator.to_json(sources),
            Self::Index(operator) => operator.to_json(sources),
            Self::Expansion(operator) => operator.to_json(sources),
        }
    }
}

impl MethodReceiver {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::with_value(
            "method_receiver",
            self.mode.label(),
            json_span(sources, self.span),
            vec![JsonAstNode::with_value(
                "parameter",
                self.name.clone(),
                json_span(sources, self.name_span),
                Vec::new(),
            )],
        )
    }
}

impl ResultProvenanceClause {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::new(
            "result_provenance",
            json_span(sources, self.span),
            self.origins
                .iter()
                .map(|origin| {
                    JsonAstNode::with_value(
                        "result_provenance_origin",
                        origin.kind.source_label(),
                        json_span(sources, origin.span),
                        Vec::new(),
                    )
                })
                .collect(),
        )
    }
}

impl Parameter {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::with_value(
            "parameter",
            self.name.clone(),
            json_span(sources, self.span),
            vec![self.ty.to_json(sources)],
        )
    }
}

impl StructField {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::with_value(
            "struct_field",
            self.name.clone(),
            json_span(sources, self.span),
            vec![visibility_json(self.visibility), self.ty.to_json(sources)],
        )
    }
}

impl StructLiteralField {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::with_value(
            "struct_literal_field",
            self.name.clone(),
            json_span(sources, self.span),
            vec![self.value.to_json(sources)],
        )
    }
}

impl EnumVariant {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        JsonAstNode::with_value(
            "enum_variant",
            self.name.clone(),
            json_span(sources, self.span),
            self.payload
                .iter()
                .map(|parameter| parameter.to_json(sources))
                .collect(),
        )
    }
}

fn visibility_json(visibility: Visibility) -> JsonAstNode {
    JsonAstNode::with_value(
        "visibility",
        match visibility {
            Visibility::Private => "private".to_string(),
            Visibility::ModuleTree(_) | Visibility::Package | Visibility::Public => {
                visibility.source_notation()
            }
        },
        None,
        Vec::new(),
    )
}
