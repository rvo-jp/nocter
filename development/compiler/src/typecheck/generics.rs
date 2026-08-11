use super::diagnostics::{
    duplicate_binder_refinement_diagnostic, duplicate_callable_parameter_name_diagnostic,
    duplicate_copy_requirement_diagnostic, duplicate_generic_bound_diagnostic,
    duplicate_type_equality_diagnostic, generic_bound_not_interface_diagnostic,
    generic_type_argument_count_diagnostic, invalid_callable_provenance_origin_diagnostic,
    invalid_type_equality_diagnostic, multiple_callable_bounds_diagnostic,
    nominal_type_equality_not_satisfied_diagnostic,
    nominal_type_requirement_not_satisfied_diagnostic, recursive_binder_refinement_diagnostic,
    self_type_outside_context_diagnostic, unknown_where_parameter_diagnostic,
    unresolved_type_reference_diagnostic,
};
use super::model::Type;
use super::type_expr::type_expr_to_type_with_substitutions;
use crate::ast::{
    AstFile, Block, ConformanceMember, Expr, GenericParam, GenericParamList,
    InterpolatedStringPart, Item, MethodDecl, Parameter, Stmt, TypeExpr, WhereClause,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::{ByteSpan, SourceMap};
use std::collections::HashMap;

pub(super) fn check_generic_type_arities(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in &ast.items {
        match item {
            Item::Function(function) => {
                let scope = if function.owner.is_some() {
                    GenericScope::new(&function.generics).with_self_type()
                } else {
                    GenericScope::new(&function.generics)
                }
                .with_where_clause(function.requirements.as_ref());
                check_where_clause(
                    sources,
                    function.requirements.as_ref(),
                    resolved,
                    &scope,
                    diagnostics,
                );
                check_parameters(
                    sources,
                    &function.parameters.parameters,
                    resolved,
                    &scope,
                    diagnostics,
                );
                check_type_expr(
                    sources,
                    &function.return_type,
                    resolved,
                    &scope,
                    diagnostics,
                );
                if let Some(body) = &function.body {
                    check_block(sources, body, resolved, &scope, diagnostics);
                }
            }
            Item::Test(test) => {
                let generics = GenericParamList {
                    span: None,
                    parameters: Vec::new(),
                };
                check_block(
                    sources,
                    &test.body,
                    resolved,
                    &GenericScope::new(&generics),
                    diagnostics,
                );
            }
            Item::Primitive(primitive) => {
                let scope = GenericScope::new(&primitive.generics)
                    .with_where_clause(primitive.requirements.as_ref());
                check_where_clause(
                    sources,
                    primitive.requirements.as_ref(),
                    resolved,
                    &scope,
                    diagnostics,
                );
                check_parameters(
                    sources,
                    &primitive.parameters.parameters,
                    resolved,
                    &scope,
                    diagnostics,
                );
                check_type_expr(
                    sources,
                    &primitive.return_type,
                    resolved,
                    &scope,
                    diagnostics,
                );
            }
            Item::TypeAlias(alias) => {
                let scope = GenericScope::new(&alias.generics)
                    .with_where_clause(alias.requirements.as_ref());
                check_where_clause(
                    sources,
                    alias.requirements.as_ref(),
                    resolved,
                    &scope,
                    diagnostics,
                );
                check_type_expr(sources, &alias.target, resolved, &scope, diagnostics);
            }
            Item::Struct(struct_) => {
                let scope = GenericScope::new(&struct_.generics)
                    .with_where_clause(struct_.requirements.as_ref());
                check_where_clause(
                    sources,
                    struct_.requirements.as_ref(),
                    resolved,
                    &scope,
                    diagnostics,
                );
                for field in &struct_.fields {
                    check_type_expr(sources, &field.ty, resolved, &scope, diagnostics);
                }
            }
            Item::Enum(enum_) => {
                let scope = GenericScope::new(&enum_.generics)
                    .with_where_clause(enum_.requirements.as_ref());
                check_where_clause(
                    sources,
                    enum_.requirements.as_ref(),
                    resolved,
                    &scope,
                    diagnostics,
                );
                for variant in &enum_.variants {
                    check_parameters(sources, &variant.payload, resolved, &scope, diagnostics);
                }
            }
            Item::Interface(interface) => {
                let scope = GenericScope::new(&interface.generics)
                    .with_interface(interface)
                    .with_where_clause(interface.requirements.as_ref());
                check_where_clause(
                    sources,
                    interface.requirements.as_ref(),
                    resolved,
                    &scope,
                    diagnostics,
                );
                for associated in &interface.associated_types {
                    check_bound_list(sources, &associated.bounds, resolved, &scope, diagnostics);
                }
                for method in &interface.methods {
                    let method_scope = scope
                        .clone()
                        .with_generics(&method.generics)
                        .with_where_clause(method.requirements.as_ref());
                    check_method_signature(sources, method, resolved, &method_scope, diagnostics);
                    if let Some(body) = &method.body {
                        check_block(sources, body, resolved, &method_scope, diagnostics);
                    }
                }
            }
            Item::Instance(instance) => {
                check_instance_types(sources, instance, resolved, diagnostics)
            }
            Item::Destruct(destruct) => {
                let scope = GenericScope::new(&destruct.generics).with_self_type();
                check_type_expr(sources, &destruct.target_ty, resolved, &scope, diagnostics);
                check_type_expr(sources, &destruct.binding.ty, resolved, &scope, diagnostics);
                check_block(sources, &destruct.body, resolved, &scope, diagnostics);
            }
            Item::Conformance(conformance) => {
                check_conformance_types(sources, conformance, resolved, diagnostics)
            }
            Item::Construct(construct) => {
                for (_, function) in construct.functions() {
                    let scope = GenericScope::new(&function.generics)
                        .with_self_type()
                        .with_where_clause(function.requirements.as_ref());
                    check_where_clause(
                        sources,
                        function.requirements.as_ref(),
                        resolved,
                        &scope,
                        diagnostics,
                    );
                    check_parameters(
                        sources,
                        &function.parameters.parameters,
                        resolved,
                        &scope,
                        diagnostics,
                    );
                    check_type_expr(
                        sources,
                        &function.return_type,
                        resolved,
                        &scope,
                        diagnostics,
                    );
                    if let Some(body) = &function.body {
                        check_block(sources, body, resolved, &scope, diagnostics);
                    }
                }
            }
            Item::Coerce(coerce) => {
                check_instance_types(sources, &coerce.callable_instance(), resolved, diagnostics);
            }
            Item::Import(_) | Item::FromImport(_) => {}
        }
    }
}

fn check_instance_types(
    sources: &SourceMap,
    instance: &crate::ast::InstanceDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let scope =
        GenericScope::new(&instance.generics).with_where_clause(instance.requirements.as_ref());
    check_where_clause(
        sources,
        instance.requirements.as_ref(),
        resolved,
        &scope,
        diagnostics,
    );
    check_type_expr(sources, &instance.target_ty, resolved, &scope, diagnostics);
    let member_scope = scope.clone().with_self_type();
    for method in instance.callable_methods() {
        let method_scope = member_scope
            .clone()
            .with_generics(&method.generics)
            .with_where_clause(method.requirements.as_ref());
        check_method_signature(sources, method, resolved, &method_scope, diagnostics);
        if let Some(body) = &method.body {
            check_block(sources, body, resolved, &method_scope, diagnostics);
        }
    }
}

fn check_conformance_types(
    sources: &SourceMap,
    conformance: &crate::ast::ConformanceDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let scope = GenericScope::new(&conformance.generics)
        .with_where_clause(conformance.requirements.as_ref());
    check_where_clause(
        sources,
        conformance.requirements.as_ref(),
        resolved,
        &scope,
        diagnostics,
    );
    check_type_expr(
        sources,
        &conformance.interface_ty,
        resolved,
        &scope,
        diagnostics,
    );
    check_type_expr(
        sources,
        &conformance.target_ty,
        resolved,
        &scope,
        diagnostics,
    );
    let member_scope = scope
        .clone()
        .with_conformance_interface(conformance, resolved);
    for member in &conformance.members {
        match member {
            ConformanceMember::AssociatedType(binding) => {
                check_type_expr(
                    sources,
                    &binding.value,
                    resolved,
                    &member_scope,
                    diagnostics,
                );
            }
            ConformanceMember::Method(method) => {
                let method_scope = member_scope
                    .clone()
                    .with_generics(&method.generics)
                    .with_where_clause(method.requirements.as_ref());
                check_method_signature(sources, method, resolved, &method_scope, diagnostics);
                if let Some(body) = &method.body {
                    check_block(sources, body, resolved, &method_scope, diagnostics);
                }
            }
        }
    }
}

fn check_bound_list(
    sources: &SourceMap,
    bounds: &[TypeExpr],
    resolved: &ResolveOutput,
    scope: &GenericScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let substitutions = scope
        .parameters
        .keys()
        .map(|name| ((*name).to_string(), Type::Parameter((*name).to_string())))
        .collect();
    let mut seen_bounds = HashMap::<String, ByteSpan>::new();
    let mut callable_bound_span = None;
    for bound in bounds {
        check_type_expr(sources, bound, resolved, scope, diagnostics);
        let bound_type =
            type_expr_to_type_with_substitutions(bound, resolved, None, &substitutions);
        if bound_type.is_unknown_or_unresolved() {
            continue;
        }
        let is_interface_or_callable = matches!(bound_type, Type::Callable(_))
            || bound_type
                .nominal_name()
                .and_then(|name| resolved.type_symbol_by_canonical_name(name))
                .is_some_and(|symbol| symbol.kind == crate::resolve::TypeSymbolKind::Interface);
        if !is_interface_or_callable {
            diagnostics.push(generic_bound_not_interface_diagnostic(
                sources,
                bound,
                &bound_type,
            ));
            continue;
        }
        if matches!(bound_type, Type::Callable(_)) {
            if let Some(first_span) = callable_bound_span {
                diagnostics.push(multiple_callable_bounds_diagnostic(
                    sources,
                    bound.span(),
                    first_span,
                ));
            } else {
                callable_bound_span = Some(bound.span());
            }
        }
        let key = bound_type.display();
        if let Some(first_span) = seen_bounds.insert(key, bound.span()) {
            diagnostics.push(duplicate_generic_bound_diagnostic(
                sources,
                bound,
                &bound_type,
                first_span,
            ));
        }
    }
}

fn check_where_clause(
    sources: &SourceMap,
    clause: Option<&WhereClause>,
    resolved: &ResolveOutput,
    scope: &GenericScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(clause) = clause else {
        return;
    };
    let substitutions = scope
        .parameters
        .keys()
        .map(|name| ((*name).to_string(), Type::Parameter((*name).to_string())))
        .collect();
    let mut seen_copy = HashMap::new();
    let mut seen_bounds = HashMap::<&str, HashMap<String, ByteSpan>>::new();
    let mut callable_bound_spans = HashMap::new();

    for requirement in clause.copy_requirements() {
        if !scope.parameters.contains_key(requirement.name.as_str()) {
            diagnostics.push(unknown_where_parameter_diagnostic(
                sources,
                &requirement.name,
                requirement.name_span,
            ));
            continue;
        }
        if let Some(first_span) =
            seen_copy.insert(requirement.name.as_str(), requirement.keyword_span)
        {
            diagnostics.push(duplicate_copy_requirement_diagnostic(
                sources,
                requirement.keyword_span,
                first_span,
            ));
        }
    }

    for requirement in clause.generic_requirements() {
        let Some(_) = scope.parameters.get(requirement.name.as_str()) else {
            diagnostics.push(unknown_where_parameter_diagnostic(
                sources,
                &requirement.name,
                requirement.name_span,
            ));
            for bound in &requirement.bounds {
                check_type_expr(sources, bound, resolved, scope, diagnostics);
            }
            continue;
        };
        for bound in &requirement.bounds {
            check_type_expr(sources, bound, resolved, scope, diagnostics);
            let bound_type =
                type_expr_to_type_with_substitutions(bound, resolved, None, &substitutions);
            if bound_type.is_unknown_or_unresolved() {
                continue;
            }
            let is_interface_or_callable = matches!(bound_type, Type::Callable(_))
                || bound_type
                    .nominal_name()
                    .and_then(|name| resolved.type_symbol_by_canonical_name(name))
                    .is_some_and(|symbol| symbol.kind == crate::resolve::TypeSymbolKind::Interface);
            if !is_interface_or_callable {
                diagnostics.push(generic_bound_not_interface_diagnostic(
                    sources,
                    bound,
                    &bound_type,
                ));
                continue;
            }
            if matches!(bound_type, Type::Callable(_))
                && let Some(first_span) =
                    callable_bound_spans.insert(requirement.name.as_str(), bound.span())
            {
                diagnostics.push(multiple_callable_bounds_diagnostic(
                    sources,
                    bound.span(),
                    first_span,
                ));
            }
            let parameter_bounds = seen_bounds.entry(requirement.name.as_str()).or_default();
            if let Some(first_span) = parameter_bounds.insert(bound_type.display(), bound.span()) {
                diagnostics.push(duplicate_generic_bound_diagnostic(
                    sources,
                    bound,
                    &bound_type,
                    first_span,
                ));
            }
        }
    }

    let mut seen_refinements = HashMap::new();
    let refinements = clause
        .refinements()
        .map(|refinement| (refinement.name.as_str(), &refinement.value))
        .collect::<HashMap<_, _>>();
    for refinement in clause.refinements() {
        check_type_expr(sources, &refinement.value, resolved, scope, diagnostics);
        if let Some(first_span) = seen_refinements.insert(refinement.name.as_str(), refinement.span)
        {
            diagnostics.push(duplicate_binder_refinement_diagnostic(
                sources,
                &refinement.name,
                refinement.span,
                first_span,
            ));
        }
        if type_expr_mentions_parameter(&refinement.value, &refinement.name)
            || type_expr_mentions_parameter(&refinement.value, "Self")
            || refinement_dependency_reaches(
                &refinement.name,
                &refinement.value,
                &refinements,
                &mut std::collections::HashSet::new(),
            )
        {
            diagnostics.push(recursive_binder_refinement_diagnostic(
                sources,
                &refinement.name,
                refinement.span,
            ));
        }
    }

    let mut seen_equalities = HashMap::<(String, String), ByteSpan>::new();
    for equality in clause.equalities() {
        check_type_expr(sources, &equality.left, resolved, scope, diagnostics);
        check_type_expr(sources, &equality.right, resolved, scope, diagnostics);
        if !type_expr_contains_projection(&equality.left)
            && !type_expr_contains_projection(&equality.right)
        {
            diagnostics.push(invalid_type_equality_diagnostic(
                sources,
                &equality.left,
                &equality.right,
            ));
            continue;
        }
        let left = crate::ast::canonical_type_expr(&equality.left);
        let right = crate::ast::canonical_type_expr(&equality.right);
        let key = if left <= right {
            (left, right)
        } else {
            (right, left)
        };
        if let Some(first_span) = seen_equalities.insert(key, equality.span) {
            diagnostics.push(duplicate_type_equality_diagnostic(
                sources,
                equality.span,
                first_span,
            ));
        }
    }

    let mut seen_operators = HashMap::<String, ByteSpan>::new();
    for requirement in clause.operator_requirements() {
        let valid = match &requirement.shape {
            crate::ast::OperatorRequirementShape::Equality { left, right, .. } => {
                check_type_expr(sources, left, resolved, scope, diagnostics);
                check_type_expr(sources, right, resolved, scope, diagnostics);
                let operands_valid = match (left, right) {
                    (TypeExpr::Borrow(left), TypeExpr::Borrow(right))
                        if !left.is_readwrite && !right.is_readwrite =>
                    {
                        matches!(
                            (left.inner.as_ref(), right.inner.as_ref()),
                            (TypeExpr::Reference(left), TypeExpr::Reference(right))
                                if left.name == right.name
                                    && scope.parameters.contains_key(left.name.as_str())
                        )
                    }
                    _ => false,
                };
                operands_valid
                    && matches!(
                        &requirement.result,
                        TypeExpr::Reference(reference) if reference.name == "bool"
                    )
            }
            crate::ast::OperatorRequirementShape::Index { target, index, .. } => {
                check_type_expr(sources, target, resolved, scope, diagnostics);
                check_type_expr(sources, index, resolved, scope, diagnostics);
                matches!(
                    (target, &requirement.result),
                    (TypeExpr::Borrow(target), TypeExpr::Borrow(result))
                        if target.is_readwrite == result.is_readwrite
                            && matches!(target.inner.as_ref(), TypeExpr::Reference(reference)
                                if scope.parameters.contains_key(reference.name.as_str()))
                )
            }
        };
        check_type_expr(sources, &requirement.result, resolved, scope, diagnostics);
        if !valid {
            let mut diagnostic = Diagnostic::error(
                "E0471",
                match &requirement.shape {
                    crate::ast::OperatorRequirementShape::Equality { .. } => {
                        "equality requirement must have the form `(&T == &T): bool` for one generic parameter"
                    }
                    crate::ast::OperatorRequirementShape::Index { .. } => {
                        "index requirement must borrow a generic target and return a borrow with the same capability, such as `(&C[K]): &V`"
                    }
                },
            );
            diagnostic.primary_span = sources.span_to_json(requirement.span).ok().map(Box::new);
            diagnostics.push(diagnostic);
            continue;
        }
        let key = match &requirement.shape {
            crate::ast::OperatorRequirementShape::Equality { left, right, .. } => format!(
                "{}=={}:{}",
                crate::ast::canonical_type_expr(left),
                crate::ast::canonical_type_expr(right),
                crate::ast::canonical_type_expr(&requirement.result),
            ),
            crate::ast::OperatorRequirementShape::Index { target, index, .. } => format!(
                "{}[{}]:{}",
                crate::ast::canonical_type_expr(target),
                crate::ast::canonical_type_expr(index),
                crate::ast::canonical_type_expr(&requirement.result),
            ),
        };
        if let Some(first_span) = seen_operators.insert(key, requirement.span) {
            let mut diagnostic = Diagnostic::error("E0472", "duplicate operator requirement");
            diagnostic.primary_span = sources.span_to_json(requirement.span).ok().map(Box::new);
            if let Ok(span) = sources.span_to_json(first_span) {
                diagnostic.notes.push(crate::diagnostics::DiagnosticNote {
                    message: "the first requirement is here".to_string(),
                    span: Some(span),
                });
            }
            diagnostics.push(diagnostic);
        }
    }
}

fn refinement_dependency_reaches(
    target: &str,
    value: &TypeExpr,
    refinements: &HashMap<&str, &TypeExpr>,
    visited: &mut std::collections::HashSet<String>,
) -> bool {
    let mut references = Vec::new();
    collect_type_expr_reference_names(value, &mut references);
    references.into_iter().any(|reference| {
        if reference == target {
            return true;
        }
        let Some(next) = refinements.get(reference.as_str()) else {
            return false;
        };
        visited.insert(reference.clone())
            && refinement_dependency_reaches(target, next, refinements, visited)
    })
}

fn collect_type_expr_reference_names(ty: &TypeExpr, names: &mut Vec<String>) {
    match ty {
        TypeExpr::Callable(callable) => {
            for parameter in &callable.parameters {
                collect_type_expr_reference_names(&parameter.ty, names);
            }
            collect_type_expr_reference_names(&callable.return_type, names);
        }
        TypeExpr::Closure(closure) => {
            for capture in &closure.captures {
                collect_type_expr_reference_names(&capture.ty, names);
            }
            for parameter in &closure.parameters {
                collect_type_expr_reference_names(parameter, names);
            }
            collect_type_expr_reference_names(&closure.return_type, names);
        }
        TypeExpr::Opaque(opaque) => {
            collect_type_expr_reference_names(&opaque.interface, names);
            for binding in &opaque.associated_bindings {
                collect_type_expr_reference_names(&binding.value, names);
            }
        }
        TypeExpr::Reference(reference) => names.push(reference.name.clone()),
        TypeExpr::Generic(generic) => {
            for argument in &generic.arguments {
                collect_type_expr_reference_names(argument, names);
            }
        }
        TypeExpr::Projection(projection) => {
            collect_type_expr_reference_names(&projection.base, names)
        }
        TypeExpr::Pointer(pointer) => collect_type_expr_reference_names(&pointer.inner, names),
        TypeExpr::Borrow(borrow) => collect_type_expr_reference_names(&borrow.inner, names),
        TypeExpr::View(view) => collect_type_expr_reference_names(&view.element, names),
        TypeExpr::Array(array) => collect_type_expr_reference_names(&array.element, names),
        TypeExpr::Optional(optional) => collect_type_expr_reference_names(&optional.inner, names),
        TypeExpr::Fallible(fallible) => {
            collect_type_expr_reference_names(&fallible.success, names);
            collect_type_expr_reference_names(&fallible.error, names);
        }
    }
}

fn type_expr_contains_projection(ty: &TypeExpr) -> bool {
    match ty {
        TypeExpr::Projection(_) => true,
        TypeExpr::Callable(callable) => {
            callable
                .parameters
                .iter()
                .any(|parameter| type_expr_contains_projection(&parameter.ty))
                || type_expr_contains_projection(&callable.return_type)
        }
        TypeExpr::Closure(closure) => {
            closure
                .captures
                .iter()
                .any(|capture| type_expr_contains_projection(&capture.ty))
                || closure.parameters.iter().any(type_expr_contains_projection)
                || type_expr_contains_projection(&closure.return_type)
        }
        TypeExpr::Opaque(opaque) => {
            type_expr_contains_projection(&opaque.interface)
                || opaque
                    .associated_bindings
                    .iter()
                    .any(|binding| type_expr_contains_projection(&binding.value))
        }
        TypeExpr::Generic(generic) => generic.arguments.iter().any(type_expr_contains_projection),
        TypeExpr::Pointer(pointer) => type_expr_contains_projection(&pointer.inner),
        TypeExpr::Borrow(borrow) => type_expr_contains_projection(&borrow.inner),
        TypeExpr::View(view) => type_expr_contains_projection(&view.element),
        TypeExpr::Array(array) => type_expr_contains_projection(&array.element),
        TypeExpr::Optional(optional) => type_expr_contains_projection(&optional.inner),
        TypeExpr::Fallible(fallible) => {
            type_expr_contains_projection(&fallible.success)
                || type_expr_contains_projection(&fallible.error)
        }
        TypeExpr::Reference(_) => false,
    }
}

fn type_expr_mentions_parameter(ty: &TypeExpr, parameter: &str) -> bool {
    match ty {
        TypeExpr::Callable(callable) => {
            callable
                .parameters
                .iter()
                .any(|value| type_expr_mentions_parameter(&value.ty, parameter))
                || type_expr_mentions_parameter(&callable.return_type, parameter)
        }
        TypeExpr::Closure(closure) => {
            closure
                .captures
                .iter()
                .any(|value| type_expr_mentions_parameter(&value.ty, parameter))
                || closure
                    .parameters
                    .iter()
                    .any(|value| type_expr_mentions_parameter(value, parameter))
                || type_expr_mentions_parameter(&closure.return_type, parameter)
        }
        TypeExpr::Opaque(opaque) => {
            type_expr_mentions_parameter(&opaque.interface, parameter)
                || opaque
                    .associated_bindings
                    .iter()
                    .any(|binding| type_expr_mentions_parameter(&binding.value, parameter))
        }
        TypeExpr::Reference(reference) => reference.name == parameter,
        TypeExpr::Generic(generic) => generic
            .arguments
            .iter()
            .any(|value| type_expr_mentions_parameter(value, parameter)),
        TypeExpr::Projection(projection) => {
            type_expr_mentions_parameter(&projection.base, parameter)
        }
        TypeExpr::Pointer(pointer) => type_expr_mentions_parameter(&pointer.inner, parameter),
        TypeExpr::Borrow(borrow) => type_expr_mentions_parameter(&borrow.inner, parameter),
        TypeExpr::View(view) => type_expr_mentions_parameter(&view.element, parameter),
        TypeExpr::Array(array) => type_expr_mentions_parameter(&array.element, parameter),
        TypeExpr::Optional(optional) => type_expr_mentions_parameter(&optional.inner, parameter),
        TypeExpr::Fallible(fallible) => {
            type_expr_mentions_parameter(&fallible.success, parameter)
                || type_expr_mentions_parameter(&fallible.error, parameter)
        }
    }
}

#[derive(Debug, Clone)]
struct GenericScope<'a> {
    parameters: HashMap<&'a str, &'a GenericParam>,
    requirements: HashMap<&'a str, Vec<&'a TypeExpr>>,
    clauses: Vec<&'a WhereClause>,
    self_associated_types: Option<HashMap<String, ByteSpan>>,
}

