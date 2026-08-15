//! Span-free structural identity for types crossing checked-HIR arenas.

use crate::ast::{CallableCapability, ClosureCaptureMode, ResultProvenanceOriginKind, TypeExpr};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum TypeIdentity {
    Callable {
        capability: CallableCapability,
        parameters: Vec<Self>,
        result: Box<Self>,
        provenance: Vec<ProvenanceIdentity>,
    },
    Closure {
        source: u32,
        offset: usize,
        captures: Vec<ClosureCaptureIdentity>,
        parameters: Vec<Self>,
        result: Box<Self>,
        capability: CallableCapability,
    },
    Opaque {
        interface: Box<Self>,
        associated: Vec<(String, Self)>,
        witness: Option<Box<Self>>,
    },
    Reference(String),
    Generic {
        name: String,
        arguments: Vec<Self>,
    },
    Projection {
        base: Box<Self>,
        name: String,
    },
    Pointer(Box<Self>),
    Borrow {
        readwrite: bool,
        inner: Box<Self>,
    },
    View {
        readwrite: bool,
        element: Box<Self>,
    },
    Array {
        element: Box<Self>,
        length: String,
    },
    Optional(Box<Self>),
    Fallible {
        success: Box<Self>,
        error: Box<Self>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ProvenanceIdentity {
    Receiver,
    Parameter(String),
    Static,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ClosureCaptureIdentity {
    name: String,
    mode: CaptureModeIdentity,
    ty: TypeIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CaptureModeIdentity {
    ReadonlyBorrow,
    ReadwriteBorrow,
    Move,
}

impl TypeIdentity {
    pub(crate) fn of(ty: &TypeExpr) -> Self {
        match ty {
            TypeExpr::Callable(callable) => {
                let mut provenance = callable
                    .result_provenance
                    .iter()
                    .flat_map(|clause| clause.origins.iter())
                    .map(|origin| match &origin.kind {
                        ResultProvenanceOriginKind::Receiver => ProvenanceIdentity::Receiver,
                        ResultProvenanceOriginKind::Parameter(name) => {
                            ProvenanceIdentity::Parameter(name.clone())
                        }
                        ResultProvenanceOriginKind::Static => ProvenanceIdentity::Static,
                    })
                    .collect::<Vec<_>>();
                provenance.sort_unstable();
                provenance.dedup();
                Self::Callable {
                    capability: callable.capability,
                    parameters: callable
                        .parameters
                        .iter()
                        .map(|parameter| Self::of(&parameter.ty))
                        .collect(),
                    result: Box::new(Self::of(&callable.return_type)),
                    provenance,
                }
            }
            TypeExpr::Closure(closure) => Self::Closure {
                source: closure.span.source.raw(),
                offset: closure.span.start,
                captures: closure
                    .captures
                    .iter()
                    .map(|capture| ClosureCaptureIdentity {
                        name: capture.name.clone(),
                        mode: match capture.mode {
                            ClosureCaptureMode::ReadonlyBorrow => {
                                CaptureModeIdentity::ReadonlyBorrow
                            }
                            ClosureCaptureMode::ReadwriteBorrow => {
                                CaptureModeIdentity::ReadwriteBorrow
                            }
                            ClosureCaptureMode::Move => CaptureModeIdentity::Move,
                        },
                        ty: Self::of(&capture.ty),
                    })
                    .collect(),
                parameters: closure.parameters.iter().map(Self::of).collect(),
                result: Box::new(Self::of(&closure.return_type)),
                capability: closure.capability,
            },
            TypeExpr::Opaque(opaque) => {
                let mut associated = opaque
                    .associated_bindings
                    .iter()
                    .map(|binding| (binding.name.clone(), Self::of(&binding.value)))
                    .collect::<Vec<_>>();
                associated.sort_unstable_by(|left, right| left.0.cmp(&right.0));
                Self::Opaque {
                    interface: Box::new(Self::of(&opaque.interface)),
                    associated,
                    witness: opaque
                        .witness
                        .as_ref()
                        .map(|witness| Box::new(Self::of(witness))),
                }
            }
            TypeExpr::Reference(reference) => Self::Reference(reference.name.clone()),
            TypeExpr::Generic(generic) => Self::Generic {
                name: generic.name.clone(),
                arguments: generic.arguments.iter().map(Self::of).collect(),
            },
            TypeExpr::Projection(projection) => Self::Projection {
                base: Box::new(Self::of(&projection.base)),
                name: projection.name.clone(),
            },
            TypeExpr::Pointer(pointer) => Self::Pointer(Box::new(Self::of(&pointer.inner))),
            TypeExpr::Borrow(borrow) => Self::Borrow {
                readwrite: borrow.is_readwrite,
                inner: Box::new(Self::of(&borrow.inner)),
            },
            TypeExpr::View(view) => Self::View {
                readwrite: view.is_readwrite,
                element: Box::new(Self::of(&view.element)),
            },
            TypeExpr::Array(array) => Self::Array {
                element: Box::new(Self::of(&array.element)),
                length: array.length.value.replace('_', ""),
            },
            TypeExpr::Optional(optional) => Self::Optional(Box::new(Self::of(&optional.inner))),
            TypeExpr::Fallible(fallible) => Self::Fallible {
                success: Box::new(Self::of(&fallible.success)),
                error: Box::new(Self::of(&fallible.error)),
            },
        }
    }

    /// Runtime destruction follows the concrete witness hidden behind an
    /// opaque result. The destructor `DefId` supplies nominal ownership, so
    /// the package qualification of the outer nominal target is redundant;
    /// generic arguments remain fully qualified structural identities.
    pub(crate) fn runtime_drop_subject(ty: &TypeExpr) -> Self {
        let ty = match ty {
            TypeExpr::Opaque(opaque) => opaque.witness.as_deref().unwrap_or(ty),
            _ => ty,
        };
        match Self::of(ty) {
            Self::Reference(name) => Self::Reference(short_nominal_name(&name).to_string()),
            Self::Generic { name, arguments } => Self::Generic {
                name: short_nominal_name(&name).to_string(),
                arguments,
            },
            identity => identity,
        }
    }

    /// A declared-call key already contains the canonical callable `DefId`.
    /// Normalize only the outer nominal receiver spelling so an imported
    /// `package.Type` and the declaration-local `Type` project to the same
    /// exact key without erasing generic arguments or trying a weaker key.
    pub(crate) fn call_receiver(ty: &TypeExpr) -> Self {
        match Self::of(ty) {
            Self::Reference(name) => Self::Reference(short_nominal_name(&name).to_string()),
            Self::Generic { name, arguments } => Self::Generic {
                name: short_nominal_name(&name).to_string(),
                arguments,
            },
            identity => identity,
        }
    }
}

fn short_nominal_name(name: &str) -> &str {
    name.rsplit(['.', '/']).next().unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{FallibleType, TypeReference};
    use crate::source::{ByteSpan, SourceId};

    fn reference(source: u32, start: usize, name: &str) -> TypeExpr {
        TypeExpr::Reference(TypeReference {
            span: ByteSpan::new(SourceId::new(source), start, start + name.len()),
            name: name.to_string(),
        })
    }

    #[test]
    fn identity_ignores_source_location_but_preserves_fallible_error_type() {
        assert_eq!(
            TypeIdentity::of(&reference(0, 1, "Text")),
            TypeIdentity::of(&reference(9, 40, "Text"))
        );
        let left = TypeExpr::Fallible(FallibleType {
            span: ByteSpan::new(SourceId::new(0), 0, 1),
            success: Box::new(reference(0, 0, "i32")),
            error: Box::new(reference(0, 0, "ParseError")),
        });
        let right = TypeExpr::Fallible(FallibleType {
            span: ByteSpan::new(SourceId::new(0), 0, 1),
            success: Box::new(reference(0, 0, "i32")),
            error: Box::new(reference(0, 0, "IoError")),
        });
        assert_ne!(TypeIdentity::of(&left), TypeIdentity::of(&right));
    }
}
