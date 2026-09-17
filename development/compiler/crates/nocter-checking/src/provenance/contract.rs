use std::collections::{BTreeMap, HashSet};

use nocter_declarations::{DeclarationGraph, NominalShape};
use nocter_model::{GenericParameterId, TypeId, TypeKind, TypeStore};

/// Reports whether a callable's declared result shape can retain the place loan created only for
/// this invocation.
///
/// Generic parameters and associated projections describe types fixed outside the invocation.
/// Even when later specialization binds one to a borrow, that borrow is carried by the input value;
/// it cannot acquire the lifetime of an implicit receiver or argument reborrow. Explicit borrow
/// structure in the declaration is the only shape that can retain that invocation loan.
pub(crate) fn invocation_place_can_reach_result(
    graph: &DeclarationGraph,
    types: &TypeStore,
    root: TypeId,
) -> bool {
    type Bindings = BTreeMap<GenericParameterId, TypeId>;

    let mut pending = vec![(root, Bindings::new())];
    let mut visited = HashSet::new();
    while let Some((ty, bindings)) = pending.pop() {
        let key = (
            ty,
            bindings
                .iter()
                .map(|(parameter, ty)| (*parameter, *ty))
                .collect::<Vec<_>>(),
        );
        if !visited.insert(key) {
            continue;
        }
        match types.get(ty) {
            // Hidden values may contain declaration-invisible borrow slots. Treating them as
            // retaining the invocation place is the only sound choice until their witnesses are
            // part of the callable boundary contract. Built-in errors own text snapshots and do
            // not retain constructor loans.
            Some(TypeKind::Borrow { .. } | TypeKind::Opaque { .. } | TypeKind::Closure { .. }) => {
                return true;
            }
            Some(TypeKind::GenericParameter(parameter)) => {
                if let Some(replacement) = bindings.get(parameter) {
                    pending.push((*replacement, bindings));
                }
            }
            Some(TypeKind::Nominal {
                definition,
                arguments,
            }) => {
                let Some(declaration) = graph.declarations().nominal_types().get(*definition)
                else {
                    // A missing declaration is an invalid checked program. Remaining
                    // conservative here prevents that invalid state from hiding a loan.
                    return true;
                };
                let mut member_bindings = bindings.clone();
                member_bindings.extend(
                    declaration
                        .generic_parameters()
                        .iter()
                        .copied()
                        .zip(arguments.iter().copied()),
                );
                // A nominal may keep its generic values in opaque raw storage. Concrete borrow
                // arguments therefore count even when no declaration-visible field names them.
                // An unresolved generic does not: its lifetime already belongs to the carried
                // value and cannot be acquired from this invocation's implicit reborrow.
                pending.extend(
                    arguments
                        .iter()
                        .copied()
                        .map(|argument| (argument, bindings.clone())),
                );
                match declaration.shape() {
                    NominalShape::Struct { fields, .. } => {
                        for field in fields {
                            let Some(field) = graph.declarations().fields().get(*field) else {
                                return true;
                            };
                            pending.push((field.ty(), member_bindings.clone()));
                        }
                    }
                    NominalShape::Enum { variants } => {
                        for variant in variants {
                            let Some(variant) = graph.declarations().variants().get(*variant)
                            else {
                                return true;
                            };
                            for parameter in variant.payload() {
                                let Some(parameter) =
                                    graph.declarations().parameters().get(*parameter)
                                else {
                                    return true;
                                };
                                pending.push((parameter.ty(), member_bindings.clone()));
                            }
                        }
                    }
                }
            }
            Some(
                TypeKind::FixedArray { element, .. }
                | TypeKind::Optional(element)
                | TypeKind::Fallible(element),
            ) => pending.push((*element, bindings)),
            Some(TypeKind::Tuple(elements)) => {
                pending.extend(elements.iter().map(|element| (element, bindings.clone())));
            }
            Some(TypeKind::PackEntry { key, value }) => {
                pending.push((*key, bindings.clone()));
                pending.push((*value, bindings));
            }
            Some(_) | None => {}
        }
    }
    false
}

/// Reports whether one invocation-origin contract may retain the place loan created at the call.
///
/// Concrete structure in the declared result remains the primary authority. An authored `from`
/// contract extends that authority only for an associated result selected through the receiver
/// contract. A plain generic result is fixed independently of the invocation: specializing `T` to
/// a borrowed type carries the borrow already stored in `T`, but must not acquire the call's
/// receiver reborrow.
pub(crate) fn invocation_origin_retains_place(
    graph: &DeclarationGraph,
    types: &TypeStore,
    declared_result: TypeId,
    result: TypeId,
    authored_contract: bool,
) -> bool {
    invocation_place_can_reach_result(graph, types, declared_result)
        || (authored_contract
            && contains_associated_projection(types, declared_result)
            && type_can_carry_loan(graph, types, result))
}