impl<'a> GenericScope<'a> {
    fn new(generics: &'a GenericParamList) -> Self {
        Self {
            parameters: generics
                .parameters
                .iter()
                .map(|parameter| (parameter.name.as_str(), parameter))
                .collect(),
            requirements: generics
                .parameters
                .iter()
                .map(|parameter| (parameter.name.as_str(), Vec::new()))
                .collect(),
            clauses: Vec::new(),
            self_associated_types: None,
        }
    }

    fn with_self_type(mut self) -> Self {
        self.self_associated_types = Some(HashMap::new());
        self
    }

    fn with_interface(mut self, interface: &'a crate::ast::InterfaceDecl) -> Self {
        self.self_associated_types = Some(
            interface
                .associated_types
                .iter()
                .map(|associated| (associated.name.clone(), associated.name_span))
                .collect(),
        );
        self
    }

    fn with_conformance_interface(
        mut self,
        conformance: &crate::ast::ConformanceDecl,
        resolved: &ResolveOutput,
    ) -> Self {
        self.self_associated_types = Some(HashMap::new());
        let name = match &conformance.interface_ty {
            TypeExpr::Reference(reference) => &reference.name,
            TypeExpr::Generic(generic) => &generic.name,
            _ => return self,
        };
        if let Some(interface) = resolved.type_symbol_by_reference_name(name) {
            self.self_associated_types = Some(
                interface
                    .associated_types
                    .iter()
                    .map(|associated| (associated.name.clone(), associated.name_span))
                    .collect(),
            );
        }
        self
    }

