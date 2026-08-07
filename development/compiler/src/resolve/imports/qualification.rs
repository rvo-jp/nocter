use super::*;
use crate::resolve::ConstructionEntryKind;

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
        for coercion in &mut symbol.coercions {
            coercion.is_accessible =
                coercion.is_accessible && visibility_is_visible_to(coercion.visibility, access);
        }
        refresh_construction_access(symbol);
    }

    imported
}

fn refresh_construction_access(symbol: &mut TypeSymbol) {
    let structural_accessible = symbol.fields.iter().all(|field| field.is_accessible);
    for entry in &mut symbol.construction.entries {
        entry.is_accessible = match &entry.kind {
            ConstructionEntryKind::Structural => entry.is_accessible && structural_accessible,
            ConstructionEntryKind::Function(name) => symbol
                .associated_functions
                .iter()
                .find(|function| function.name == *name)
                .is_some_and(|function| function.is_accessible),
            ConstructionEntryKind::Literal(shape) => symbol
                .literals
                .iter()
                .find(|literal| literal.shape == *shape)
                .is_some_and(|literal| literal.is_accessible),
            ConstructionEntryKind::Variant(name) => {
                symbol.variants.iter().any(|variant| variant.name == *name)
            }
        };
    }
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
        qualify_function_signature(
            &mut method.signature,
            import_path,
            local_type_names,
            imported_type_names,
        );
    }
    for conformance in &mut symbol.interface_conformances {
        qualify_type_expr(
            &mut conformance.interface_ty,
            import_path,
            local_type_names,
            imported_type_names,
        );
        qualify_type_expr(
            &mut conformance.target_ty,
            import_path,
            local_type_names,
            imported_type_names,
        );
        for bounds in &mut conformance.generic_parameter_bounds {
            for bound in bounds {
                qualify_type_expr(bound, import_path, local_type_names, imported_type_names);
            }
        }
        for method in &mut conformance.methods {
            if let Some(impl_target_ty) = &mut method.impl_target_ty {
                qualify_type_expr(
                    impl_target_ty,
                    import_path,
                    local_type_names,
                    imported_type_names,
                );
            }
            qualify_function_signature(
                &mut method.signature,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
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
    for coercion in &mut symbol.coercions {
        qualify_type_expr(
            &mut coercion.target,
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
    for bounds in &mut signature.generic_parameter_bounds {
        for bound in bounds {
            qualify_type_expr(bound, import_path, local_type_names, imported_type_names);
        }
    }
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
        TypeExpr::Callable(callable) => {
            for parameter in &mut callable.parameters {
                qualify_type_expr(
                    &mut parameter.ty,
                    import_path,
                    local_type_names,
                    imported_type_names,
                );
            }
            qualify_type_expr(
                &mut callable.return_type,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
        TypeExpr::Closure(closure) => {
            for capture in &mut closure.captures {
                qualify_type_expr(
                    &mut capture.ty,
                    import_path,
                    local_type_names,
                    imported_type_names,
                );
            }
            for parameter in &mut closure.parameters {
                qualify_type_expr(
                    parameter,
                    import_path,
                    local_type_names,
                    imported_type_names,
                );
            }
            qualify_type_expr(
                &mut closure.return_type,
                import_path,
                local_type_names,
                imported_type_names,
            );
        }
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
