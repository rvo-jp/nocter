use crate::ast::TypeExpr;
use crate::resolve::{
    AssociatedFunctionSignature, DestructSignature, FunctionSignature, LiteralSignature,
    MethodSignature, ResolveOutput, TypeSymbol, TypeSymbolKind,
};
use crate::typecheck::type_expr_presentation_label;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CallablePresentation {
    kind: String,
    name: String,
    generics: Vec<String>,
    parameters: Vec<String>,
    return_type: String,
    result_origins: Vec<String>,
    requirements: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiteralPresentation {
    target: String,
    shape: &'static str,
    parameters: Vec<String>,
    return_type: String,
    result_origins: Vec<String>,
}

impl LiteralPresentation {
    pub(crate) fn new(
        target: impl Into<String>,
        shape: &'static str,
        parameters: Vec<String>,
        return_type: impl Into<String>,
        result_origins: Vec<String>,
    ) -> Self {
        Self {
            target: target.into(),
            shape,
            parameters,
            return_type: return_type.into(),
            result_origins,
        }
    }

    pub(crate) fn render(&self) -> String {
        let origins = if self.result_origins.is_empty() {
            String::new()
        } else {
            format!(" from {}", self.result_origins.join(" | "))
        };
        format!(
            "literal {} {}({}): {}{origins}",
            self.target,
            self.shape,
            self.parameters.join(", "),
            self.return_type,
        )
    }
}

impl CallablePresentation {
    pub(crate) fn new(
        kind: impl Into<String>,
        name: impl Into<String>,
        generics: Vec<String>,
        parameters: Vec<String>,
        return_type: impl Into<String>,
        result_origins: Vec<String>,
        requirements: Vec<String>,
    ) -> Self {
        Self {
            kind: kind.into(),
            name: name.into(),
            generics,
            parameters,
            return_type: return_type.into(),
            result_origins,
            requirements,
        }
    }

    pub(crate) fn render(&self) -> String {
        let generics = if self.generics.is_empty() {
            String::new()
        } else {
            format!("<{}>", self.generics.join(", "))
        };
        let origins = if self.result_origins.is_empty() {
            String::new()
        } else {
            format!(" from {}", self.result_origins.join(" | "))
        };
        let requirements = if self.requirements.is_empty() {
            String::new()
        } else {
            format!(" where {}", self.requirements.join(", "))
        };
        format!(
            "{} {}{generics}({}): {}{origins}{requirements}",
            self.kind,
            self.name,
            self.parameters.join(", "),
            self.return_type
        )
    }
}

pub(crate) fn callable_signature_presentation(
    kind: &str,
    name: &str,
    signature: &FunctionSignature,
    resolved: &ResolveOutput,
) -> CallablePresentation {
    let generics = signature.generic_parameters.clone();
    let parameters = signature
        .parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name,
                type_expr_presentation_label(&parameter.ty, resolved)
            )
        })
        .collect();
    CallablePresentation::new(
        kind,
        name,
        generics,
        parameters,
        type_expr_presentation_label(&signature.return_type, resolved),
        result_origin_labels(signature.result_provenance.as_ref()),
        where_predicate_labels(signature.where_clause.as_ref(), resolved),
    )
}

pub(crate) fn associated_function_presentation(
    owner: &TypeSymbol,
    function: &AssociatedFunctionSignature,
    resolved: &ResolveOutput,
) -> CallablePresentation {
    let owner_label = super::type_owner_presentation_label(owner, resolved);
    let owner_generic_count =
        if crate::analysis::constructions::construction_owns_function(owner, &function.name) {
            owner.generic_parameters.len()
        } else {
            0
        };
    let signature = signature_with_owner_type(&function.signature, owner, owner_generic_count);
    callable_signature_presentation(
        "func",
        &format!("{owner_label}.{}", function.name),
        &signature,
        resolved,
    )
}

pub(crate) fn method_presentation(
    owner: &TypeSymbol,
    method: &MethodSignature,
    resolved: &ResolveOutput,
) -> CallablePresentation {
    let concrete_owner = owner.kind != TypeSymbolKind::Interface;
    let owner_label = if concrete_owner {
        super::type_owner_presentation_label(owner, resolved)
    } else {
        "Self".to_string()
    };
    let signature = if concrete_owner {
        signature_with_owner_type(&method.signature, owner, owner.generic_parameters.len())
    } else {
        signature_without_owner_generics(&method.signature, owner.generic_parameters.len())
    };
    callable_signature_presentation(
        "method",
        &format!(
            "{}{owner_label}.{}",
            method.receiver.mode.source_prefix(),
            method.name
        ),
        &signature,
        resolved,
    )
}

