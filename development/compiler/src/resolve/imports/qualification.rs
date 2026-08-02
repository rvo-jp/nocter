use super::*;

pub(super) fn filter_importable_symbol_for_access(
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
        for literal in &mut symbol.literals {
            literal.is_accessible =
                literal.is_accessible && visibility_is_visible_to(literal.visibility, access);
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

pub(super) fn qualify_imported_symbol(
    mut imported: ImportableSymbol,
    import_path: &str,
    imported_name: &str,
) -> ImportableSymbol {
    match &mut imported.kind {
        SymbolKind::Function(signature) | SymbolKind::Primitive(signature) => {
            qualify_function_signature(
                signature,
                import_path,
                &imported.local_type_names,
                &imported.imported_type_names,
            );
        }
        SymbolKind::Type(symbol) => {
            if symbol.canonical_name == imported_name {
                symbol.canonical_name = format!("{import_path}.{imported_name}");
            }
            qualify_type_symbol(
                symbol,
                import_path,
                &imported.local_type_names,
                &imported.imported_type_names,
            );
        }
        SymbolKind::Imported(_) => {}
    }

    imported
}

fn qualify_type_symbol(
    symbol: &mut TypeSymbol,
    import_path: &str,
    local_type_names: &[String],
    imported_type_names: &[ImportedTypeName],
) {
    if let Some(target) = &mut symbol.alias_target {
        qualify_type_expr(target, import_path, local_type_names, imported_type_names);
    }
    for field in &mut symbol.fields {
        qualify_type_expr(
            &mut field.ty,
            import_path,
            local_type_names,
            imported_type_names,
        );
    }
    for variant in &mut symbol.variants {
        for parameter in &mut variant.payload {
            qualify_parameter_signature(
                parameter,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
    }
    for function in &mut symbol.associated_functions {
        qualify_function_signature(
            &mut function.signature,
            import_path,
            local_type_names,
            imported_type_names,
        );
    }
    for method in &mut symbol.methods {
        if let Some(impl_target_ty) = &mut method.impl_target_ty {
            qualify_type_expr(
                impl_target_ty,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
        qualify_parameter_signature(
            &mut method.receiver,
            import_path,
            local_type_names,
            imported_type_names,
        );
        qualify_function_signature(
            &mut method.signature,
            import_path,
            local_type_names,
            imported_type_names,
        );
    }
    for literal in &mut symbol.literals {
        if let Some(capture) = &mut literal.capture {
            qualify_type_expr(
                &mut capture.element_type,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
        for parameter in &mut literal.parameters {
            qualify_parameter_signature(
                parameter,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
        qualify_type_expr(
            &mut literal.return_type,
            import_path,
            local_type_names,
            imported_type_names,
        );
    }
    if let Some(drop_member) = &mut symbol.drop_member {
        qualify_parameter_signature(
            &mut drop_member.binding,
            import_path,
            local_type_names,
            imported_type_names,
        );
    }
}

fn qualify_function_signature(
    signature: &mut FunctionSignature,
    import_path: &str,
    local_type_names: &[String],
    imported_type_names: &[ImportedTypeName],
) {
    for parameter in &mut signature.parameters {
        qualify_parameter_signature(
            parameter,
            import_path,
            local_type_names,
            imported_type_names,
        );
    }
    qualify_type_expr(
        &mut signature.return_type,
        import_path,
        local_type_names,
        imported_type_names,
    );
}

fn qualify_parameter_signature(
    parameter: &mut ParameterSignature,
    import_path: &str,
    local_type_names: &[String],
    imported_type_names: &[ImportedTypeName],
) {
    qualify_type_expr(
        &mut parameter.ty,
        import_path,
        local_type_names,
        imported_type_names,
    );
}

fn qualify_type_expr(
    ty: &mut TypeExpr,
    import_path: &str,
    local_type_names: &[String],
    imported_type_names: &[ImportedTypeName],
) {
    match ty {
        TypeExpr::Reference(reference) => {
            if local_type_names.iter().any(|name| name == &reference.name) {
                reference.name = format!("{import_path}.{}", reference.name);
            } else if let Some(imported) = imported_type_names
                .iter()
                .find(|imported| imported.local_name == reference.name)
            {
                reference.name = imported.qualified_name();
            }
        }
        TypeExpr::Generic(generic) => {
            if local_type_names.iter().any(|name| name == &generic.name) {
                generic.name = format!("{import_path}.{}", generic.name);
            } else if let Some(imported) = imported_type_names
                .iter()
                .find(|imported| imported.local_name == generic.name)
            {
                generic.name = imported.qualified_name();
            }
            for argument in &mut generic.arguments {
                qualify_type_expr(argument, import_path, local_type_names, imported_type_names);
            }
        }
        TypeExpr::Pointer(pointer) => {
            qualify_type_expr(
                &mut pointer.inner,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
        TypeExpr::Borrow(borrow) => {
            qualify_type_expr(
                &mut borrow.inner,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
        TypeExpr::View(view) => {
            qualify_type_expr(
                &mut view.element,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
        TypeExpr::Array(array) => {
            qualify_type_expr(
                &mut array.element,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
        TypeExpr::Optional(optional) => {
            qualify_type_expr(
                &mut optional.inner,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
        TypeExpr::Fallible(fallible) => {
            qualify_type_expr(
                &mut fallible.success,
                import_path,
                local_type_names,
                imported_type_names,
            );
            qualify_type_expr(
                &mut fallible.error,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
    }
}
