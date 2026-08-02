use super::*;

impl Item {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        match self {
            Item::Import(item) => JsonAstNode::with_value(
                "use_namespace_item",
                item.path.value.clone(),
                json_span(sources, item.span),
                vec![item.path.to_json(sources), item.alias.to_json(sources)],
            ),
            Item::FromImport(item) => {
                let mut children = vec![item.path.to_json(sources)];
                children.extend(item.names.iter().map(|name| name.to_json(sources)));
                JsonAstNode::with_value(
                    match item.visibility {
                        Visibility::Public => "pub_use_names_item",
                        Visibility::Private | Visibility::Nocter => "use_names_item",
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
                    item.body.to_json(sources),
                ]);
                JsonAstNode::with_value(
                    "function_decl",
                    item.name.clone(),
                    json_span(sources, item.span),
                    children,
                )
            }
            Item::Primitive(item) => {
                let mut children = target_directive_json(item.target.as_ref(), sources);
                children.extend([
                    visibility_json(item.visibility),
                    item.generics.to_json(sources),
                    item.parameters.to_json(sources),
                    item.return_type.to_json(sources),
                ]);
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
                children.extend(item.methods.iter().map(|method| method.to_json(sources)));
                JsonAstNode::with_value(
                    "interface_decl",
                    item.name.clone(),
                    json_span(sources, item.span),
                    children,
                )
            }
            Item::Impl(item) => {
                let mut children = vec![item.generics.to_json(sources)];
                if let Some(interface_ty) = &item.interface_ty {
                    children.push(JsonAstNode::new(
                        "interface_type",
                        json_span(sources, interface_ty.span()),
                        vec![interface_ty.to_json(sources)],
                    ));
                }
                children.push(JsonAstNode::new(
                    "impl_target_type",
                    json_span(sources, item.target_ty.span()),
                    vec![item.target_ty.to_json(sources)],
                ));
                children.extend(item.members.iter().map(|member| member.to_json(sources)));
                JsonAstNode::new("impl_decl", json_span(sources, item.span), children)
            }
            Item::Literal(item) => {
                let mut parameter_children = item
                    .parameters
                    .parameters
                    .iter()
                    .map(|parameter| parameter.to_json(sources))
                    .collect::<Vec<_>>();
                if let Some(capture) = &item.capture {
                    parameter_children.push(JsonAstNode::with_value(
                        "literal_capture",
                        capture.name.clone(),
                        json_span(sources, capture.span),
                        vec![capture.element_type.to_json(sources)],
                    ));
                }
                JsonAstNode::with_value(
                    "literal_decl",
                    match item.shape {
                        LiteralShape::Sequence => "sequence",
                        LiteralShape::String => "string",
                    },
                    json_span(sources, item.span),
                    vec![
                        visibility_json(item.visibility),
                        item.target.to_json(sources),
                        JsonAstNode::new(
                            "literal_parameter_list",
                            json_span(sources, item.parameters.span),
                            parameter_children,
                        ),
                        item.return_type.to_json(sources),
                        item.body.to_json(sources),
                    ],
                )
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
        let children = self
            .bound
            .as_ref()
            .map(|bound| vec![bound.to_json(sources)])
            .unwrap_or_default();
        JsonAstNode::with_value(
            "generic_param",
            self.name.clone(),
            json_span(sources, self.span),
            children,
        )
    }
}

impl ImplMember {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        match self {
            ImplMember::Method(method) => method.to_json(sources),
            ImplMember::Drop(drop_) => JsonAstNode::with_value(
                "drop_decl",
                drop_.binding.name.clone(),
                json_span(sources, drop_.span),
                vec![drop_.binding.to_json(sources), drop_.body.to_json(sources)],
            ),
        }
    }
}

impl MethodDecl {
    pub(super) fn to_json(&self, sources: &SourceMap) -> JsonAstNode {
        let mut children = vec![
            visibility_json(self.visibility),
            JsonAstNode::with_value(
                "method_receiver",
                self.receiver.mode.label(),
                json_span(sources, self.receiver.span),
                vec![JsonAstNode::with_value(
                    "parameter",
                    self.receiver.name.clone(),
                    json_span(sources, self.receiver.name_span),
                    Vec::new(),
                )],
            ),
            self.parameters.to_json(sources),
            self.return_type.to_json(sources),
        ];
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
            Visibility::Private => "private",
            Visibility::Public => "pub",
            Visibility::Nocter => "pub(nocter)",
        },
        None,
        Vec::new(),
    )
}