pub(crate) fn method_or_operator_presentation(
    owner: &TypeSymbol,
    method: &MethodSignature,
    resolved: &ResolveOutput,
) -> String {
    if !crate::ast::is_operator_method_name(&method.name) {
        return method_presentation(owner, method, resolved).render();
    }
    let owner = super::type_owner_presentation_label(owner, resolved);
    if matches!(
        method.name.as_str(),
        crate::ast::READONLY_INDEX_OPERATOR_METHOD_NAME
            | crate::ast::READWRITE_INDEX_OPERATOR_METHOD_NAME
    ) {
        let parameter = method.signature.parameters.first();
        let name = parameter
            .map(|parameter| parameter.name.as_str())
            .unwrap_or("index");
        let index_type = parameter
            .map(|parameter| type_expr_presentation_label(&parameter.ty, resolved))
            .unwrap_or_else(|| "?".to_string());
        let result = type_expr_presentation_label(&method.signature.return_type, resolved);
        return format!(
            "operator ({}{owner}[{name}: {index_type}]): {result}",
            method.receiver.mode.source_prefix(),
        );
    }
    let other = method
        .signature
        .parameters
        .first()
        .map(|parameter| parameter.name.as_str())
        .unwrap_or("other");
    format!("operator (&{owner} == {other}: &{owner}): bool")
}

pub(crate) fn method_presentation_with_substitutions(
    method: &MethodSignature,
    substitutions: &std::collections::HashMap<String, TypeExpr>,
    resolved: &ResolveOutput,
) -> CallablePresentation {
    let mut substitutions = substitutions.clone();
    if let Some(owner_target) = &method.owner_target_ty {
        let owner_target =
            crate::ast::substitute_type_expr_parameters(owner_target, &substitutions);
        substitutions.insert("Self".to_string(), owner_target);
    }
    let owner = substitutions
        .get("Self")
        .map(|ty| type_expr_presentation_label(ty, resolved))
        .unwrap_or_else(|| "Self".to_string());
    let generics = method
        .signature
        .generic_parameters
        .iter()
        .skip(method.owner_generic_count)
        .map(|parameter| {
            if let Some(argument) = substitutions.get(parameter) {
                return type_expr_presentation_label(argument, resolved);
            }
            parameter.clone()
        })
        .collect();
    let parameters = method
        .signature
        .parameters
        .iter()
        .map(|parameter| {
            let ty = crate::ast::substitute_type_expr_parameters(&parameter.ty, &substitutions);
            format!(
                "{}: {}",
                parameter.name,
                type_expr_presentation_label(&ty, resolved)
            )
        })
        .collect();
    let return_type =
        crate::ast::substitute_type_expr_parameters(&method.signature.return_type, &substitutions);
    CallablePresentation::new(
        "method",
        format!(
            "{}{owner}.{}",
            method.receiver.mode.source_prefix(),
            method.name
        ),
        generics,
        parameters,
        type_expr_presentation_label(&return_type, resolved),
        result_origin_labels(method.signature.result_provenance.as_ref()),
        where_predicate_labels_with_substitutions(
            method.signature.where_clause.as_ref(),
            &substitutions,
            resolved,
        ),
    )
}

pub(crate) fn where_predicate_labels(
    clause: Option<&crate::ast::WhereClause>,
    resolved: &ResolveOutput,
) -> Vec<String> {
    where_predicate_labels_with(clause, |ty| type_expr_presentation_label(ty, resolved))
}

pub(crate) fn canonical_where_predicate_labels(
    clause: Option<&crate::ast::WhereClause>,
) -> Vec<String> {
    where_predicate_labels_with(clause, crate::ast::canonical_type_expr)
}

fn where_predicate_labels_with_substitutions(
    clause: Option<&crate::ast::WhereClause>,
    substitutions: &std::collections::HashMap<String, TypeExpr>,
    resolved: &ResolveOutput,
) -> Vec<String> {
    where_predicate_labels_with(clause, |ty| {
        let ty = crate::ast::substitute_type_expr_parameters(ty, substitutions);
        type_expr_presentation_label(&ty, resolved)
    })
}

fn where_predicate_labels_with(
    clause: Option<&crate::ast::WhereClause>,
    type_label: impl Fn(&TypeExpr) -> String,
) -> Vec<String> {
    clause
        .into_iter()
        .flat_map(|clause| &clause.predicates)
        .map(|predicate| match predicate {
            crate::ast::WherePredicate::Copy(requirement) => {
                format!("copy {}", requirement.name)
            }
            crate::ast::WherePredicate::Generic(requirement) => {
                let bounds = requirement
                    .bounds
                    .iter()
                    .map(&type_label)
                    .collect::<Vec<_>>();
                format!("{}: {}", requirement.name, bounds.join(" + "))
            }
            crate::ast::WherePredicate::Refinement(refinement) => {
                format!("{} = {}", refinement.name, type_label(&refinement.value))
            }
            crate::ast::WherePredicate::Equality(equality) => format!(
                "{} = {}",
                type_label(&equality.left),
                type_label(&equality.right)
            ),
            crate::ast::WherePredicate::Operator(requirement) => {
                let operands = match &requirement.shape {
                    crate::ast::OperatorRequirementShape::Equality { left, right, .. } => {
                        format!("{} == {}", type_label(left), type_label(right))
                    }
                    crate::ast::OperatorRequirementShape::Index { target, index, .. } => {
                        format!("{}[{}]", type_label(target), type_label(index))
                    }
                };
                format!("({operands}): {}", type_label(&requirement.result))
            }
        })
        .collect()
}