    fn with_generics(mut self, generics: &'a GenericParamList) -> Self {
        self.parameters.extend(
            generics
                .parameters
                .iter()
                .map(|parameter| (parameter.name.as_str(), parameter)),
        );
        self.requirements.extend(
            generics
                .parameters
                .iter()
                .map(|parameter| (parameter.name.as_str(), Vec::new())),
        );
        self
    }

    fn with_where_clause(mut self, clause: Option<&'a WhereClause>) -> Self {
        if let Some(clause) = clause {
            self.clauses.push(clause);
            for requirement in clause.generic_requirements() {
                self.requirements
                    .entry(requirement.name.as_str())
                    .or_default()
                    .extend(&requirement.bounds);
            }
        }
        self
    }

    fn allows_self_type(&self) -> bool {
        self.self_associated_types.is_some()
    }

    fn contains(&self, name: &str) -> bool {
        self.parameters.contains_key(name)
    }

    fn parameter_span(&self, name: &str) -> Option<ByteSpan> {
        self.parameters
            .get(name)
            .map(|parameter| parameter.name_span)
    }

    fn type_environment(&self, resolved: &ResolveOutput) -> super::model::TypeEnvironment {
        let mut environment = super::model::TypeEnvironment::default();
        environment
            .define_generic_parameters(self.parameters.keys().map(|name| (*name).to_string()));
        for clause in &self.clauses {
            environment.apply_where_clause(Some(clause), resolved);
        }
        environment
    }
}

