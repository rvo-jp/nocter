use crate::ast::TypeExpr;
use crate::resolve::{
    AssociatedFunctionSignature, DropSignature, FunctionSignature, LiteralSignature,
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
    ) -> Self {
        Self {
            kind: kind.into(),
            name: name.into(),
            generics,
            parameters,
            return_type: return_type.into(),
            result_origins,
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
        format!(
            "{} {}{generics}({}): {}{origins}",
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
    let generics = signature
        .generic_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let Some(bounds) = signature.generic_parameter_bounds.get(index) else {
                return parameter.clone();
            };
            if bounds.is_empty() {
                return parameter.clone();
            }
            format!(
                "{parameter}: {}",
                bounds
                    .iter()
                    .map(|bound| type_expr_presentation_label(bound, resolved))
                    .collect::<Vec<_>>()
                    .join(" + ")
            )
        })
        .collect();
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

pub(crate) fn drop_presentation(drop_: &DropSignature, resolved: &ResolveOutput) -> String {
    let binding = match self_receiver_prefix(&drop_.binding.ty) {
        Some(prefix) => format!("{prefix}{}", drop_.binding.name),
        None => format!(
            "{}: {}",
            drop_.binding.name,
            type_expr_presentation_label(&drop_.binding.ty, resolved)
        ),
    };
    format!("drop {binding}")
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

fn self_receiver_prefix(ty: &TypeExpr) -> Option<&'static str> {
    match ty {
        TypeExpr::Reference(reference) if reference.name == "Self" => Some(""),
        TypeExpr::Borrow(borrow) if matches!(borrow.inner.as_ref(), TypeExpr::Reference(reference) if reference.name == "Self") => {
            Some(if borrow.is_readwrite { "&+" } else { "&" })
        }
        _ => None,
    }
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
    for bounds in &mut specialized.generic_parameter_bounds {
        for bound in bounds {
            *bound = crate::ast::substitute_type_expr_parameters(bound, &substitutions);
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
    let bound_split = owner_generic_count.min(signature.generic_parameter_bounds.len());
    signature.generic_parameter_bounds.drain(..bound_split);
    signature
}
