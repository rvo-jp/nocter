use crate::ast::{BindingKind, CallableCapability, ClosureTypeExpr, ResultProvenanceClause};
use crate::source::ByteSpan;
use crate::type_notation::{PostfixOperator, PrefixOperator, TypeNotation, TypeNotationParameter};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Type {
    Callable(CallableType),
    Closure(ClosureTypeExpr),
    Opaque(OpaqueType),
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
    Borrow {
        is_readwrite: bool,
        inner: Box<Type>,
    },
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
    Projection {
        base: Box<Type>,
        member: String,
    },
    Parameter(String),
    Unresolved(String),
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OpaqueType {
    pub(super) identity: ByteSpan,
    pub(super) interface: Box<Type>,
    pub(super) associated_bindings: Vec<(String, Type)>,
    pub(super) witness: Option<Box<Type>>,
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
    /// Returns the concrete representation used only by static lowering and
    /// implementation specialization. Ordinary type checking must retain the
    /// opaque public identity and interface surface.
    pub(super) fn opaque_lowering_view(&self) -> &Type {
        match self {
            Type::Opaque(opaque) => opaque
                .witness
                .as_deref()
                .map(Type::opaque_lowering_view)
                .unwrap_or(self),
            _ => self,
        }
    }

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
            Type::Opaque(opaque) => Type::Opaque(OpaqueType {
                identity: opaque.identity,
                interface: Box::new(opaque.interface.substitute_parameters(substitutions)),
                associated_bindings: opaque
                    .associated_bindings
                    .iter()
                    .map(|(name, ty)| (name.clone(), ty.substitute_parameters(substitutions)))
                    .collect(),
                witness: opaque
                    .witness
                    .as_ref()
                    .map(|witness| Box::new(witness.substitute_parameters(substitutions))),
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
            Type::Borrow {
                is_readwrite,
                inner,
            } => Type::Borrow {
                is_readwrite: *is_readwrite,
                inner: Box::new(inner.substitute_parameters(substitutions)),
            },
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
            Type::Projection { base, member } => Type::Projection {
                base: Box::new(base.substitute_parameters(substitutions)),
                member: member.clone(),
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
        self.notation_with_name(&str::to_string).render()
    }

    pub(super) fn notation_with_name(
        &self,
        display_name: &impl Fn(&str) -> String,
    ) -> TypeNotation {
        match self {
            Type::Callable(callable) => TypeNotation::Callable {
                capability_prefix: callable.capability.source_prefix(),
                parameters: callable
                    .parameters
                    .iter()
                    .map(|parameter| TypeNotationParameter {
                        name: parameter.name.clone(),
                        ty: parameter.ty.notation_with_name(display_name),
                    })
                    .collect(),
                return_type: Box::new(callable.return_type.notation_with_name(display_name)),
                provenance: callable
                    .result_provenance
                    .iter()
                    .flat_map(|clause| clause.origins.iter())
                    .map(|origin| origin.kind.source_label().to_string())
                    .collect(),
            },
            Type::Closure(closure) => atom(&closure.identity_name()),
            Type::Opaque(opaque) => {
                let (interface_name, interface_arguments) = match opaque.interface.as_ref() {
                    Type::Named(name) => (display_name(name), Vec::new()),
                    Type::Generic { name, arguments } => (
                        display_name(name),
                        arguments
                            .iter()
                            .map(|argument| argument.notation_with_name(display_name))
                            .collect(),
                    ),
                    other => (other.notation_with_name(display_name).render(), Vec::new()),
                };
                TypeNotation::Opaque {
                    interface_name,
                    interface_arguments,
                    associated_bindings: opaque
                        .associated_bindings
                        .iter()
                        .map(|(name, ty)| (name.clone(), ty.notation_with_name(display_name)))
                        .collect(),
                }
            }
            Type::I32 => atom("i32"),
            Type::Primitive(name) => atom(name),
            Type::StrData => atom("str"),
            Type::Str => prefix(PrefixOperator::ReadonlyBorrow, atom("str")),
            Type::Error => atom("error"),
            Type::Void => atom("void"),
            Type::Never => atom("never"),
            Type::None => atom("none"),
            Type::ArrayData { element } => {
                TypeNotation::View(Box::new(element.notation_with_name(display_name)))
            }
            Type::View {
                is_readwrite,
                element,
            } => prefix(
                if *is_readwrite {
                    PrefixOperator::ReadwriteBorrow
                } else {
                    PrefixOperator::ReadonlyBorrow
                },
                TypeNotation::View(Box::new(element.notation_with_name(display_name))),
            ),
            Type::Array { element, length } => TypeNotation::Array {
                element: Box::new(element.notation_with_name(display_name)),
                length: length.clone(),
            },
            Type::Pointer(inner) => prefix(
                PrefixOperator::Pointer,
                inner.notation_with_name(display_name),
            ),
            Type::Borrow {
                is_readwrite,
                inner,
            } => prefix(
                if *is_readwrite {
                    PrefixOperator::ReadwriteBorrow
                } else {
                    PrefixOperator::ReadonlyBorrow
                },
                inner.notation_with_name(display_name),
            ),
            Type::Optional(inner) => postfix(
                PostfixOperator::Optional,
                inner.notation_with_name(display_name),
            ),
            Type::Fallible { success, .. } => postfix(
                PostfixOperator::Fallible,
                success.notation_with_name(display_name),
            ),
            Type::Named(name) => atom(&display_name(name)),
            Type::Generic { name, arguments } => TypeNotation::Generic {
                name: display_name(name),
                arguments: arguments
                    .iter()
                    .map(|argument| argument.notation_with_name(display_name))
                    .collect(),
            },
            Type::Projection { base, member } => TypeNotation::Projection {
                base: Box::new(base.notation_with_name(display_name)),
                member: member.clone(),
            },
            Type::Parameter(name) | Type::Unresolved(name) => atom(name),
            Type::Unknown => atom("<unknown>"),
        }
    }

    pub(super) fn nominal_name(&self) -> Option<&str> {
        match self {
            Type::Callable(_) | Type::Closure(_) | Type::Opaque(_) | Type::Projection { .. } => {
                None
            }
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
            Type::Opaque(opaque) => {
                opaque.interface.is_unknown_or_unresolved()
                    || opaque
                        .associated_bindings
                        .iter()
                        .any(|(_, ty)| ty.is_unknown_or_unresolved())
                    || opaque
                        .witness
                        .as_ref()
                        .is_some_and(|witness| witness.is_unknown_or_unresolved())
            }
            Type::Unknown | Type::Unresolved(_) => true,
            Type::ArrayData { element } => element.is_unknown_or_unresolved(),
            Type::View { element, .. } => element.is_unknown_or_unresolved(),
            Type::Array { element, .. } => element.is_unknown_or_unresolved(),
            Type::Pointer(inner) | Type::Borrow { inner, .. } => inner.is_unknown_or_unresolved(),
            Type::Optional(inner) => inner.is_unknown_or_unresolved(),
            Type::Generic { arguments, .. } => arguments.iter().any(Type::is_unknown_or_unresolved),
            Type::Projection { base, .. } => base.is_unknown_or_unresolved(),
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
            Type::Opaque(opaque) => opaque.witness.as_deref().and_then(Type::first_unsized_part),
            Type::StrData | Type::ArrayData { .. } => Some(self),
            Type::View { element, .. } | Type::Array { element, .. } => {
                element.first_unsized_part()
            }
            Type::Pointer(_) | Type::Borrow { .. } => None,
            Type::Optional(inner) => inner.first_unsized_part(),
            Type::Generic { arguments, .. } => arguments.iter().find_map(Type::first_unsized_part),
            Type::Projection { base, .. } => base.first_unsized_part(),
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

fn atom(name: &str) -> TypeNotation {
    TypeNotation::Atom(name.to_string())
}

fn prefix(operator: PrefixOperator, inner: TypeNotation) -> TypeNotation {
    TypeNotation::Prefix {
        operator,
        inner: Box::new(inner),
    }
}

fn postfix(operator: PostfixOperator, inner: TypeNotation) -> TypeNotation {
    TypeNotation::Postfix {
        inner: Box::new(inner),
        operator,
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct TypeEnvironment {
    bindings: HashMap<String, TypeBinding>,
    literal_packs: HashMap<String, Type>,
    self_type: Option<Type>,
    generic_parameters: HashSet<String>,
    generic_requirements: HashMap<String, crate::resolve::GenericRequirements>,
    type_equalities: Vec<(Type, Type)>,
    equality_requirements: Vec<(Type, Type, crate::source::ByteSpan)>,
    index_requirements: Vec<IndexRequirement>,
    expansion_requirements: Vec<ExpansionRequirement>,
}

#[derive(Debug, Clone)]
pub(super) struct IndexRequirement {
    pub(super) target: Type,
    pub(super) index: Type,
    pub(super) element: Type,
    pub(super) is_readwrite: bool,
    pub(super) span: crate::source::ByteSpan,
}

#[derive(Debug, Clone)]
pub(super) struct ExpansionRequirement {
    pub(super) source: Type,
    pub(super) result: Type,
    pub(super) span: crate::source::ByteSpan,
}

impl TypeEnvironment {
    pub(super) fn with_self_type(self_type: Type) -> Self {
        Self {
            bindings: HashMap::new(),
            literal_packs: HashMap::new(),
            self_type: Some(self_type),
            generic_parameters: HashSet::new(),
            generic_requirements: HashMap::new(),
            type_equalities: Vec::new(),
            equality_requirements: Vec::new(),
            index_requirements: Vec::new(),
            expansion_requirements: Vec::new(),
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
            generic_requirements: self.generic_requirements.clone(),
            type_equalities: self.type_equalities.clone(),
            equality_requirements: self.equality_requirements.clone(),
            index_requirements: self.index_requirements.clone(),
            expansion_requirements: self.expansion_requirements.clone(),
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
        }
    }

    pub(super) fn define_generic_parameter(
        &mut self,
        name: String,
        requirements: crate::resolve::GenericRequirements,
    ) {
        self.generic_parameters.insert(name.clone());
        if !requirements.is_empty() {
            self.generic_requirements.insert(name, requirements);
        }
    }

    pub(super) fn apply_where_clause(
        &mut self,
        clause: Option<&crate::ast::WhereClause>,
        resolved: &crate::resolve::ResolveOutput,
    ) {
        let Some(clause) = clause else {
            return;
        };
        for authored in clause.copy_requirements() {
            if !self.generic_parameters.contains(&authored.name) {
                continue;
            }
            self.generic_requirements
                .entry(authored.name.clone())
                .or_default()
                .push(crate::resolve::GenericRequirement::Copy {
                    span: authored.keyword_span,
                });
        }
        for authored in clause.generic_requirements() {
            if !self.generic_parameters.contains(&authored.name) {
                continue;
            }
            let requirements = self
                .generic_requirements
                .entry(authored.name.clone())
                .or_default();
            for bound in &authored.bounds {
                requirements.push(crate::resolve::GenericRequirement::from_type_expr(
                    bound.clone(),
                ));
            }
        }
        for refinement in clause.refinements() {
            if !self.generic_parameters.contains(&refinement.name) {
                continue;
            }
            let left = Type::Parameter(refinement.name.clone());
            let right = super::type_expr::type_expr_to_type_in_environment(
                &refinement.value,
                resolved,
                self,
            );
            self.type_equalities.push((left, right));
        }
        for equality in clause.equalities() {
            let left =
                super::type_expr::type_expr_to_type_in_environment(&equality.left, resolved, self);
            let right =
                super::type_expr::type_expr_to_type_in_environment(&equality.right, resolved, self);
            self.type_equalities.push((left, right));
        }
        for requirement in clause.operator_requirements() {
            match &requirement.shape {
                crate::ast::OperatorRequirementShape::Equality {
                    left,
                    operator_span,
                    right,
                } => {
                    let left =
                        super::type_expr::type_expr_to_type_in_environment(left, resolved, self);
                    let right =
                        super::type_expr::type_expr_to_type_in_environment(right, resolved, self);
                    self.equality_requirements
                        .push((left, right, *operator_span));
                }
                crate::ast::OperatorRequirementShape::Index { target, index, .. } => {
                    let target =
                        super::type_expr::type_expr_to_type_in_environment(target, resolved, self);
                    let index =
                        super::type_expr::type_expr_to_type_in_environment(index, resolved, self);
                    let result = super::type_expr::type_expr_to_type_in_environment(
                        &requirement.result,
                        resolved,
                        self,
                    );
                    let Type::Borrow {
                        is_readwrite: result_is_readwrite,
                        inner: element,
                    } = result
                    else {
                        continue;
                    };
                    let is_readwrite = matches!(
                        target,
                        Type::Borrow {
                            is_readwrite: true,
                            ..
                        }
                    );
                    if result_is_readwrite != is_readwrite {
                        continue;
                    }
                    self.index_requirements.push(IndexRequirement {
                        target,
                        index,
                        element: *element,
                        is_readwrite,
                        span: requirement.span,
                    });
                }
                crate::ast::OperatorRequirementShape::Expansion { source, .. } => {
                    let source =
                        super::type_expr::type_expr_to_type_in_environment(source, resolved, self);
                    let result = super::type_expr::type_expr_to_type_in_environment(
                        &requirement.result,
                        resolved,
                        self,
                    );
                    self.expansion_requirements.push(ExpansionRequirement {
                        source,
                        result,
                        span: requirement.span,
                    });
                }
            }
        }
    }

    pub(super) fn generic_requirements(
        &self,
        name: &str,
    ) -> Option<&crate::resolve::GenericRequirements> {
        self.generic_requirements.get(name)
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

    pub(super) fn types_equal(&self, left: &Type, right: &Type) -> bool {
        types_equal_with_relations(left, right, &self.type_equalities, &mut Vec::new())
    }

    pub(super) fn equality_requirement_span(
        &self,
        left: &Type,
        right: &Type,
    ) -> Option<crate::source::ByteSpan> {
        self.equality_requirements
            .iter()
            .find_map(|(required_left, required_right, span)| {
                (self.types_equal(left, required_left) && self.types_equal(right, required_right))
                    .then_some(*span)
            })
    }

    pub(super) fn index_requirement(
        &self,
        target: &Type,
        index: &Type,
        require_readwrite: bool,
    ) -> Option<&IndexRequirement> {
        self.index_requirements.iter().find(|requirement| {
            (!require_readwrite || requirement.is_readwrite)
                && self.types_equal(target, &requirement.target)
                && self.types_equal(index, &requirement.index)
        })
    }

    pub(super) fn expansion_requirement(&self, source: &Type) -> Option<&ExpansionRequirement> {
        self.expansion_requirements
            .iter()
            .find(|requirement| self.types_equal(source, &requirement.source))
    }

    pub(super) fn generic_parameter_substitutions(&self) -> HashMap<String, Type> {
        self.generic_parameters
            .iter()
            .map(|name| (name.clone(), Type::Parameter(name.clone())))
            .collect()
    }
}

fn types_equal_with_relations(
    left: &Type,
    right: &Type,
    relations: &[(Type, Type)],
    active: &mut Vec<(Type, Type)>,
) -> bool {
    if left == right || relation_connects(left, right, relations) {
        return true;
    }
    if active
        .iter()
        .any(|(active_left, active_right)| active_left == left && active_right == right)
    {
        return false;
    }
    active.push((left.clone(), right.clone()));
    let equal = match (left, right) {
        (Type::ArrayData { element: left }, Type::ArrayData { element: right })
        | (Type::Pointer(left), Type::Pointer(right))
        | (Type::Optional(left), Type::Optional(right)) => {
            types_equal_with_relations(left, right, relations, active)
        }
        (
            Type::View {
                is_readwrite: left_mode,
                element: left,
            },
            Type::View {
                is_readwrite: right_mode,
                element: right,
            },
        ) => left_mode == right_mode && types_equal_with_relations(left, right, relations, active),
        (
            Type::Borrow {
                is_readwrite: left_mode,
                inner: left,
            },
            Type::Borrow {
                is_readwrite: right_mode,
                inner: right,
            },
        ) => left_mode == right_mode && types_equal_with_relations(left, right, relations, active),
        (
            Type::Array {
                element: left,
                length: left_length,
            },
            Type::Array {
                element: right,
                length: right_length,
            },
        ) => {
            left_length == right_length
                && types_equal_with_relations(left, right, relations, active)
        }
        (
            Type::Fallible {
                success: left_success,
                error: left_error,
            },
            Type::Fallible {
                success: right_success,
                error: right_error,
            },
        ) => {
            types_equal_with_relations(left_success, right_success, relations, active)
                && types_equal_with_relations(left_error, right_error, relations, active)
        }
        (
            Type::Generic {
                name: left_name,
                arguments: left_arguments,
            },
            Type::Generic {
                name: right_name,
                arguments: right_arguments,
            },
        ) => {
            left_name == right_name
                && left_arguments.len() == right_arguments.len()
                && left_arguments
                    .iter()
                    .zip(right_arguments)
                    .all(|(left, right)| types_equal_with_relations(left, right, relations, active))
        }
        _ => false,
    };
    active.pop();
    equal
}

fn relation_connects(left: &Type, right: &Type, relations: &[(Type, Type)]) -> bool {
    let mut reached = vec![left.clone()];
    let mut cursor = 0;
    while cursor < reached.len() {
        let current = reached[cursor].clone();
        if &current == right {
            return true;
        }
        for (relation_left, relation_right) in relations {
            let next = if relation_left == &current {
                relation_right
            } else if relation_right == &current {
                relation_left
            } else {
                continue;
            };
            if !reached.contains(next) {
                reached.push(next.clone());
            }
        }
        cursor += 1;
    }
    false
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
            CallableKind::Drop(name) => format!("destructor `{name}`"),
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
