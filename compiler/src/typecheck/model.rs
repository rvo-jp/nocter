use crate::ast::BindingKind;
use crate::source::ByteSpan;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Type {
    I32,
    Primitive(String),
    Str,
    Error,
    Void,
    Never,
    None,
    View {
        is_readwrite: bool,
        element: Box<Type>,
    },
    Array {
        element: Box<Type>,
        length: String,
    },
    Optional(Box<Type>),
    Fallible {
        success: Box<Type>,
        error: Box<Type>,
    },
    Named(String),
    Unresolved(String),
    Unknown,
}

impl Type {
    pub(super) fn display(&self) -> String {
        match self {
            Type::I32 => "i32".to_string(),
            Type::Primitive(name) => name.clone(),
            Type::Str => "str".to_string(),
            Type::Error => "error".to_string(),
            Type::Void => "void".to_string(),
            Type::Never => "never".to_string(),
            Type::None => "none".to_string(),
            Type::View {
                is_readwrite: true,
                element,
            } => format!("[+{}]", element.display()),
            Type::View {
                is_readwrite: false,
                element,
            } => format!("[{}]", element.display()),
            Type::Array { element, length } => format!("[{}; {}]", element.display(), length),
            Type::Optional(inner) => format!("{}?", inner.display()),
            Type::Fallible { success, .. } => format!("{}!", success.display()),
            Type::Named(name) => name.clone(),
            Type::Unresolved(name) => name.clone(),
            Type::Unknown => "<unknown>".to_string(),
        }
    }

    pub(super) fn is_unknown(&self) -> bool {
        matches!(self, Type::Unknown)
    }

    pub(super) fn is_unknown_or_unresolved(&self) -> bool {
        match self {
            Type::Unknown | Type::Unresolved(_) => true,
            Type::View { element, .. } => element.is_unknown_or_unresolved(),
            Type::Array { element, .. } => element.is_unknown_or_unresolved(),
            Type::Optional(inner) => inner.is_unknown_or_unresolved(),
            Type::Fallible { success, error } => {
                success.is_unknown_or_unresolved() || error.is_unknown_or_unresolved()
            }
            Type::I32
            | Type::Primitive(_)
            | Type::Str
            | Type::Error
            | Type::Void
            | Type::Never
            | Type::None
            | Type::Named(_) => false,
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
    self_type: Option<Type>,
}

impl TypeEnvironment {
    pub(super) fn with_self_type(self_type: Type) -> Self {
        Self {
            bindings: HashMap::new(),
            self_type: Some(self_type),
        }
    }

    pub(super) fn define(&mut self, name: String, ty: Type) {
        self.define_binding(name, ty, false);
    }

    pub(super) fn define_binding(&mut self, name: String, ty: Type, is_mutable: bool) {
        self.bindings.insert(name, TypeBinding { ty, is_mutable });
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
            CallableKind::Program => "`program`".to_string(),
            CallableKind::Function(name) => format!("function `{name}`"),
            CallableKind::AssociatedFunction(name) => format!("associated function `{name}`"),
            CallableKind::Method(name) => format!("method `{name}`"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CallableKind {
    Program,
    Function(String),
    AssociatedFunction(String),
    Method(String),
}

pub(super) fn binding_kind_is_mutable(kind: BindingKind) -> bool {
    matches!(kind, BindingKind::Var)
}
