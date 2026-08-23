use std::collections::BTreeSet;

mod errors;

pub use errors::{PrimitiveContractError, PrimitiveContractRule, PrimitiveRegistryValidationError};

use nocter_declarations::{
    CallableKind, CallableOwner, CallableProvenanceContract, DeclarationGraph, GenericOwner,
    NominalShape, ParameterOwner, ParameterRole, ProvenanceOrigin, Visibility,
};
use nocter_model::{
    BorrowCapability, BuiltinType, CallableId, CompilationTarget, GenericParameterId,
    NominalTypeId, PackageId, TypeId, TypeKind, TypeStore,
};

use crate::{PrimitiveRole, ToolchainSnapshot};

#[derive(Clone, Debug, Eq, PartialEq)]
enum TypeContract {
    Builtin(BuiltinType),
    Generic(usize),
    SyscallResult,
    Pointer(Box<Self>),
    Borrow {
        capability: BorrowCapability,
        referent: Box<Self>,
    },
    Slice(Box<Self>),
}

impl TypeContract {
    fn pointer(pointee: Self) -> Self {
        Self::Pointer(Box::new(pointee))
    }

    fn borrow(capability: BorrowCapability, referent: Self) -> Self {
        Self::Borrow {
            capability,
            referent: Box::new(referent),
        }
    }

    fn readonly(referent: Self) -> Self {
        Self::borrow(BorrowCapability::Readonly, referent)
    }

    fn readwrite(referent: Self) -> Self {
        Self::borrow(BorrowCapability::ReadWrite, referent)
    }