fn check_method_signature(
    sources: &SourceMap,
    method: &MethodDecl,
    resolved: &ResolveOutput,
    scope: &GenericScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_where_clause(
        sources,
        method.requirements.as_ref(),
        resolved,
        scope,
        diagnostics,
    );
    check_parameters(
        sources,
        &method.parameters.parameters,
        resolved,
        scope,
        diagnostics,
    );
    check_type_expr(sources, &method.return_type, resolved, scope, diagnostics);
}

fn check_parameters(
    sources: &SourceMap,
    parameters: &[Parameter],
    resolved: &ResolveOutput,
    scope: &GenericScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for parameter in parameters {
        check_type_expr(sources, &parameter.ty, resolved, scope, diagnostics);
    }
}

fn check_type_expr(
    sources: &SourceMap,
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    scope: &GenericScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match ty {
        TypeExpr::Callable(callable) => {
            let mut names = HashMap::new();
            for parameter in &callable.parameters {
                if let (Some(name), Some(span)) = (&parameter.name, parameter.name_span)
                    && let Some(first_span) = names.insert(name.as_str(), span)
                {
                    diagnostics.push(duplicate_callable_parameter_name_diagnostic(
                        sources, name, span, first_span,
                    ));
                }
                check_type_expr(sources, &parameter.ty, resolved, scope, diagnostics);
            }
            check_type_expr(sources, &callable.return_type, resolved, scope, diagnostics);
            if let Some(clause) = &callable.result_provenance {
                for origin in &clause.origins {
                    let valid = match &origin.kind {
                        crate::ast::ResultProvenanceOriginKind::Parameter(name) => callable
                            .parameters
                            .iter()
                            .any(|parameter| parameter.name.as_deref() == Some(name)),
                        crate::ast::ResultProvenanceOriginKind::Static => true,
                        crate::ast::ResultProvenanceOriginKind::Receiver => false,
                    };
                    if !valid {
                        diagnostics.push(invalid_callable_provenance_origin_diagnostic(
                            sources, origin,
                        ));
                    }
                }
            } else {
                let parameters = callable
                    .parameters
                    .iter()
                    .enumerate()
                    .map(|(index, parameter)| crate::resolve::ParameterSignature {
                        name: parameter
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("argument{index}")),
                        name_span: parameter.name_span.unwrap_or(parameter.span),
                        ty: parameter.ty.clone(),
                    })
                    .collect::<Vec<_>>();
                let return_type = crate::typecheck::type_expr::type_expr_to_type_with_substitutions(
                    &callable.return_type,
                    resolved,
                    None,
                    &HashMap::new(),
                );
                if let crate::typecheck::provenance::ElidedResultContract::Ambiguous {
                    labels,
                    ..
                } = crate::typecheck::provenance::elided_signature_result_contract(
                    &parameters,
                    &return_type,
                    resolved,
                ) {
                    diagnostics.push(
                        crate::typecheck::diagnostics::ambiguous_bodyless_result_contract_diagnostic(
                            sources,
                            callable.return_type.span(),
                            &labels,
                        ),
                    );
                }
            }
        }
        TypeExpr::Closure(closure) => {
            for capture in &closure.captures {
                check_type_expr(sources, &capture.ty, resolved, scope, diagnostics);
            }
            for parameter in &closure.parameters {
                check_type_expr(sources, parameter, resolved, scope, diagnostics);
            }
            check_type_expr(sources, &closure.return_type, resolved, scope, diagnostics);
        }
        TypeExpr::Opaque(opaque) => {
            check_type_expr(sources, &opaque.interface, resolved, scope, diagnostics);
            for binding in &opaque.associated_bindings {
                check_type_expr(sources, &binding.value, resolved, scope, diagnostics);
            }
        }
        TypeExpr::Reference(reference) => {
            if reference.name == "Self" {
                if !scope.allows_self_type() {
                    diagnostics.push(self_type_outside_context_diagnostic(
                        sources,
                        reference.span,
                    ));
                }
                return;
            }
            if scope.contains(&reference.name) {
                return;
            }
            if builtin_type_argument_arity(&reference.name).is_some() {
                return;
            }
            match resolved.type_symbol_definition_by_reference_name(&reference.name) {
                Some((symbol, type_symbol)) if type_symbol.generic_arity > 0 => {
                    diagnostics.push(generic_type_argument_count_diagnostic(
                        sources,
                        &reference.name,
                        reference.span,
                        Some(symbol.declaration_span),
                        type_symbol.generic_arity,
                        0,
                    ));
                }
                Some(_) => {}
                None => {
                    diagnostics.push(unresolved_type_reference_diagnostic(
                        sources,
                        &reference.name,
                        reference.span,
                    ));
                }
            }
        }
        TypeExpr::Generic(generic) => {
            if generic.name == "Self" {
                if scope.allows_self_type() {
                    diagnostics.push(generic_type_argument_count_diagnostic(
                        sources,
                        &generic.name,
                        generic.name_span,
                        None,
                        0,
                        generic.arguments.len(),
                    ));
                } else {
                    diagnostics.push(self_type_outside_context_diagnostic(
                        sources,
                        generic.name_span,
                    ));
                }
            } else if builtin_type_argument_arity(&generic.name).is_some() {
                diagnostics.push(generic_type_argument_count_diagnostic(
                    sources,
                    &generic.name,
                    generic.name_span,
                    None,
                    0,
                    generic.arguments.len(),
                ));
            } else if let Some(parameter_span) = scope.parameter_span(&generic.name) {
                diagnostics.push(generic_type_argument_count_diagnostic(
                    sources,
                    &generic.name,
                    generic.name_span,
                    Some(parameter_span),
                    0,
                    generic.arguments.len(),
                ));
            } else {
                match resolved.type_symbol_definition_by_reference_name(&generic.name) {
                    Some((symbol, type_symbol))
                        if type_symbol.generic_arity != generic.arguments.len() =>
                    {
                        diagnostics.push(generic_type_argument_count_diagnostic(
                            sources,
                            &generic.name,
                            generic.name_span,
                            Some(symbol.declaration_span),
                            type_symbol.generic_arity,
                            generic.arguments.len(),
                        ));
                    }
                    Some(_) => {}
                    None => {
                        diagnostics.push(unresolved_type_reference_diagnostic(
                            sources,
                            &generic.name,
                            generic.name_span,
                        ));
                    }
                }
            }
            for argument in &generic.arguments {
                check_type_expr(sources, argument, resolved, scope, diagnostics);
            }
            check_nominal_type_requirements(sources, generic, resolved, scope, diagnostics);
        }
        TypeExpr::Projection(projection) => {
            check_type_expr(sources, &projection.base, resolved, scope, diagnostics);
            let candidates = associated_type_projection_candidates(projection, resolved, scope);
            if candidates.len() != 1 {
                let message = if candidates.is_empty() {
                    format!(
                        "type `{}` has no associated type `{}` in this context",
                        crate::ast::canonical_type_expr(&projection.base),
                        projection.name
                    )
                } else {
                    format!(
                        "associated type projection `{}.{}` is ambiguous between {} interface requirements",
                        crate::ast::canonical_type_expr(&projection.base),
                        projection.name,
                        candidates.len()
                    )
                };
                let mut diagnostic = Diagnostic::error("E0436", message);
                diagnostic.primary_span = sources
                    .span_to_json(projection.name_span)
                    .ok()
                    .map(Box::new);
                for span in candidates {
                    if let Ok(span) = sources.span_to_json(span) {
                        diagnostic.notes.push(crate::diagnostics::DiagnosticNote {
                            message: "candidate associated type is declared here".to_string(),
                            span: Some(span),
                        });
                    }
                }
                diagnostic.help = Some(
                    "project an associated type supplied by one unambiguous interface contract"
                        .to_string(),
                );
                diagnostics.push(diagnostic);
            }
        }
        TypeExpr::Pointer(pointer) => {
            check_type_expr(sources, &pointer.inner, resolved, scope, diagnostics)
        }
        TypeExpr::Borrow(borrow) => {
            check_type_expr(sources, &borrow.inner, resolved, scope, diagnostics)
        }
        TypeExpr::View(view) => {
            check_type_expr(sources, &view.element, resolved, scope, diagnostics)
        }
        TypeExpr::Array(array) => {
            check_type_expr(sources, &array.element, resolved, scope, diagnostics)
        }
        TypeExpr::Optional(optional) => {
            check_type_expr(sources, &optional.inner, resolved, scope, diagnostics)
        }
        TypeExpr::Fallible(fallible) => {
            check_type_expr(sources, &fallible.success, resolved, scope, diagnostics);
            check_type_expr(sources, &fallible.error, resolved, scope, diagnostics);
        }
    }
}

