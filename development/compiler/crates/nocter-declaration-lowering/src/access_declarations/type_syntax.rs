//! One recursive source type projection, independent of its semantic position and witness evidence.
use super::{Failure, input_operation, text};
use nocter_declarations::value_access::ParameterBinding;
use nocter_model::{ValueInputOperation, ValueInvocation};
use nocter_syntax::{
    BoundSyntax, ContextualSpelling, Keyword, NodeId, NodeKind, SyntaxOrigin, SyntaxToken,
    TokenKind, child_node_iter, direct_node, first_direct_token,
};

/// A complete length expression that is just a possibly grouped name.
pub(super) fn array_length_name(syntax: BoundSyntax<'_>, mut node: NodeId) -> Option<SyntaxToken> {
    let tree = syntax.tree();
    loop {
        match tree.node(node)?.kind() {
            NodeKind::Expression | NodeKind::GroupedExpression => {
                node = child_node_iter(tree, node).next()?;
            }
            NodeKind::ReferenceExpression => return nocter_syntax::direct_identifier(tree, node),
            _ => return None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CallableTypeSyntax {
    pub head: SyntaxToken,
    pub invocation: ValueInvocation,
    pub inputs: Box<[CallableInputSyntax]>,
    pub result: Box<TypeSyntax>,
}
#[derive(Clone, Debug)]
pub struct CallableInputSyntax {
    pub operation: ValueInputOperation,
    pub data: TypeSyntax,
    pub binding: Option<CallableInputBinding>,
}
#[derive(Clone, Debug)]
pub struct CallableInputBinding {
    pub origin: NodeId,
    pub shape: ParameterBinding<SyntaxOrigin>,
}
impl CallableTypeSyntax {
    /// The original and repeated leaf names in this callable type's own input scope.
    #[must_use]
    pub fn repeated_input_name(
        &self,
        syntax: BoundSyntax<'_>,
    ) -> Option<(SyntaxToken, SyntaxToken)> {
        let mut names = std::collections::BTreeMap::new();
        for name in self
            .inputs
            .iter()
            .filter_map(|input| input.binding.as_ref())
            .flat_map(|binding| binding.shape.names())
        {
            let SyntaxOrigin::Token(name) = *name else {
                continue;
            };
            if let Some(first) = names.insert(text(syntax, name), name) {
                return Some((first, name));
            }
        }
        None
    }
}
#[derive(Clone, Debug)]
pub enum TypeSyntax {
    Named(Box<[SyntaxToken]>),
    Callable(CallableTypeSyntax),
    Future {
        output: Box<TypeSyntax>,
        origin: NodeId,
    },
    Fallible {
        payload: Box<TypeSyntax>,
        marker: SyntaxToken,
    },
    Optional {
        payload: Box<TypeSyntax>,
        layers: std::num::NonZeroUsize,
    },
    Tuple(Box<[TypeSyntax]>),
    Array {
        element: Box<TypeSyntax>,
        length: NodeId,
    },
}
impl TypeSyntax {
    pub(super) fn length_expressions(&self, lengths: &mut Vec<NodeId>) {
        match self {
            Self::Named(_) => {}
            Self::Callable(contract) => {
                for input in &contract.inputs {
                    input.data.length_expressions(lengths);
                }
                contract.result.length_expressions(lengths);
            }
            Self::Optional { payload, .. }
            | Self::Fallible { payload, .. }
            | Self::Future {
                output: payload, ..
            } => {
                payload.length_expressions(lengths);
            }
            Self::Tuple(fields) => {
                for field in fields {
                    field.length_expressions(lengths);
                }
            }
            Self::Array { element, length } => {
                element.length_expressions(lengths);
                lengths.push(*length);
            }
        }
    }
    pub(super) fn tokens(&self) -> Vec<SyntaxToken> {
        self.named_paths().into_iter().flatten().copied().collect()
    }

    pub(super) fn named_paths(&self) -> Vec<&[SyntaxToken]> {
        let mut pending = vec![self];
        let mut names = Vec::new();
        while let Some(ty) = pending.pop() {
            match ty {
                Self::Named(path) => names.push(path.as_ref()),
                Self::Callable(contract) => {
                    pending.push(&contract.result);
                    pending.extend(contract.inputs.iter().rev().map(|input| &input.data));
                }
                Self::Optional { payload, .. }
                | Self::Fallible { payload, .. }
                | Self::Future {
                    output: payload, ..
                }
                | Self::Array {
                    element: payload, ..
                } => pending.push(payload),
                Self::Tuple(fields) => pending.extend(fields.iter().rev()),
            }
        }
        names
    }
}
/// Project a complete source type without name resolution, representation or callable evidence.
/// # Errors
/// Rejects syntax inconsistent with the admitted private grammar.
pub fn project_type_syntax(syntax: BoundSyntax<'_>, ty: NodeId) -> Result<TypeSyntax, Failure> {
    let shape = child_node_iter(syntax.tree(), ty)
        .next()
        .ok_or(Failure::InconsistentSyntax)?;
    let mut data = match syntax
        .tree()
        .node(shape)
        .ok_or(Failure::InconsistentSyntax)?
        .kind()
    {
        NodeKind::NamedType => {
            if let Some(arguments) = direct_node(syntax.tree(), shape, NodeKind::GenericArguments) {
                let origin = nocter_source_index::SourceOrigin::from_node(syntax.tree(), arguments)
                    .map_err(|_| Failure::InconsistentSyntax)?;
                return Err(Failure::Syntax(Box::new([
                    nocter_diagnostics::SourceDiagnostic::new(
                        nocter_diagnostics::DiagnosticCode::E0291,
                        "this type cannot be specialized with explicit generic arguments",
                        origin,
                        [],
                        None::<&str>,
                    ),
                ])));
            }
            let path = nocter_syntax::direct_token_iter(syntax.tree(), shape)
                .filter(|token| {
                    matches!(
                        token.kind(),
                        TokenKind::Identifier | TokenKind::Keyword(Keyword::Void)
                    )
                })
                .collect::<Box<[_]>>();
            if path.is_empty() {
                return Err(Failure::InconsistentSyntax);
            }
            TypeSyntax::Named(path)
        }
        NodeKind::CallableType => TypeSyntax::Callable(callable(syntax, shape)?),
        NodeKind::FutureType => TypeSyntax::Future {
            output: Box::new(project_type_syntax(
                syntax,
                direct_node(syntax.tree(), shape, NodeKind::Type)
                    .ok_or(Failure::InconsistentSyntax)?,
            )?),
            origin: shape,
        },
        NodeKind::GroupedType => project_type_syntax(
            syntax,
            direct_node(syntax.tree(), shape, NodeKind::Type).ok_or(Failure::InconsistentSyntax)?,
        )?,
        NodeKind::FixedArrayType => TypeSyntax::Array {
            element: Box::new(project_type_syntax(
                syntax,
                direct_node(syntax.tree(), shape, NodeKind::Type)
                    .ok_or(Failure::InconsistentSyntax)?,
            )?),
            length: direct_node(syntax.tree(), shape, NodeKind::Expression)
                .ok_or(Failure::InconsistentSyntax)?,
        },
        NodeKind::TupleType => {
            let fields = child_node_iter(syntax.tree(), shape)
                .map(|field| project_type_syntax(syntax, field))
                .collect::<Result<Box<[_]>, _>>()?;
            if fields.len() < 2 {
                return Err(Failure::InconsistentSyntax);
            }
            TypeSyntax::Tuple(fields)
        }
        _ => return Err(Failure::InconsistentSyntax),
    };
    let layers = nocter_syntax::direct_token_iter(syntax.tree(), ty)
        .filter(|token| {
            token.kind() == TokenKind::Punctuation(nocter_syntax::Punctuation::Question)
        })
        .count();
    if let Some(layers) = std::num::NonZeroUsize::new(layers) {
        data = TypeSyntax::Optional {
            payload: Box::new(data),
            layers,
        };
    }
    if let Some(marker) = nocter_syntax::direct_token_iter(syntax.tree(), ty)
        .find(|token| token.kind() == TokenKind::Punctuation(nocter_syntax::Punctuation::Bang))
    {
        data = TypeSyntax::Fallible {
            payload: Box::new(data),
            marker,
        };
    }
    Ok(data)
}
fn callable(syntax: BoundSyntax<'_>, contract: NodeId) -> Result<CallableTypeSyntax, Failure> {
    let head = first_direct_token(syntax.tree(), contract).ok_or(Failure::InconsistentSyntax)?;
    let invocation = match (
        head.kind(),
        ContextualSpelling::from_spelling(text(syntax, head)),
    ) {
        (TokenKind::Keyword(Keyword::Func), _) => ValueInvocation::Read,
        (_, Some(ContextualSpelling::Mut)) => ValueInvocation::Edit,
        (_, Some(ContextualSpelling::Once)) => ValueInvocation::Take,
        _ => return Err(Failure::InconsistentSyntax),
    };
    let parameters = direct_node(syntax.tree(), contract, NodeKind::CallableParameters)
        .ok_or(Failure::InconsistentSyntax)?;
    let inputs = child_node_iter(syntax.tree(), parameters)
        .map(|parameter| {
            let ty = direct_node(syntax.tree(), parameter, NodeKind::Type)
                .ok_or(Failure::InconsistentSyntax)?;
            let binding = direct_node(syntax.tree(), parameter, NodeKind::BindingPattern)
                .map(|origin| {
                    Ok(CallableInputBinding {
                        origin,
                        shape: super::headers::parameter_binding(syntax, origin)?,
                    })
                })
                .transpose()?;
            Ok(CallableInputSyntax {
                operation: input_operation(syntax, parameter)?,
                data: project_type_syntax(syntax, ty)?,
                binding,
            })
        })
        .collect::<Result<_, Failure>>()?;
    let result =
        direct_node(syntax.tree(), contract, NodeKind::Type).ok_or(Failure::InconsistentSyntax)?;
    Ok(CallableTypeSyntax {
        head,
        invocation,
        inputs,
        result: Box::new(project_type_syntax(syntax, result)?),
    })
}
