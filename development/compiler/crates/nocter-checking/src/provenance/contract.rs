use std::collections::{BTreeMap, HashSet};

use nocter_declarations::{DeclarationGraph, NominalShape};
use nocter_model::{
    GenericApplication, GenericParameterId, NominalTypeId, TypeId, TypeKind, TypeStore,
};

type Bindings = BTreeMap<GenericParameterId, TypeId>;

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
                if enqueue_nominal(graph, *definition, arguments, &bindings, &mut pending).is_none()
                {
                    // Invalid declaration structure cannot prove that a loan is absent.
                    return true;
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
                pending.extend(arguments.type_values());
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
                | TypeKind::ValueFunction { .. }
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
                if enqueue_nominal(graph, *definition, arguments, &bindings, &mut pending).is_none()
                {
                    // Invalid declaration structure cannot prove that a loan is absent.
                    return true;
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
                | TypeKind::ValueFunction {
                    origin:
                        nocter_model::ValueFunctionOrigin::Closure(_)
                        | nocter_model::ValueFunctionOrigin::Parameter(_)
                        | nocter_model::ValueFunctionOrigin::Contract(_)
                        | nocter_model::ValueFunctionOrigin::Result(_)
                        | nocter_model::ValueFunctionOrigin::Application(_),
                    ..
                }
                | TypeKind::Opaque { .. }
                | TypeKind::Closure { .. }
                | TypeKind::Callable(_),
            ) => return true,
            Some(
                TypeKind::Builtin(_)
                | TypeKind::ValueFunction {
                    origin: nocter_model::ValueFunctionOrigin::Declaration(_),
                    ..
                }
                | TypeKind::Pointer(_)
                | TypeKind::Slice(_),
            )
            | None => {}
        }
    }
    false
}

/// Both loan queries inspect the same nominal fields and hidden generic storage. Their treatment
/// of unresolved generics remains distinct at the traversal point that consumes each child.
fn enqueue_nominal(
    graph: &DeclarationGraph,
    definition: NominalTypeId,
    arguments: &GenericApplication,
    bindings: &Bindings,
    pending: &mut Vec<(TypeId, Bindings)>,
) -> Option<()> {
    let declaration = graph.declarations().nominal_types().get(definition)?;
    let mut member_bindings = bindings.clone();
    member_bindings.extend(
        declaration
            .generic_parameters()
            .iter()
            .copied()
            .zip(arguments.iter().copied())
            .filter_map(|(parameter, argument)| argument.as_type().map(|ty| (parameter, ty))),
    );
    // Generic values may occupy opaque raw storage, as in Vec<T>, without a visible field.
    pending.extend(
        arguments
            .type_values()
            .map(|argument| (argument, bindings.clone())),
    );
    match declaration.shape() {
        NominalShape::Struct { fields, .. } => {
            for field in fields {
                let field = graph.declarations().fields().get(*field)?;
                pending.push((field.ty(), member_bindings.clone()));
            }
        }
        NominalShape::Enum { variants } => {
            for variant in variants {
                let variant = graph.declarations().variants().get(*variant)?;
                for parameter in variant.payload() {
                    let parameter = graph.declarations().parameters().get(*parameter)?;
                    pending.push((parameter.ty(), member_bindings.clone()));
                }
            }
        }
    }
    Some(())
}