fn contains_associated_projection(types: &TypeStore, root: TypeId) -> bool {
    let mut pending = vec![root];
    let mut visited = HashSet::new();
    while let Some(ty) = pending.pop() {
        if !visited.insert(ty) {
            continue;
        }
        match types.get(ty) {
            Some(TypeKind::AssociatedProjection { .. }) => return true,
            Some(TypeKind::Nominal { arguments, .. }) => {
                pending.extend(arguments.iter().copied());
            }
            Some(TypeKind::Tuple(elements)) => pending.extend(elements.iter()),
            Some(
                TypeKind::FixedArray { element: ty, .. }
                | TypeKind::Optional(ty)
                | TypeKind::Fallible(ty)
                | TypeKind::Future(ty)
                | TypeKind::Pointer(ty)
                | TypeKind::Slice(ty),
            ) => pending.push(*ty),
            Some(TypeKind::Borrow { referent, .. }) => pending.push(*referent),
            Some(TypeKind::PackEntry { key, value }) => {
                pending.push(*key);
                pending.push(*value);
            }
            Some(TypeKind::Callable(callable)) => {
                pending.extend(callable.parameters().iter().copied());
                if let Some(pack) = callable.pack() {
                    pending.extend(pack.components());
                }
                pending.push(callable.result());
            }
            Some(
                TypeKind::Builtin(_)
                | TypeKind::GenericParameter(_)
                | TypeKind::InterfaceSelf(_)
                | TypeKind::Opaque { .. }
                | TypeKind::Closure { .. },
            )
            | None => {}
        }
    }
    false
}

/// Reports whether a value representation can carry a source loan.
///
/// This is deliberately narrower than storage provenance: raw pointers and allocator-backed owned
/// values retain storage authority without borrowing the allocator handle. Nominal fields and type
/// arguments are inspected through their declaration identities so an owned scalar-only buffer is
/// not confused with an aggregate that actually stores borrowed values.
pub(crate) fn type_can_carry_loan(
    graph: &DeclarationGraph,
    types: &TypeStore,
    root: TypeId,
) -> bool {
    type Bindings = BTreeMap<GenericParameterId, TypeId>;

    let mut pending = vec![(root, Bindings::new())];
    let mut visited = HashSet::new();
    while let Some((ty, bindings)) = pending.pop() {
        let key = (
            ty,
            bindings
                .iter()
                .map(|(parameter, ty)| (*parameter, *ty))
                .collect::<Vec<_>>(),
        );
        if !visited.insert(key) {
            continue;
        }
        match types.get(ty) {
            Some(TypeKind::GenericParameter(parameter)) => {
                let Some(replacement) = bindings.get(parameter) else {
                    return true;
                };
                pending.push((*replacement, bindings));
            }
            Some(TypeKind::Nominal {
                definition,
                arguments,
            }) => {
                let Some(declaration) = graph.declarations().nominal_types().get(*definition)
                else {
                    return true;
                };
                let mut member_bindings = bindings.clone();
                member_bindings.extend(
                    declaration
                        .generic_parameters()
                        .iter()
                        .copied()
                        .zip(arguments.iter().copied()),
                );
                // Generic arguments are conservatively representation-bearing. Types such as
                // Vec<T> store T values behind raw storage rather than in a declaration-visible
                // field, so field traversal alone cannot prove that a borrowed T is absent.
                pending.extend(
                    arguments
                        .iter()
                        .copied()
                        .map(|argument| (argument, bindings.clone())),
                );
                match declaration.shape() {
                    NominalShape::Struct { fields, .. } => {
                        for field in fields {
                            let Some(field) = graph.declarations().fields().get(*field) else {
                                return true;
                            };
                            pending.push((field.ty(), member_bindings.clone()));
                        }
                    }
                    NominalShape::Enum { variants } => {
                        for variant in variants {
                            let Some(variant) = graph.declarations().variants().get(*variant)
                            else {
                                return true;
                            };
                            for parameter in variant.payload() {
                                let Some(parameter) =
                                    graph.declarations().parameters().get(*parameter)
                                else {
                                    return true;
                                };
                                pending.push((parameter.ty(), member_bindings.clone()));
                            }
                        }
                    }
                }
            }
            Some(
                TypeKind::FixedArray { element, .. }
                | TypeKind::Optional(element)
                | TypeKind::Fallible(element),
            ) => pending.push((*element, bindings)),
            Some(TypeKind::Tuple(elements)) => {
                pending.extend(elements.iter().map(|element| (element, bindings.clone())));
            }
            Some(TypeKind::PackEntry { key, value }) => {
                pending.push((*key, bindings.clone()));
                pending.push((*value, bindings));
            }
            Some(
                TypeKind::Borrow { .. }
                | TypeKind::Future(_)
                | TypeKind::InterfaceSelf(_)
                | TypeKind::AssociatedProjection { .. }
                | TypeKind::Opaque { .. }
                | TypeKind::Closure { .. }
                | TypeKind::Callable(_),
            ) => return true,
            Some(TypeKind::Builtin(_) | TypeKind::Pointer(_) | TypeKind::Slice(_)) | None => {}
        }
    }
    false
}
