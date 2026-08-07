//! Canonical, structure-preserving source notation for types.
//!
//! Producers convert their own type model into `TypeNotation`; this module is
//! the single place that decides which parentheses are required by the parser.

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TypeNotation {
    Atom(String),
    Generic {
        name: String,
        arguments: Vec<TypeNotation>,
    },
    Prefix {
        operator: PrefixOperator,
        inner: Box<TypeNotation>,
    },
    View(Box<TypeNotation>),
    Array {
        element: Box<TypeNotation>,
        length: String,
    },
    Postfix {
        inner: Box<TypeNotation>,
        operator: PostfixOperator,
    },
    Callable {
        result_may_allocate: bool,
        capability_prefix: &'static str,
        parameters: Vec<TypeNotationParameter>,
        return_type: Box<TypeNotation>,
        provenance: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrefixOperator {
    Pointer,
    ReadonlyBorrow,
    ReadwriteBorrow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PostfixOperator {
    Optional,
    Fallible,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypeNotationParameter {
    pub(crate) name: Option<String>,
    pub(crate) ty: TypeNotation,
}

impl TypeNotation {
    pub(crate) fn render(&self) -> String {
        let mut output = String::new();
        self.render_into(&mut output, RenderContext::Root);
        output
    }

    fn render_into(&self, output: &mut String, context: RenderContext) {
        let parenthesize = self.needs_parentheses(context);
        if parenthesize {
            output.push('(');
        }

        match self {
            Self::Atom(name) => output.push_str(name),
            Self::Generic { name, arguments } => {
                output.push_str(name);
                output.push('<');
                render_separated(arguments, output, |argument, output| {
                    argument.render_into(output, RenderContext::Root);
                });
                output.push('>');
            }
            Self::Prefix { operator, inner } => {
                output.push_str(operator.source_text());
                inner.render_into(output, RenderContext::PrefixOperand(*operator));
            }
            Self::View(element) => {
                output.push('[');
                element.render_into(output, RenderContext::Root);
                output.push(']');
            }
            Self::Array { element, length } => {
                output.push('[');
                element.render_into(output, RenderContext::Root);
                output.push_str("; ");
                output.push_str(length);
                output.push(']');
            }
            Self::Postfix { inner, operator } => {
                inner.render_into(output, RenderContext::PostfixOperand);
                output.push_str(operator.source_text());
            }
            Self::Callable {
                result_may_allocate,
                capability_prefix,
                parameters,
                return_type,
                provenance,
            } => {
                if *result_may_allocate {
                    output.push_str("alloc ");
                }
                output.push_str(capability_prefix);
                output.push_str("func(");
                render_separated(parameters, output, |parameter, output| {
                    if let Some(name) = &parameter.name {
                        output.push_str(name);
                        output.push_str(": ");
                    }
                    parameter.ty.render_into(output, RenderContext::Root);
                });
                output.push_str("): ");
                return_type.render_into(output, RenderContext::Root);
                if !provenance.is_empty() {
                    output.push_str(" from ");
                    for (index, origin) in provenance.iter().enumerate() {
                        if index > 0 {
                            output.push_str(" | ");
                        }
                        output.push_str(origin);
                    }
                }
            }
        }

        if parenthesize {
            output.push(')');
        }
    }

    fn needs_parentheses(&self, context: RenderContext) -> bool {
        match context {
            RenderContext::Root => false,
            RenderContext::PrefixOperand(operator) => {
                matches!(self, Self::Postfix { .. })
                    || (matches!(
                        operator,
                        PrefixOperator::ReadonlyBorrow | PrefixOperator::ReadwriteBorrow
                    ) && matches!(self, Self::Callable { .. }))
            }
            RenderContext::PostfixOperand => matches!(self, Self::Callable { .. }),
        }
    }
}

impl PrefixOperator {
    fn source_text(self) -> &'static str {
        match self {
            Self::Pointer => "*",
            Self::ReadonlyBorrow => "&",
            Self::ReadwriteBorrow => "&+",
        }
    }
}

impl PostfixOperator {
    fn source_text(self) -> &'static str {
        match self {
            Self::Optional => "?",
            Self::Fallible => "!",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum RenderContext {
    Root,
    PrefixOperand(PrefixOperator),
    PostfixOperand,
}

fn render_separated<T>(values: &[T], output: &mut String, mut render: impl FnMut(&T, &mut String)) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }
        render(value, output);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(name: &str) -> TypeNotation {
        TypeNotation::Atom(name.to_string())
    }

    fn prefix(operator: PrefixOperator, inner: TypeNotation) -> TypeNotation {
        TypeNotation::Prefix {
            operator,
            inner: Box::new(inner),
        }
    }

    fn postfix(inner: TypeNotation, operator: PostfixOperator) -> TypeNotation {
        TypeNotation::Postfix {
            inner: Box::new(inner),
            operator,
        }
    }

    #[test]
    fn optional_borrow_does_not_need_parentheses() {
        let ty = postfix(
            prefix(PrefixOperator::ReadonlyBorrow, atom("T")),
            PostfixOperator::Optional,
        );
        assert_eq!(ty.render(), "&T?");
    }

    #[test]
    fn borrow_of_optional_keeps_its_structure() {
        let ty = prefix(
            PrefixOperator::ReadonlyBorrow,
            postfix(atom("T"), PostfixOperator::Optional),
        );
        assert_eq!(ty.render(), "&(T?)");
    }

    #[test]
    fn postfix_callable_is_grouped_away_from_its_return_type() {
        let callable = TypeNotation::Callable {
            result_may_allocate: false,
            capability_prefix: "",
            parameters: Vec::new(),
            return_type: Box::new(atom("T")),
            provenance: Vec::new(),
        };
        assert_eq!(
            postfix(callable, PostfixOperator::Optional).render(),
            "(func(): T)?"
        );
    }

    #[test]
    fn borrowed_callable_is_not_confused_with_callable_capability() {
        let callable = TypeNotation::Callable {
            result_may_allocate: false,
            capability_prefix: "",
            parameters: Vec::new(),
            return_type: Box::new(atom("T")),
            provenance: Vec::new(),
        };
        assert_eq!(
            prefix(PrefixOperator::ReadonlyBorrow, callable).render(),
            "&(func(): T)"
        );
    }
}
