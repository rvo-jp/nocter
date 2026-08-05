use crate::ast::{
    BindingKind, CallableCapability, ClosureTypeExpr, ResultProvenanceClause, TypeExpr,
};
use crate::source::ByteSpan;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Type {
    Callable(CallableType),
    Closure(ClosureTypeExpr),
    I32,
    Primitive(String),
    StrData,
    Str,
    Error,
    Void,
    Never,
    None,
    ArrayData {
        element: Box<Type>,
    },
    View {
        is_readwrite: bool,
        element: Box<Type>,
    },
    Array {
        element: Box<Type>,
        length: String,
    },
    Pointer(Box<Type>),
    Optional(Box<Type>),
    Fallible {
        success: Box<Type>,
        error: Box<Type>,
    },
    Named(String),
    Generic {
        name: String,
        arguments: Vec<Type>,
    },
    Parameter(String),
    Unresolved(String),
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CallableType {
    pub(super) span: ByteSpan,
    pub(super) capability: CallableCapability,
    pub(super) parameters: Vec<CallableParameterType>,
    pub(super) return_type: Box<Type>,
    pub(super) result_provenance: Option<ResultProvenanceClause>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CallableParameterType {
    pub(super) name: Option<String>,
    pub(super) name_span: Option<ByteSpan>,
    pub(super) ty: Type,
}

impl Type {
    pub(super) fn substitute_parameters(&self, substitutions: &HashMap<String, Type>) -> Type {
        match self {
            Type::Parameter(parameter) => substitutions
                .get(parameter)
                .cloned()
                .unwrap_or_else(|| self.clone()),
            Type::Callable(callable) => Type::Callable(CallableType {
                span: callable.span,
                capability: callable.capability,
                parameters: callable
                    .parameters
                    .iter()
                    .map(|parameter| CallableParameterType {
                        name: parameter.name.clone(),
                        name_span: parameter.name_span,
                        ty: parameter.ty.substitute_parameters(substitutions),
                    })
                    .collect(),
                return_type: Box::new(callable.return_type.substitute_parameters(substitutions)),
                result_provenance: callable.result_provenance.clone(),
            }),
            Type::ArrayData { element } => Type::ArrayData {
                element: Box::new(element.substitute_parameters(substitutions)),
            },
            Type::View {
                is_readwrite,
                element,
            } => Type::View {
                is_readwrite: *is_readwrite,
                element: Box::new(element.substitute_parameters(substitutions)),
            },
            Type::Array { element, length } => Type::Array {
                element: Box::new(element.substitute_parameters(substitutions)),
                length: length.clone(),
            },
            Type::Pointer(inner) => {
                Type::Pointer(Box::new(inner.substitute_parameters(substitutions)))
            }
            Type::Optional(inner) => {
                Type::Optional(Box::new(inner.substitute_parameters(substitutions)))
            }
            Type::Fallible { success, error } => Type::Fallible {
                success: Box::new(success.substitute_parameters(substitutions)),
                error: Box::new(error.substitute_parameters(substitutions)),
            },
            Type::Generic { name, arguments } => Type::Generic {
                name: name.clone(),
                arguments: arguments
                    .iter()
                    .map(|argument| argument.substitute_parameters(substitutions))
                    .collect(),
            },
            Type::Closure(_)
            | Type::I32
            | Type::Primitive(_)
            | Type::StrData
            | Type::Str
            | Type::Error
            | Type::Void
            | Type::Never
            | Type::None
            | Type::Named(_)
            | Type::Unresolved(_)
            | Type::Unknown => self.clone(),
        }
    }

    pub(super) fn display(&self) -> String {
        match self {
            Type::Callable(callable) => callable.display(),
            Type::Closure(closure) => closure.identity_name(),
            Type::I32 => "i32".to_string(),
            Type::Primitive(name) => name.clone(),
            Type::StrData => "str".to_string(),
            Type::Str => "&str".to_string(),
            Type::Error => "error".to_string(),
            Type::Void => "void".to_string(),
            Type::Never => "never".to_string(),
            Type::None => "none".to_string(),
            Type::ArrayData { element } => format!("[{}]", element.display()),
            Type::View {
                is_readwrite: true,
                element,
            } => format!("&+[{}]", element.display()),
            Type::View {
                is_readwrite: false,
                element,
            } => format!("&[{}]", element.display()),
            Type::Array { element, length } => format!("[{}; {}]", element.display(), length),
            Type::Pointer(inner) => format!("*{}", inner.display()),
            Type::Optional(inner) => format!("{}?", inner.display()),
            Type::Fallible { success, .. } => format!("{}!", success.display()),
            Type::Named(name) => name.clone(),
            Type::Generic { name, arguments } => {
                let arguments = arguments
                    .iter()
                    .map(Type::display)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name}<{arguments}>")
            }
            Type::Parameter(name) => name.clone(),
            Type::Unresolved(name) => name.clone(),
            Type::Unknown => "<unknown>".to_string(),
        }
    }

    pub(super) fn nominal_name(&self) -> Option<&str> {
        match self {
            Type::Callable(_) | Type::Closure(_) => None,
            Type::Named(name) | Type::Generic { name, .. } => Some(name),
            _ => None,
        }
    }

    pub(super) fn is_unknown(&self) -> bool {
        matches!(self, Type::Unknown)
    }

    pub(super) fn is_unknown_or_unresolved(&self) -> bool {
        match self {
            Type::Callable(_) | Type::Closure(_) => false,
            Type::Unknown | Type::Unresolved(_) => true,
            Type::ArrayData { element } => element.is_unknown_or_unresolved(),
            Type::View { element, .. } => element.is_unknown_or_unresolved(),
            Type::Array { element, .. } => element.is_unknown_or_unresolved(),
            Type::Pointer(inner) => inner.is_unknown_or_unresolved(),
            Type::Optional(inner) => inner.is_unknown_or_unresolved(),
            Type::Generic { arguments, .. } => arguments.iter().any(Type::is_unknown_or_unresolved),
            Type::Fallible { success, error } => {
                success.is_unknown_or_unresolved() || error.is_unknown_or_unresolved()
            }
            Type::I32
            | Type::Primitive(_)
            | Type::StrData
            | Type::Str
            | Type::Error
            | Type::Void
            | Type::Never
            | Type::None
            | Type::Named(_)
            | Type::Parameter(_) => false,
        }
    }

    pub(super) fn first_unsized_part(&self) -> Option<&Type> {
        match self {
            Type::Callable(_) | Type::Closure(_) => None,
            Type::StrData | Type::ArrayData { .. } => Some(self),
            Type::View { element, .. } | Type::Array { element, .. } => {
                element.first_unsized_part()
            }
            Type::Pointer(_) => None,
            Type::Optional(inner) => inner.first_unsized_part(),
            Type::Generic { arguments, .. } => arguments.iter().find_map(Type::first_unsized_part),
            Type::Fallible { success, error } => success
                .first_unsized_part()
                .or_else(|| error.first_unsized_part()),
            Type::I32
            | Type::Primitive(_)
            | Type::Str
            | Type::Error
            | Type::Void
            | Type::Never
            | Type::None
            | Type::Named(_)
            | Type::Parameter(_)
            | Type::Unresolved(_)
            | Type::Unknown => None,
        }
    }

    pub(super) fn success_type(&self) -> &Type {
        match self {
            Type::Fallible { success, .. } => success,
            _ => self,
        }
    }

    pub(super) fn into_fallible_success_type(self) -> Type {
        match self {
            Type::Fallible { success, .. } => *success,
            Type::Unknown | Type::Unresolved(_) => Type::Unknown,
            _ => Type::Unknown,
        }
    }

    pub(super) fn into_propagated_type(self) -> Type {
        match self {
            Type::Fallible { success, .. } => *success,
            Type::Optional(inner) => *inner,
            Type::Unknown | Type::Unresolved(_) => Type::Unknown,
            _ => Type::Unknown,
        }
    }
}

impl CallableType {
    fn display(&self) -> String {
        let parameters = self
            .parameters
            .iter()
            .map(|parameter| {
                let ty = parameter.ty.display();
                parameter
                    .name
                    .as_ref()
                    .map_or(ty.clone(), |name| format!("{name}: {ty}"))
            })
            .collect::<Vec<_>>()
            .join(", ");
        let provenance = self
            .result_provenance
            .as_ref()
            .map_or_else(String::new, |clause| {
                format!(
                    " from {}",
                    clause
                        .origins
                        .iter()
                        .map(|origin| origin.kind.source_label())
                        .collect::<Vec<_>>()
                        .join(" | ")
                )
            });
        format!(
            "{}func({parameters}): {}{provenance}",
            self.capability.source_prefix(),
            self.return_type.display()
        )
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct TypeEnvironment {
    bindings: HashMap<String, TypeBinding>,
    literal_packs: HashMap<String, Type>,
    self_type: Option<Type>,
    generic_parameters: HashSet<String>,
    generic_bounds: HashMap<String, Vec<TypeExpr>>,
}

impl TypeEnvironment {
    pub(super) fn with_self_type(self_type: Type) -> Self {
        Self {
            bindings: HashMap::new(),
            literal_packs: HashMap::new(),
            self_type: Some(self_type),
            generic_parameters: HashSet::new(),
            generic_bounds: HashMap::new(),
        }
    }

    /// Starts a nested callable scope without leaking the enclosing callable's
    /// locals. Generic declarations and `Self` remain visible because they are
    /// lexical type scope, while captures are reintroduced explicitly.
    pub(super) fn nested_callable_scope(&self) -> Self {
        Self {
            bindings: HashMap::new(),
            literal_packs: HashMap::new(),
            self_type: self.self_type.clone(),
            generic_parameters: self.generic_parameters.clone(),
            generic_bounds: self.generic_bounds.clone(),
        }
    }

    pub(super) fn define(&mut self, name: String, ty: Type) {
        self.define_binding(name, ty, false);
    }

    pub(super) fn define_binding(&mut self, name: String, ty: Type, is_mutable: bool) {
        self.bindings.insert(name, TypeBinding { ty, is_mutable });
    }

    pub(super) fn define_literal_pack(&mut self, name: String, element_type: Type) {
        self.literal_packs.insert(name, element_type);
    }

    pub(super) fn literal_pack_element(&self, name: &str) -> Option<&Type> {
        self.literal_packs.get(name)
    }

    pub(super) fn define_generic_parameters(&mut self, names: impl IntoIterator<Item = String>) {
        self.generic_parameters.extend(names);
    }

    pub(super) fn define_generic_parameter_list(
        &mut self,
        generics: &crate::ast::GenericParamList,
    ) {
        for parameter in &generics.parameters {
            self.generic_parameters.insert(parameter.name.clone());
            if !parameter.bounds.is_empty() {
                self.generic_bounds
                    .insert(parameter.name.clone(), parameter.bounds.clone());
            }
        }
    }

    pub(super) fn define_generic_parameter(&mut self, name: String, bounds: Vec<TypeExpr>) {
        self.generic_parameters.insert(name.clone());
        if !bounds.is_empty() {
            self.generic_bounds.insert(name, bounds);
        }
    }

    pub(super) fn generic_bounds(&self, name: &str) -> Option<&[TypeExpr]> {
        self.generic_bounds.get(name).map(Vec::as_slice)
    }

    pub(super) fn get(&self, name: &str) -> Option<&Type> {
        self.bindings.get(name).map(|binding| &binding.ty)
    }

    pub(super) fn is_mutable_binding(&self, name: &str) -> bool {
        self.bindings
            .get(name)
            .is_some_and(|binding| binding.is_mutable)
    }

    pub(super) fn self_type(&self) -> Option<&Type> {
        self.self_type.as_ref()
    }

    pub(super) fn generic_parameter_substitutions(&self) -> HashMap<String, Type> {
        self.generic_parameters
            .iter()
            .map(|name| (name.clone(), Type::Parameter(name.clone())))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TypeBinding {
    ty: Type,
    is_mutable: bool,
}

pub(super) fn same_known_type(left: &Type, right: &Type) -> bool {
    !left.is_unknown() && !right.is_unknown() && left == right
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReturnContext {
    pub(super) kind: CallableKind,
    pub(super) declared_type: Type,
    pub(super) return_type_span: ByteSpan,
}

impl ReturnContext {
    pub(super) fn new(kind: CallableKind, declared_type: Type, return_type_span: ByteSpan) -> Self {
        Self {
            kind,
            declared_type,
            return_type_span,
        }
    }

    pub(super) fn success_type(&self) -> &Type {
        self.declared_type.success_type()
    }

    pub(super) fn requires_explicit_return(&self) -> bool {
        let success_type = self.success_type();
        !matches!(
            success_type,
            Type::Void | Type::Unknown | Type::Unresolved(_)
        )
    }

    pub(super) fn subject(&self) -> String {
        match &self.kind {
            CallableKind::Function(name) => format!("function `{name}`"),
            CallableKind::AssociatedFunction(name) => format!("associated function `{name}`"),
            CallableKind::Method(name) => format!("method `{name}`"),
            CallableKind::Drop(name) => format!("drop member `{name}`"),
            CallableKind::Literal(name) => format!("literal definition for `{name}`"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CallableKind {
    Function(String),
    AssociatedFunction(String),
    Method(String),
    Drop(String),
    Literal(String),
}

pub(super) fn binding_kind_is_mutable(kind: BindingKind) -> bool {
    matches!(kind, BindingKind::Var)
}