fn check_nominal_type_requirements(
    sources: &SourceMap,
    generic: &crate::ast::GenericType,
    resolved: &ResolveOutput,
    scope: &GenericScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(symbol) = resolved.type_symbol_by_reference_name(&generic.name) else {
        return;
    };
    if symbol.generic_parameters.len() != generic.arguments.len() {
        return;
    }

    let scope_substitutions = scope
        .parameters
        .keys()
        .map(|name| ((*name).to_string(), Type::Parameter((*name).to_string())))
        .collect::<HashMap<_, _>>();
    let arguments = generic
        .arguments
        .iter()
        .map(|argument| {
            type_expr_to_type_with_substitutions(argument, resolved, None, &scope_substitutions)
        })
        .collect::<Vec<_>>();
    if arguments.iter().any(Type::is_unknown_or_unresolved) {
        return;
    }
    let substitutions = symbol
        .generic_parameters
        .iter()
        .cloned()
        .zip(arguments.iter().cloned())
        .collect::<HashMap<_, _>>();
    let environment = scope.type_environment(resolved);

    for ((parameter, requirements), (argument_expr, actual)) in symbol
        .generic_parameters
        .iter()
        .zip(&symbol.generic_parameter_requirements)
        .zip(generic.arguments.iter().zip(&arguments))
    {
        if let Some(requirement_span) = requirements.copy_span()
            && !super::copyability::type_is_copy_in_environment(actual, resolved, &environment)
        {
            diagnostics.push(nominal_type_requirement_not_satisfied_diagnostic(
                sources,
                argument_expr.span(),
                &symbol.canonical_name,
                actual,
                &format!("copy {parameter}"),
                requirement_span,
            ));
        }
        for bound in requirements.type_bounds() {
            let expected =
                type_expr_to_type_with_substitutions(bound, resolved, None, &substitutions);
            if expected.is_unknown_or_unresolved()
                || type_satisfies_nominal_requirement(
                    actual,
                    &expected,
                    resolved,
                    &environment,
                    &scope_substitutions,
                )
            {
                continue;
            }
            diagnostics.push(nominal_type_requirement_not_satisfied_diagnostic(
                sources,
                argument_expr.span(),
                &symbol.canonical_name,
                actual,
                &expected.display(),
                bound.span(),
            ));
        }
    }

    if let Some(clause) = &symbol.where_clause {
        for equality in clause.equalities() {
            let left = type_expr_to_type_with_substitutions(
                &equality.left,
                resolved,
                None,
                &substitutions,
            );
            let right = type_expr_to_type_with_substitutions(
                &equality.right,
                resolved,
                None,
                &substitutions,
            );
            if left.is_unknown_or_unresolved()
                || right.is_unknown_or_unresolved()
                || environment.types_equal(&left, &right)
            {
                continue;
            }
            diagnostics.push(nominal_type_equality_not_satisfied_diagnostic(
                sources,
                generic.span,
                &symbol.canonical_name,
                &left,
                &right,
                equality.span,
            ));
        }
    }
}