    fn slice(element: Self) -> Self {
        Self::Slice(Box::new(element))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PrimitiveContract {
    generic_count: usize,
    parameters: Vec<TypeContract>,
    result: TypeContract,
    visibility: ContractVisibility,
    target: Option<CompilationTarget>,
    provenance_parameters: Vec<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContractVisibility {
    Public,
    Package,
}

pub(crate) fn validate_primitive_registry(
    graph: &DeclarationGraph,
    types: &TypeStore,
    snapshot: &ToolchainSnapshot,
) -> Result<(), PrimitiveRegistryValidationError> {
    let registry = snapshot.primitives();
    for binding in registry.bindings() {
        validate_binding(
            graph,
            types,
            snapshot.standard_package(),
            binding.role(),
            binding.callable(),
        )?;
    }
    let registered = registry
        .bindings()
        .iter()
        .map(|binding| binding.callable())
        .collect::<BTreeSet<_>>();
    if let Some((callable, _)) =
        graph
            .declarations()
            .callables()
            .iter()
            .find(|(callable, declaration)| {
                declaration.kind() == CallableKind::Primitive && !registered.contains(callable)
            })
    {
        return Err(PrimitiveRegistryValidationError::UnregisteredPrimitive(
            callable,
        ));
    }
    Ok(())
}

fn validate_binding(
    graph: &DeclarationGraph,
    types: &TypeStore,
    standard_package: PackageId,
    role: PrimitiveRole,
    callable: CallableId,
) -> Result<(), PrimitiveRegistryValidationError> {
    let contract = contract(role);
    let declaration = graph
        .declarations()
        .callables()
        .get(callable)
        .ok_or_else(|| contract_error(role, callable, PrimitiveContractRule::Authority))?;
    validate_identity(graph, standard_package, declaration, &contract)
        .and_then(|module| {
            validate_signature(
                graph,
                types,
                standard_package,
                callable,
                declaration,
                &contract,
                module,
            )
        })
        .map_err(|rule| contract_error(role, callable, rule))
}

fn contract_error(
    role: PrimitiveRole,
    callable: CallableId,
    violated_rule: PrimitiveContractRule,
) -> PrimitiveRegistryValidationError {
    PrimitiveRegistryValidationError::Contract(PrimitiveContractError::new(
        role,
        callable,
        violated_rule,
    ))
}

fn validate_identity(
    graph: &DeclarationGraph,
    standard_package: PackageId,
    declaration: &nocter_declarations::CallableDeclaration,
    contract: &PrimitiveContract,
) -> Result<nocter_model::ModuleId, PrimitiveContractRule> {
    let CallableOwner::Module(module) = declaration.owner() else {
        return Err(PrimitiveContractRule::Module);
    };
    let module_declaration = graph
        .modules()
        .get(module)
        .ok_or(PrimitiveContractRule::Module)?;
    if module_declaration.package() != standard_package {
        return Err(PrimitiveContractRule::Authority);
    }
    declaration.name().ok_or(PrimitiveContractRule::Name)?;
    if declaration.kind() != CallableKind::Primitive || declaration.receiver().is_some() {
        return Err(PrimitiveContractRule::CallableKind);
    }
    let site = graph
        .declaration_sites()
        .get(declaration.site())
        .ok_or(PrimitiveContractRule::Authority)?;
    let expected_visibility = match contract.visibility {
        ContractVisibility::Public => Visibility::Public,
        ContractVisibility::Package => Visibility::Package(standard_package),
    };
    if site.module() != module || site.visibility() != expected_visibility {
        return Err(PrimitiveContractRule::Visibility);
    }
    Ok(module)
}

fn validate_signature(
    graph: &DeclarationGraph,
    types: &TypeStore,
    standard_package: PackageId,
    callable: CallableId,
    declaration: &nocter_declarations::CallableDeclaration,
    contract: &PrimitiveContract,
    _module: nocter_model::ModuleId,
) -> Result<(), PrimitiveContractRule> {
    validate_generics(
        graph,
        callable,
        declaration.generic_parameters(),
        contract.generic_count,
    )
    .map_err(|()| PrimitiveContractRule::GenericShape)?;
    if !declaration.requirements().is_empty() {
        return Err(PrimitiveContractRule::Requirements);
    }
    validate_parameters(
        graph,
        types,
        standard_package,
        callable,
        declaration,
        contract,
    )?;
    if !type_matches(
        graph,
        types,
        callable,
        declaration.result(),
        &contract.result,
        standard_package,
    ) {
        return Err(PrimitiveContractRule::ResultType);
    }
    if !provenance_matches(types, declaration, contract) {
        return Err(PrimitiveContractRule::Provenance);
    }
    let actual_target = declaration
        .target_gate()
        .and_then(|symbol| graph.symbols().spelling(symbol));
    if actual_target != contract.target.map(CompilationTarget::name) {
        return Err(PrimitiveContractRule::TargetGate);
    }
    if declaration.body().is_some() {
        return Err(PrimitiveContractRule::Body);
    }
    Ok(())
}

fn validate_parameters(
    graph: &DeclarationGraph,
    types: &TypeStore,
    standard_package: PackageId,
    callable: CallableId,
    declaration: &nocter_declarations::CallableDeclaration,
    contract: &PrimitiveContract,
) -> Result<(), PrimitiveContractRule> {
    if declaration.parameters().len() != contract.parameters.len() {
        return Err(PrimitiveContractRule::ParameterShape);
    }
    for (position, (parameter, expected)) in declaration
        .parameters()
        .iter()
        .zip(&contract.parameters)
        .enumerate()
    {
        let actual = graph
            .declarations()
            .parameters()
            .get(*parameter)
            .ok_or(PrimitiveContractRule::ParameterShape)?;
        if actual.owner() != ParameterOwner::Callable(callable)
            || actual.role() != (ParameterRole::Ordinary { position })
            || !type_matches(
                graph,
                types,
                callable,
                actual.ty(),
                expected,
                standard_package,
            )
        {
            return Err(PrimitiveContractRule::ParameterShape);
        }
    }
    Ok(())
}

fn validate_generics(
    graph: &DeclarationGraph,
    callable: CallableId,
    parameters: &[GenericParameterId],
    expected_count: usize,
) -> Result<(), ()> {
    if parameters.len() != expected_count {
        return Err(());
    }
    for (position, parameter) in parameters.iter().copied().enumerate() {
        let declaration = graph
            .declarations()
            .generic_parameters()
            .get(parameter)
            .ok_or(())?;
        if declaration.owner() != GenericOwner::Callable(callable)
            || declaration.position() != position
        {
            return Err(());
        }
    }
    Ok(())
}

fn provenance_matches(
    _types: &TypeStore,
    declaration: &nocter_declarations::CallableDeclaration,
    contract: &PrimitiveContract,
) -> bool {
    let CallableProvenanceContract::Declared(actual) = declaration.provenance() else {
        return false;
    };
    contract.provenance_parameters.len() == actual.origins().len()
        && contract
            .provenance_parameters
            .iter()
            .zip(actual.origins())
            .all(|(position, origin)| {
                declaration
                    .parameters()
                    .get(*position)
                    .is_some_and(|parameter| *origin == ProvenanceOrigin::Parameter(*parameter))
            })
}

fn type_matches(
    graph: &DeclarationGraph,
    types: &TypeStore,
    callable: CallableId,
    actual: TypeId,
    expected: &TypeContract,
    standard_package: PackageId,
) -> bool {
    match (types.get(actual), expected) {
        (Some(TypeKind::Builtin(actual)), TypeContract::Builtin(expected)) => actual == expected,
        (Some(TypeKind::GenericParameter(actual)), TypeContract::Generic(position)) => {
            graph
                .declarations()
                .callables()
                .get(callable)
                .and_then(|declaration| declaration.generic_parameters().get(*position))
                == Some(actual)
        }
        (
            Some(TypeKind::Nominal {
                definition,
                arguments,
            }),
            TypeContract::SyscallResult,
        ) => {
            arguments.is_empty()
                && validate_syscall_result(graph, types, *definition, standard_package)
        }
        (Some(TypeKind::Pointer(actual)), TypeContract::Pointer(expected))
        | (Some(TypeKind::Slice(actual)), TypeContract::Slice(expected)) => {
            type_matches(graph, types, callable, *actual, expected, standard_package)
        }
        (
            Some(TypeKind::Borrow {
                capability: actual_capability,
                referent: actual,
            }),
            TypeContract::Borrow {
                capability: expected_capability,
                referent: expected,
            },
        ) => {
            actual_capability == expected_capability
                && type_matches(graph, types, callable, *actual, expected, standard_package)
        }
        _ => false,
    }
}

fn validate_syscall_result(
    graph: &DeclarationGraph,
    types: &TypeStore,
    nominal: NominalTypeId,
    standard_package: PackageId,
) -> bool {
    let Some(declaration) = graph.declarations().nominal_types().get(nominal) else {
        return false;
    };
    let Some(site) = graph.declaration_sites().get(declaration.site()) else {
        return false;
    };
    let Some(target_symbol) = graph.symbols().get(CompilationTarget::Arm64Darwin.name()) else {
        return false;
    };
    let NominalShape::Struct {
        copy_declared: true,
        fields,
    } = declaration.shape()
    else {
        return false;
    };
    if site.visibility() != Visibility::Package(standard_package)
        || declaration.target_gate() != Some(target_symbol)
        || !declaration.generic_parameters().is_empty()
        || !declaration.requirements().is_empty()
        || fields.len() != 2
    {
        return false;
    }
    [BuiltinType::Usize, BuiltinType::I32]
        .into_iter()
        .zip(fields.iter().copied())
        .all(|(ty, field)| {
            graph
                .declarations()
                .fields()
                .get(field)
                .is_some_and(|field| {
                    field.owner() == nominal
                        && types.get(field.ty()) == Some(&TypeKind::Builtin(ty))
                        && graph
                            .declaration_sites()
                            .get(field.site())
                            .is_some_and(|field_site| {
                                field_site.module() == site.module()
                                    && field_site.visibility() == Visibility::Public
                            })
                })
        })
}

// This is the closed registry's declarative data table. Keeping all role-to-contract rows in one
// exhaustive match makes review detect omissions and accidental fallthrough directly.
#[allow(clippy::too_many_lines)]
fn contract(role: PrimitiveRole) -> PrimitiveContract {
    let builtin = TypeContract::Builtin;
    let void = || builtin(BuiltinType::Void);
    let never = || builtin(BuiltinType::Never);
    let usize = || builtin(BuiltinType::Usize);
    let i32 = || builtin(BuiltinType::I32);
    let u8 = || builtin(BuiltinType::U8);
    let str_ref = || TypeContract::readonly(builtin(BuiltinType::Str));
    let byte_pointer = || TypeContract::pointer(u8());
    let readonly_bytes = || TypeContract::readonly(TypeContract::slice(u8()));
    let syscall_result = || TypeContract::SyscallResult;
    let package = ContractVisibility::Package;
    let public = ContractVisibility::Public;
    let arm64_darwin = Some(CompilationTarget::Arm64Darwin);
    let make = |generic_count, parameters, result, visibility, target, provenance_parameters| {
        PrimitiveContract {
            generic_count,
            parameters,
            result,
            visibility,
            target,
            provenance_parameters,
        }
    };
    match role {
        PrimitiveRole::NewError => make(
            0,
            vec![str_ref(), str_ref()],
            builtin(BuiltinType::Error),
            package,
            None,
            vec![0, 1],
        ),
        PrimitiveRole::CurrentAllocatorState | PrimitiveRole::CurrentAllocatorKind => {
            make(0, vec![], usize(), package, None, vec![])
        }
        PrimitiveRole::AllocationAbort => make(0, vec![], never(), package, None, vec![]),
        PrimitiveRole::PointerAddress => make(
            1,
            vec![TypeContract::pointer(TypeContract::Generic(0))],
            usize(),
            public,
            None,
            vec![],
        ),
        PrimitiveRole::PointerFromReference => make(
            1,
            vec![TypeContract::readonly(TypeContract::Generic(0))],
            TypeContract::pointer(TypeContract::Generic(0)),
            public,
            None,
            vec![0],
        ),
        PrimitiveRole::PointerFromReadWriteReference => make(
            1,
            vec![TypeContract::readwrite(TypeContract::Generic(0))],
            TypeContract::pointer(TypeContract::Generic(0)),
            public,
            None,
            vec![0],
        ),
        PrimitiveRole::PointerFromAddress => make(
            1,
            vec![usize()],
            TypeContract::pointer(TypeContract::Generic(0)),
            package,
            None,
            vec![],
        ),
        PrimitiveRole::PointeeSize | PrimitiveRole::PointeeAlignment => make(
            1,
            vec![TypeContract::pointer(TypeContract::Generic(0))],
            usize(),
            package,
            None,
            vec![],
        ),
        PrimitiveRole::CopyStringToPointer => make(
            0,
            vec![byte_pointer(), usize(), str_ref()],
            void(),
            package,
            None,
            vec![],
        ),
        PrimitiveRole::CopyPointerToPointer => make(
            0,
            vec![byte_pointer(), byte_pointer(), usize()],
            void(),
            package,
            None,
            vec![],
        ),
        PrimitiveRole::StoreByteToPointer => make(
            0,
            vec![byte_pointer(), usize(), u8()],
            void(),
            package,
            None,
            vec![],
        ),
        PrimitiveRole::StoreValueToPointer => make(
            1,
            vec![
                TypeContract::pointer(TypeContract::Generic(0)),
                usize(),
                TypeContract::Generic(0),
            ],
            void(),
            package,
            None,
            vec![],
        ),
        PrimitiveRole::DropValueAtPointer => make(
            1,
            vec![TypeContract::pointer(TypeContract::Generic(0)), usize()],
            void(),
            package,
            None,
            vec![],
        ),
        PrimitiveRole::TakeValueAtPointer => make(
            1,
            vec![TypeContract::pointer(TypeContract::Generic(0)), usize()],
            TypeContract::Generic(0),
            package,
            None,
            vec![0],
        ),
        PrimitiveRole::StringFromRawParts => make(
            0,
            vec![byte_pointer(), usize()],
            str_ref(),
            package,
            None,
            vec![0],
        ),
        PrimitiveRole::ByteSliceFromRawParts => make(
            0,
            vec![byte_pointer(), usize()],
            readonly_bytes(),
            package,
            None,
            vec![0],
        ),
        PrimitiveRole::MutableByteSliceFromRawParts => make(
            0,
            vec![byte_pointer(), usize()],
            TypeContract::readwrite(TypeContract::slice(u8())),
            package,
            None,
            vec![0],
        ),
        PrimitiveRole::ValueSliceFromRawParts => make(
            1,
            vec![TypeContract::pointer(TypeContract::Generic(0)), usize()],
            TypeContract::readonly(TypeContract::slice(TypeContract::Generic(0))),
            package,
            None,
            vec![0],
        ),
        PrimitiveRole::MutableValueSliceFromRawParts => make(
            1,
            vec![TypeContract::pointer(TypeContract::Generic(0)), usize()],
            TypeContract::readwrite(TypeContract::slice(TypeContract::Generic(0))),
            package,
            None,
            vec![0],
        ),
        PrimitiveRole::BytesFromString => {
            make(0, vec![str_ref()], readonly_bytes(), package, None, vec![0])
        }
        PrimitiveRole::StringSubviewUnchecked => make(
            0,
            vec![str_ref(), usize(), usize()],
            str_ref(),
            package,
            None,
            vec![0],
        ),
        PrimitiveRole::SliceLength | PrimitiveRole::SlicePointerAddress => make(
            1,
            vec![TypeContract::readonly(TypeContract::slice(
                TypeContract::Generic(0),
            ))],
            usize(),
            package,
            None,
            vec![],
        ),
        PrimitiveRole::StringLength | PrimitiveRole::StringPointerAddress => {
            make(0, vec![str_ref()], usize(), package, None, vec![])
        }
        PrimitiveRole::ProcessExit => make(0, vec![i32()], never(), package, arm64_darwin, vec![]),
        PrimitiveRole::ProcessArgumentCount | PrimitiveRole::ProcessEnvironmentCount => {
            make(0, vec![], usize(), package, arm64_darwin, vec![])
        }
        PrimitiveRole::ProcessArgument
        | PrimitiveRole::ProcessEnvironmentName
        | PrimitiveRole::ProcessEnvironmentValue => {
            make(0, vec![usize()], str_ref(), package, arm64_darwin, vec![])
        }
        PrimitiveRole::Syscall0
        | PrimitiveRole::Syscall1
        | PrimitiveRole::Syscall2
        | PrimitiveRole::Syscall3
        | PrimitiveRole::Syscall4
        | PrimitiveRole::Syscall5
        | PrimitiveRole::Syscall6 => {
            let argument_count = match role {
                PrimitiveRole::Syscall0 => 1,
                PrimitiveRole::Syscall1 => 2,
                PrimitiveRole::Syscall2 => 3,
                PrimitiveRole::Syscall3 => 4,
                PrimitiveRole::Syscall4 => 5,
                PrimitiveRole::Syscall5 => 6,
                PrimitiveRole::Syscall6 => 7,
                _ => unreachable!(),
            };
            make(
                0,
                (0..argument_count).map(|_| usize()).collect(),
                syscall_result(),
                package,
                arm64_darwin,
                vec![],
            )
        }
        PrimitiveRole::Trap | PrimitiveRole::Unreachable => {
            make(0, vec![], never(), package, arm64_darwin, vec![])
        }
    }
}
