use crate::ast::{BindingKind, TypeExpr};
use crate::source::ByteSpan;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Type {
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

impl Type {
    pub(super) fn display(&self) -> String {
        match self {
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
            Type::Named(name) | Type::Generic { name, .. } => Some(name),
            _ => None,
        }
    }

    pub(super) fn is_unknown(&self) -> bool {
        matches!(self, Type::Unknown)
    }

    pub(super) fn is_unknown_or_unresolved(&self) -> bool {
        match self {
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

#[derive(Debug, Clone, Default)]
pub(super) struct TypeEnvironment {
    bindings: HashMap<String, TypeBinding>,
    literal_packs: HashMap<String, Type>,
    self_type: Option<Type>,
    generic_parameters: HashSet<String>,
    generic_bounds: HashMap<String, TypeExpr>,
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
            if let Some(bound) = &parameter.bound {
                self.generic_bounds
                    .insert(parameter.name.clone(), bound.clone());
            }
        }
    }

    pub(super) fn generic_bound(&self, name: &str) -> Option<&TypeExpr> {
        self.generic_bounds.get(name)
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