fn type_satisfies_nominal_requirement(
    actual: &Type,
    expected: &Type,
    resolved: &ResolveOutput,
    environment: &super::model::TypeEnvironment,
    substitutions: &HashMap<String, Type>,
) -> bool {
    if super::conformance::type_satisfies_interface_bound(actual, expected, resolved) {
        return true;
    }
    let Type::Parameter(name) = actual else {
        return false;
    };
    let Some(requirements) = environment.generic_requirements(name) else {
        return false;
    };
    requirements.type_bounds().any(|bound| {
        let actual_bound =
            type_expr_to_type_with_substitutions(bound, resolved, None, substitutions);
        actual_bound == *expected
            || super::conformance::type_satisfies_interface_bound(&actual_bound, expected, resolved)
    })
}

fn associated_type_projection_candidates(
    projection: &crate::ast::ProjectedType,
    resolved: &ResolveOutput,
    scope: &GenericScope<'_>,
) -> Vec<ByteSpan> {
    if let TypeExpr::Reference(reference) = projection.base.as_ref() {
        if reference.name == "Self" {
            return scope
                .self_associated_types
                .as_ref()
                .and_then(|types| types.get(&projection.name))
                .copied()
                .into_iter()
                .collect();
        }
        if let Some(bounds) = scope.requirements.get(reference.name.as_str()) {
            let mut candidates = bounds
                .iter()
                .filter_map(|bound| {
                    let name = match bound {
                        TypeExpr::Reference(reference) => &reference.name,
                        TypeExpr::Generic(generic) => &generic.name,
                        _ => return None,
                    };
                    let interface = resolved.type_symbol_by_reference_name(name)?;
                    (interface.kind == crate::resolve::TypeSymbolKind::Interface)
                        .then_some(interface)?
                        .associated_types
                        .iter()
                        .find(|associated| associated.name == projection.name)
                        .map(|associated| associated.name_span)
                })
                .collect::<Vec<_>>();
            candidates.sort_by_key(|span| (span.source.raw(), span.start, span.end));
            candidates.dedup();
            return candidates;
        }
    }

    if let TypeExpr::Projection(base_projection) = projection.base.as_ref() {
        let mut candidates =
            associated_type_signatures_for_projection(base_projection, resolved, scope)
                .into_iter()
                .flat_map(|associated| associated.requirements.type_bounds())
                .filter_map(|bound| {
                    let name = match bound {
                        TypeExpr::Reference(reference) => &reference.name,
                        TypeExpr::Generic(generic) => &generic.name,
                        _ => return None,
                    };
                    resolved
                        .type_symbol_by_reference_name(name)?
                        .associated_types
                        .iter()
                        .find(|associated| associated.name == projection.name)
                        .map(|associated| associated.name_span)
                })
                .collect::<Vec<_>>();
        candidates.sort_by_key(|span| (span.source.raw(), span.start, span.end));
        candidates.dedup();
        if !candidates.is_empty() {
            return candidates;
        }
    }

    let substitutions = scope
        .parameters
        .keys()
        .map(|name| ((*name).to_string(), Type::Parameter((*name).to_string())))
        .collect::<HashMap<_, _>>();
    let projected = type_expr_to_type_with_substitutions(
        &TypeExpr::Projection(projection.clone()),
        resolved,
        None,
        &substitutions,
    );
    if matches!(
        projected,
        Type::Projection { .. } | Type::Unresolved(_) | Type::Unknown
    ) {
        Vec::new()
    } else {
        vec![projection.name_span]
    }
}