pub(crate) fn drop_presentation(
    owner: &TypeSymbol,
    _drop: &DestructSignature,
    resolved: &ResolveOutput,
) -> String {
    format!(
        "destruct {}(&+self)",
        crate::typecheck::type_symbol_presentation_label(owner, resolved)
    )
}

pub(crate) fn literal_signature_presentation(
    owner: &TypeSymbol,
    literal: &LiteralSignature,
    resolved: &ResolveOutput,
) -> LiteralPresentation {
    let owner_type = owner_type_expr(owner, literal.return_type.span());
    let substitutions = std::collections::HashMap::from([("Self".to_string(), owner_type)]);
    literal_presentation_with_substitutions(owner, literal, &substitutions, resolved)
}

pub(crate) fn literal_presentation_with_substitutions(
    owner: &TypeSymbol,
    literal: &LiteralSignature,
    substitutions: &std::collections::HashMap<String, TypeExpr>,
    resolved: &ResolveOutput,
) -> LiteralPresentation {
    let parameters = if let Some(capture) = &literal.capture {
        let ty = crate::ast::substitute_type_expr_parameters(&capture.element_type, substitutions);
        vec![format!(
            "...{}: {}",
            capture.name,
            type_expr_presentation_label(&ty, resolved)
        )]
    } else {
        literal
            .parameters
            .iter()
            .map(|parameter| {
                let ty = crate::ast::substitute_type_expr_parameters(&parameter.ty, substitutions);
                format!(
                    "{}: {}",
                    parameter.name,
                    type_expr_presentation_label(&ty, resolved)
                )
            })
            .collect()
    };
    let return_type =
        crate::ast::substitute_type_expr_parameters(&literal.return_type, substitutions);
    LiteralPresentation::new(
        substitutions
            .get("Self")
            .map(|ty| type_expr_presentation_label(ty, resolved))
            .unwrap_or_else(|| super::type_owner_presentation_label(owner, resolved)),
        match literal.shape {
            crate::ast::LiteralShape::Sequence => "[]",
            crate::ast::LiteralShape::String => "\"\"",
        },
        parameters,
        type_expr_presentation_label(&return_type, resolved),
        result_origin_labels(literal.result_provenance.as_ref()),
    )
}

pub(crate) fn result_origin_labels(
    clause: Option<&crate::ast::ResultProvenanceClause>,
) -> Vec<String> {
    clause
        .into_iter()
        .flat_map(|clause| &clause.origins)
        .map(|origin| origin.kind.source_label().to_string())
        .collect()
}

fn signature_with_owner_type(
    signature: &FunctionSignature,
    owner: &TypeSymbol,
    owner_generic_count: usize,
) -> FunctionSignature {
    let owner_type = owner_type_expr(owner, signature.return_type.span());
    let substitutions = std::collections::HashMap::from([("Self".to_string(), owner_type)]);
    let mut specialized = signature_without_owner_generics(signature, owner_generic_count);
    for requirements in &mut specialized.generic_parameter_requirements {
        for requirement in requirements.iter_mut() {
            if let Some(bound) = requirement.type_expr_mut() {
                *bound = crate::ast::substitute_type_expr_parameters(bound, &substitutions);
            }
        }
    }
    for parameter in &mut specialized.parameters {
        parameter.ty = crate::ast::substitute_type_expr_parameters(&parameter.ty, &substitutions);
    }
    specialized.return_type =
        crate::ast::substitute_type_expr_parameters(&specialized.return_type, &substitutions);
    specialized
}

pub(crate) fn owner_type_expr(owner: &TypeSymbol, span: crate::source::ByteSpan) -> TypeExpr {
    if owner.generic_parameters.is_empty() {
        TypeExpr::Reference(crate::ast::TypeReference {
            span,
            name: owner.canonical_name.clone(),
        })
    } else {
        TypeExpr::Generic(crate::ast::GenericType {
            span,
            name: owner.canonical_name.clone(),
            name_span: span,
            arguments: owner
                .generic_parameters
                .iter()
                .map(|parameter| {
                    TypeExpr::Reference(crate::ast::TypeReference {
                        span,
                        name: parameter.clone(),
                    })
                })
                .collect(),
        })
    }
}

fn signature_without_owner_generics(
    signature: &FunctionSignature,
    owner_generic_count: usize,
) -> FunctionSignature {
    let mut signature = signature.clone();
    let split = owner_generic_count.min(signature.generic_parameters.len());
    signature.generic_parameters.drain(..split);
    let bound_split = owner_generic_count.min(signature.generic_parameter_requirements.len());
    signature
        .generic_parameter_requirements
        .drain(..bound_split);
    signature
}