fn associated_type_signatures_for_projection<'a>(
    projection: &crate::ast::ProjectedType,
    resolved: &'a ResolveOutput,
    scope: &GenericScope<'_>,
) -> Vec<&'a crate::resolve::AssociatedTypeSignature> {
    let TypeExpr::Reference(reference) = projection.base.as_ref() else {
        return Vec::new();
    };
    let Some(bounds) = scope.requirements.get(reference.name.as_str()) else {
        return Vec::new();
    };
    bounds
        .iter()
        .filter_map(|bound| {
            let name = match bound {
                TypeExpr::Reference(reference) => &reference.name,
                TypeExpr::Generic(generic) => &generic.name,
                _ => return None,
            };
            resolved
                .type_symbol_by_reference_name(name)?
                .associated_types
                .iter()
                .find(|associated| associated.name == projection.name)
        })
        .collect()
}

fn builtin_type_argument_arity(name: &str) -> Option<usize> {
    matches!(
        name,
        "bool"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "usize"
            | "isize"
            | "str"
            | "error"
            | "void"
            | "never"
    )
    .then_some(0)
}

fn check_block(
    sources: &SourceMap,
    block: &Block,
    resolved: &ResolveOutput,
    scope: &GenericScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for statement in &block.statements {
        check_statement(sources, statement, resolved, scope, diagnostics);
    }
    if let Some(result) = &block.result {
        check_expression(sources, result, resolved, scope, diagnostics);
    }
}

fn check_statement(
    sources: &SourceMap,
    statement: &Stmt,
    resolved: &ResolveOutput,
    scope: &GenericScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => {}
        Stmt::Binding(statement) => {
            if let Some(ty) = &statement.ty {
                check_type_expr(sources, ty, resolved, scope, diagnostics);
            }
            check_expression(
                sources,
                &statement.initializer,
                resolved,
                scope,
                diagnostics,
            );
        }
        Stmt::Assignment(statement) => {
            check_expression(sources, &statement.target, resolved, scope, diagnostics);
            check_expression(sources, &statement.value, resolved, scope, diagnostics);
        }
        Stmt::If(statement) => {
            check_expression(sources, &statement.condition, resolved, scope, diagnostics);
            check_block(sources, &statement.then_block, resolved, scope, diagnostics);
            if let Some(else_block) = &statement.else_block {
                check_block(sources, else_block, resolved, scope, diagnostics);
            }
        }
        Stmt::IfIs(statement) => {
            check_expression(sources, &statement.expression, resolved, scope, diagnostics);
            check_block(sources, &statement.then_block, resolved, scope, diagnostics);
            if let Some(else_block) = &statement.else_block {
                check_block(sources, else_block, resolved, scope, diagnostics);
            }
        }
        Stmt::Switch(statement) => {
            check_expression(sources, &statement.expression, resolved, scope, diagnostics);
            for arm in &statement.arms {
                check_block(sources, &arm.body, resolved, scope, diagnostics);
            }
            if let Some(wildcard_arm) = &statement.wildcard_arm {
                check_block(sources, &wildcard_arm.body, resolved, scope, diagnostics);
            }
        }
        Stmt::ForRange(statement) => {
            check_expression(sources, &statement.start, resolved, scope, diagnostics);
            check_expression(sources, &statement.end, resolved, scope, diagnostics);
            check_block(sources, &statement.body, resolved, scope, diagnostics);
        }
        Stmt::CollectionFor(statement) => {
            check_expression(sources, &statement.source, resolved, scope, diagnostics);
            check_block(sources, &statement.body, resolved, scope, diagnostics);
        }
        Stmt::LiteralPackFor(statement) => {
            check_block(sources, &statement.body, resolved, scope, diagnostics);
        }
        Stmt::While(statement) => {
            check_expression(sources, &statement.condition, resolved, scope, diagnostics);
            check_block(sources, &statement.body, resolved, scope, diagnostics);
        }
        Stmt::Loop(statement) => {
            check_block(sources, &statement.body, resolved, scope, diagnostics)
        }
        Stmt::Region(statement) => {
            check_expression(sources, &statement.allocator, resolved, scope, diagnostics);
            check_block(sources, &statement.body, resolved, scope, diagnostics);
        }
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression {
                check_expression(sources, expression, resolved, scope, diagnostics);
            }
        }
        Stmt::Expression(statement) => {
            check_expression(sources, &statement.expression, resolved, scope, diagnostics);
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Drop(_) => {}
    }
}

fn check_expression(
    sources: &SourceMap,
    expression: &Expr,
    resolved: &ResolveOutput,
    scope: &GenericScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expression {
        Expr::Closure(closure) => {
            for parameter in &closure.parameters {
                if let Some(ty) = &parameter.ty {
                    check_type_expr(sources, ty, resolved, scope, diagnostics);
                }
            }
            if let Some(ty) = &closure.return_type {
                check_type_expr(sources, ty, resolved, scope, diagnostics);
            }
            check_block(sources, &closure.body, resolved, scope, diagnostics);
        }
        Expr::InterpolatedString(expression) => {
            for part in &expression.parts {
                if let InterpolatedStringPart::Expression(part) = part {
                    check_expression(sources, &part.expression, resolved, scope, diagnostics);
                }
            }
        }
        Expr::ArrayLiteral(expression) => {
            for element in &expression.elements {
                check_expression(sources, element, resolved, scope, diagnostics);
            }
        }
        Expr::TypedSequenceLiteral(expression) => {
            if matches!(expression.target, TypeExpr::Generic(_)) {
                check_type_expr(sources, &expression.target, resolved, scope, diagnostics);
            }
            for element in &expression.elements {
                check_expression(sources, element, resolved, scope, diagnostics);
            }
            if let Some(using) = &expression.using {
                check_expression(sources, &using.allocator, resolved, scope, diagnostics);
            }
        }
        Expr::TypedStringLiteral(expression) => {
            if matches!(expression.target, TypeExpr::Generic(_)) {
                check_type_expr(sources, &expression.target, resolved, scope, diagnostics);
            }
            if let Some(using) = &expression.using {
                check_expression(sources, &using.allocator, resolved, scope, diagnostics);
            }
        }
        Expr::StructLiteral(expression) => {
            check_type_expr(sources, &expression.ty, resolved, scope, diagnostics);
            for field in &expression.fields {
                check_expression(sources, &field.value, resolved, scope, diagnostics);
            }
        }
        Expr::Propagate(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                scope,
                diagnostics,
            );
        }
        Expr::Force(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                scope,
                diagnostics,
            );
        }
        Expr::Catch(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                scope,
                diagnostics,
            );
            check_block(
                sources,
                &expression.catch_block,
                resolved,
                scope,
                diagnostics,
            );
        }
        Expr::Borrow(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                scope,
                diagnostics,
            );
        }
        Expr::Unary(expression) => {
            check_expression(sources, &expression.operand, resolved, scope, diagnostics);
        }
        Expr::Binary(expression) => {
            check_expression(sources, &expression.left, resolved, scope, diagnostics);
            check_expression(sources, &expression.right, resolved, scope, diagnostics);
        }
        Expr::TypeConversion(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                scope,
                diagnostics,
            );
            check_type_expr(sources, &expression.ty, resolved, scope, diagnostics);
        }
        Expr::Call(expression) => {
            check_expression(sources, &expression.callee, resolved, scope, diagnostics);
            for argument in &expression.arguments {
                check_expression(sources, argument, resolved, scope, diagnostics);
            }
        }
        Expr::Member(expression) => {
            check_expression(sources, &expression.object, resolved, scope, diagnostics);
        }
        Expr::Index(expression) => {
            check_expression(sources, &expression.object, resolved, scope, diagnostics);
            check_expression(sources, &expression.index, resolved, scope, diagnostics);
        }
        Expr::Group(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                scope,
                diagnostics,
            );
        }
        Expr::Otherwise(expression) => {
            check_expression(sources, &expression.value, resolved, scope, diagnostics);
            check_block(sources, &expression.fallback, resolved, scope, diagnostics);
        }
        Expr::If(expression) => {
            check_expression(sources, &expression.condition, resolved, scope, diagnostics);
            check_block(
                sources,
                &expression.then_block,
                resolved,
                scope,
                diagnostics,
            );
            if let Some(block) = &expression.else_block {
                check_block(sources, block, resolved, scope, diagnostics);
            }
        }
        Expr::IfIs(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                scope,
                diagnostics,
            );
            check_block(
                sources,
                &expression.then_block,
                resolved,
                scope,
                diagnostics,
            );
            if let Some(block) = &expression.else_block {
                check_block(sources, block, resolved, scope, diagnostics);
            }
        }
        Expr::Match(expression) => {
            check_expression(
                sources,
                &expression.expression,
                resolved,
                scope,
                diagnostics,
            );
            for arm in &expression.arms {
                check_block(sources, &arm.body, resolved, scope, diagnostics);
            }
            if let Some(arm) = &expression.wildcard_arm {
                check_block(sources, &arm.body, resolved, scope, diagnostics);
            }
        }
        Expr::Identifier(_)
        | Expr::IntegerLiteral(_)
        | Expr::ByteLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BoolLiteral(_)
        | Expr::NoneLiteral(_) => {}
    }
}
