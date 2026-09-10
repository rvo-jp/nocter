mod errors;

pub use errors::{PrimitiveContractError, PrimitiveContractRule};

use nocter_declarations::{
    CallableKind, CallableOwner, CallableProvenanceContract, DeclarationGraph, GenericOwner,
    NominalShape, ParameterOwner, ParameterRole, ProvenanceOrigin, Visibility,
};
use nocter_model::{
    BorrowCapability, BuiltinType, CallableId, CompilationTarget, GenericParameterId,
    NominalTypeId, PackageId, TypeId, TypeKind, TypeStore,
};
use nocter_runtime_contract::{RuntimeStorageRegistry, RuntimeStorageRole};

use crate::{PrimitiveRole, ToolchainSnapshot};

#[derive(Clone, Debug, Eq, PartialEq)]
enum TypeContract {
    Builtin(BuiltinType),
    Generic(usize),
    SyscallResult,
    SyscallPairResult,
    Pointer(Box<Self>),
    Borrow {
        capability: BorrowCapability,
        referent: Box<Self>,
    },
    Slice(Box<Self>),
    Tuple(Vec<Self>),
    Future(Box<Self>),
    Optional(Box<Self>),
    RuntimeStorage(RuntimeStorageRole),
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

    fn asynchronous(output: Self) -> Self {
        Self::Future(Box::new(output))
    }

    fn optional(value: Self) -> Self {
        Self::Optional(Box::new(value))
    }

    fn tuple(elements: impl Into<Vec<Self>>) -> Self {
        Self::Tuple(elements.into())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PrimitiveContract {
    generic_count: usize,
    parameters: Vec<TypeContract>,
    result: TypeContract,
    exposure: PrimitiveExposure,
    target: Option<CompilationTarget>,
    provenance_parameters: Vec<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrimitiveExposure {
    SourcePrivate,
    Public,
    Package,
}

pub(crate) fn validate_primitive_contracts(
    graph: &DeclarationGraph,
    types: &TypeStore,
    snapshot: &ToolchainSnapshot,
) -> Result<(), PrimitiveContractError> {
    let registry = snapshot.primitives();
    for binding in registry.bindings() {
        validate_binding(
            graph,
            types,
            snapshot.standard_package(),
            snapshot.runtime_storage(),
            binding.role(),
            binding.callable(),
        )?;
    }
    Ok(())
}

fn validate_binding(
    graph: &DeclarationGraph,
    types: &TypeStore,
    standard_package: PackageId,
    runtime_storage: &RuntimeStorageRegistry,
    role: PrimitiveRole,
    callable: CallableId,
) -> Result<(), PrimitiveContractError> {
    let contract = contract(role);
    let declaration = graph
        .declarations()
        .callables()
        .get(callable)
        .ok_or_else(|| contract_error(role, callable, PrimitiveContractRule::Authority))?;
    validate_identity(graph, standard_package, role, declaration, &contract)
        .and_then(|()| {
            validate_signature(
                graph,
                types,
                standard_package,
                runtime_storage,
                callable,
                declaration,
                &contract,
            )
        })
        .map_err(|rule| contract_error(role, callable, rule))
}

fn contract_error(
    role: PrimitiveRole,
    callable: CallableId,
    violated_rule: PrimitiveContractRule,
) -> PrimitiveContractError {
    PrimitiveContractError::new(role, callable, violated_rule)
}

fn validate_identity(
    graph: &DeclarationGraph,
    standard_package: PackageId,
    role: PrimitiveRole,
    declaration: &nocter_declarations::CallableDeclaration,
    contract: &PrimitiveContract,
) -> Result<(), PrimitiveContractRule> {
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
    if matches!(
        declaration.guarantees().allocation(),
        nocter_model::AllocationGuarantee::NoAllocation
    ) && role.effects().may_allocate()
    {
        return Err(PrimitiveContractRule::AllocationGuarantee);
    }
    let site = graph
        .declaration_sites()
        .get(declaration.site())
        .ok_or(PrimitiveContractRule::Authority)?;
    let expected_visibility = match contract.exposure {
        PrimitiveExposure::SourcePrivate => Visibility::Private,
        PrimitiveExposure::Public => Visibility::Public,
        PrimitiveExposure::Package => Visibility::Package(standard_package),
    };
    if site.module() != module || site.visibility() != expected_visibility {
        return Err(PrimitiveContractRule::Visibility);
    }
    Ok(())
}

fn validate_signature(
    graph: &DeclarationGraph,
    types: &TypeStore,
    standard_package: PackageId,
    runtime_storage: &RuntimeStorageRegistry,
    callable: CallableId,
    declaration: &nocter_declarations::CallableDeclaration,
    contract: &PrimitiveContract,
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
    let type_context = TypeContractContext {
        graph,
        types,
        callable,
        standard_package,
        runtime_storage,
    };
    validate_parameters(&type_context, declaration, contract)?;
    if !type_context.matches(declaration.result(), &contract.result) {
        return Err(PrimitiveContractRule::ResultType);
    }
    if !provenance_matches(declaration, contract) {
        return Err(PrimitiveContractRule::Provenance);
    }
    if declaration.target_gate() != contract.target {
        return Err(PrimitiveContractRule::TargetGate);
    }
    if declaration.body().is_some() {
        return Err(PrimitiveContractRule::Body);
    }
    Ok(())
}

fn validate_parameters(
    context: &TypeContractContext<'_>,
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
        let actual = context
            .graph
            .declarations()
            .parameters()
            .get(*parameter)
            .ok_or(PrimitiveContractRule::ParameterShape)?;
        if actual.owner() != ParameterOwner::Callable(context.callable)
            || actual.role() != (ParameterRole::Ordinary { position })
            || !context.matches(actual.ty(), expected)
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

struct TypeContractContext<'a> {
    graph: &'a DeclarationGraph,
    types: &'a TypeStore,
    callable: CallableId,
    standard_package: PackageId,
    runtime_storage: &'a RuntimeStorageRegistry,
}

impl TypeContractContext<'_> {
    fn matches(&self, actual: TypeId, expected: &TypeContract) -> bool {
        match (self.types.get(actual), expected) {
            (Some(TypeKind::Builtin(actual)), TypeContract::Builtin(expected)) => {
                actual == expected
            }
            (Some(TypeKind::GenericParameter(actual)), TypeContract::Generic(position)) => {
                self.graph
                    .declarations()
                    .callables()
                    .get(self.callable)
                    .and_then(|declaration| declaration.generic_parameters().get(*position))
                    == Some(actual)
            }
            (
                Some(TypeKind::Nominal {
                    definition,
                    arguments,
                }),
                TypeContract::RuntimeStorage(role),
            ) => {
                arguments.is_empty() && self.runtime_storage.declaration(*role) == Some(*definition)
            }
            (
                Some(TypeKind::Nominal {
                    definition,
                    arguments,
                }),
                TypeContract::SyscallResult,
            ) => {
                arguments.is_empty()
                    && validate_supporting_struct(
                        self.graph,
                        self.types,
                        *definition,
                        self.standard_package,
                        &[BuiltinType::Usize, BuiltinType::I32],
                    )
            }
            (
                Some(TypeKind::Nominal {
                    definition,
                    arguments,
                }),
                TypeContract::SyscallPairResult,
            ) => {
                arguments.is_empty()
                    && validate_supporting_struct(
                        self.graph,
                        self.types,
                        *definition,
                        self.standard_package,
                        &[BuiltinType::Usize, BuiltinType::Usize, BuiltinType::I32],
                    )
            }
            (Some(TypeKind::Pointer(actual)), TypeContract::Pointer(expected))
            | (Some(TypeKind::Slice(actual)), TypeContract::Slice(expected))
            | (Some(TypeKind::Future(actual)), TypeContract::Future(expected))
            | (Some(TypeKind::Optional(actual)), TypeContract::Optional(expected)) => {
                self.matches(*actual, expected)
            }
            (Some(TypeKind::Tuple(actual)), TypeContract::Tuple(expected)) => {
                actual.as_slice().len() == expected.len()
                    && actual
                        .as_slice()
                        .iter()
                        .zip(expected)
                        .all(|(actual, expected)| self.matches(*actual, expected))
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
            ) => actual_capability == expected_capability && self.matches(*actual, expected),
            _ => false,
        }
    }
}

fn validate_supporting_struct(
    graph: &DeclarationGraph,
    types: &TypeStore,
    nominal: NominalTypeId,
    standard_package: PackageId,
    field_types: &[BuiltinType],
) -> bool {
    let Some(declaration) = graph.declarations().nominal_types().get(nominal) else {
        return false;
    };
    let Some(site) = graph.declaration_sites().get(declaration.site()) else {
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
        || declaration.target_gate() != Some(CompilationTarget::Arm64Darwin)
        || !declaration.generic_parameters().is_empty()
        || !declaration.requirements().is_empty()
        || fields.len() != field_types.len()
    {
        return false;
    }
    field_types
        .iter()
        .copied()
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
    let boolean = || builtin(BuiltinType::Bool);
    let never = || builtin(BuiltinType::Never);
    let usize = || builtin(BuiltinType::Usize);
    let i8 = || builtin(BuiltinType::I8);
    let i16 = || builtin(BuiltinType::I16);
    let i32 = || builtin(BuiltinType::I32);
    let i64 = || builtin(BuiltinType::I64);
    let f32 = || builtin(BuiltinType::F32);
    let f64 = || builtin(BuiltinType::F64);
    let u8 = || builtin(BuiltinType::U8);
    let u16 = || builtin(BuiltinType::U16);
    let u32 = || builtin(BuiltinType::U32);
    let u64 = || builtin(BuiltinType::U64);
    let character = || builtin(BuiltinType::Char);
    let str_ref = || TypeContract::readonly(builtin(BuiltinType::Str));
    let byte_pointer = || TypeContract::pointer(u8());
    let network_owner = || TypeContract::RuntimeStorage(RuntimeStorageRole::NetworkOwner);
    let readonly_bytes = || TypeContract::readonly(TypeContract::slice(u8()));
    let syscall_result = || TypeContract::SyscallResult;
    let syscall_pair_result = || TypeContract::SyscallPairResult;
    let private = PrimitiveExposure::SourcePrivate;
    let package = PrimitiveExposure::Package;
    let public = PrimitiveExposure::Public;
    let arm64_darwin = Some(CompilationTarget::Arm64Darwin);
    let make = |generic_count, parameters, result, exposure, target, provenance_parameters| {
        PrimitiveContract {
            generic_count,
            parameters,
            result,
            exposure,
            target,
            provenance_parameters,
        }
    };
    match role {
        PrimitiveRole::NewError => make(
            0,
            vec![str_ref(), str_ref()],
            builtin(BuiltinType::Error),
            private,
            None,
            vec![],
        ),
        PrimitiveRole::ErrorContext => make(
            0,
            vec![builtin(BuiltinType::Error), str_ref()],
            builtin(BuiltinType::Error),
            private,
            None,
            vec![],
        ),
        PrimitiveRole::ErrorCode | PrimitiveRole::ErrorMessage => make(
            0,
            vec![TypeContract::readonly(builtin(BuiltinType::Error))],
            str_ref(),
            private,
            None,
            vec![0],
        ),
        PrimitiveRole::AllocationFailureError => make(
            0,
            vec![],
            builtin(BuiltinType::Error),
            private,
            None,
            vec![],
        ),
        PrimitiveRole::CurrentAllocatorState | PrimitiveRole::CurrentAllocatorKind => {
            make(0, vec![], usize(), private, None, vec![])
        }
        PrimitiveRole::AllocationAbort => make(0, vec![], never(), package, None, vec![]),
        PrimitiveRole::MemoryMap => make(
            0,
            vec![usize()],
            syscall_result(),
            private,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::MemoryUnmap => make(
            0,
            vec![usize(), usize()],
            syscall_result(),
            private,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::DescriptorClose => make(
            0,
            vec![usize()],
            syscall_result(),
            package,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::EntropySeedFill => make(
            0,
            vec![TypeContract::pointer(u64())],
            i32(),
            private,
            arm64_darwin,
            vec![],
        ),
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
            make(0, vec![str_ref()], readonly_bytes(), private, None, vec![0])
        }
        PrimitiveRole::StringSubviewUnchecked => make(
            0,
            vec![str_ref(), usize(), usize()],
            str_ref(),
            private,
            None,
            vec![0],
        ),
        PrimitiveRole::SliceLength | PrimitiveRole::SlicePointerAddress => make(
            1,
            vec![TypeContract::readonly(TypeContract::slice(
                TypeContract::Generic(0),
            ))],
            usize(),
            private,
            None,
            vec![],
        ),
        PrimitiveRole::StringLength | PrimitiveRole::StringPointerAddress => {
            make(0, vec![str_ref()], usize(), private, None, vec![])
        }
        PrimitiveRole::CharacterFromU32Unchecked => {
            make(0, vec![u32()], character(), package, None, vec![])
        }
        PrimitiveRole::CharacterCodePoint => {
            make(0, vec![character()], u32(), package, None, vec![])
        }
        PrimitiveRole::U8Truncate => make(0, vec![u64()], u8(), private, None, vec![]),
        PrimitiveRole::U16Truncate => make(0, vec![u64()], u16(), private, None, vec![]),
        PrimitiveRole::U32Truncate => make(0, vec![u64()], u32(), private, None, vec![]),
        PrimitiveRole::I8Truncate => make(0, vec![i64()], i8(), private, None, vec![]),
        PrimitiveRole::I16Truncate => make(0, vec![i64()], i16(), private, None, vec![]),
        PrimitiveRole::I32Truncate => make(0, vec![i64()], i32(), private, None, vec![]),
        PrimitiveRole::F32FromBits => make(0, vec![u32()], f32(), private, None, vec![]),
        PrimitiveRole::F32ToBits => make(0, vec![f32()], u32(), private, None, vec![]),
        PrimitiveRole::F64FromBits => make(0, vec![u64()], f64(), private, None, vec![]),
        PrimitiveRole::F64ToBits | PrimitiveRole::F64ToU64 => {
            make(0, vec![f64()], u64(), private, None, vec![])
        }
        PrimitiveRole::F32Floor
        | PrimitiveRole::F32Ceil
        | PrimitiveRole::F32Trunc
        | PrimitiveRole::F32RoundTiesEven => make(0, vec![f32()], f32(), private, None, vec![]),
        PrimitiveRole::F64Floor
        | PrimitiveRole::F64Ceil
        | PrimitiveRole::F64Trunc
        | PrimitiveRole::F64RoundTiesEven => make(0, vec![f64()], f64(), private, None, vec![]),
        PrimitiveRole::F64ToF32 => make(0, vec![f64()], f32(), private, None, vec![]),
        PrimitiveRole::F64ToI64 => make(0, vec![f64()], i64(), private, None, vec![]),
        PrimitiveRole::U64WrappingAdd
        | PrimitiveRole::U64WrappingSubtract
        | PrimitiveRole::U64WrappingMultiply
        | PrimitiveRole::U64MultiplyHigh
        | PrimitiveRole::U64BitwiseAnd
        | PrimitiveRole::U64BitwiseOr
        | PrimitiveRole::U64BitwiseXor
        | PrimitiveRole::U64RotateRight => {
            make(0, vec![u64(), u64()], u64(), private, None, vec![])
        }
        PrimitiveRole::U64LeadingZeros => make(0, vec![u64()], u64(), private, None, vec![]),
        PrimitiveRole::ProcessExit => make(0, vec![i32()], never(), private, arm64_darwin, vec![]),
        PrimitiveRole::ProcessArgumentCount | PrimitiveRole::ProcessEnvironmentCount => {
            make(0, vec![], usize(), private, arm64_darwin, vec![])
        }
        PrimitiveRole::ProcessArgument
        | PrimitiveRole::ProcessEnvironmentName
        | PrimitiveRole::ProcessEnvironmentValue => {
            make(0, vec![usize()], str_ref(), private, arm64_darwin, vec![])
        }
        PrimitiveRole::MonotonicCounterRead | PrimitiveRole::MonotonicCounterFrequency => {
            make(0, vec![], u64(), private, arm64_darwin, vec![])
        }
        PrimitiveRole::MonotonicCounterDelta => {
            make(0, vec![u64(), u64()], u64(), private, arm64_darwin, vec![])
        }
        PrimitiveRole::WallClockRead => make(
            0,
            vec![],
            TypeContract::tuple(vec![i64(), u64(), i32()]),
            private,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::TimeoutWait => {
            make(0, vec![u64(), u64()], i32(), private, arm64_darwin, vec![])
        }
        PrimitiveRole::DescriptorReadiness => make(
            0,
            vec![usize(), boolean()],
            TypeContract::asynchronous(void()),
            private,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::DescriptorReadinessOrDeadline => make(
            0,
            vec![usize(), boolean(), u64()],
            TypeContract::asynchronous(void()),
            private,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::MonotonicDeadline => make(
            0,
            vec![u64()],
            TypeContract::asynchronous(void()),
            private,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::TaskJoin => make(
            2,
            vec![
                TypeContract::asynchronous(TypeContract::Generic(0)),
                TypeContract::asynchronous(TypeContract::Generic(1)),
            ],
            TypeContract::asynchronous(TypeContract::tuple(vec![
                TypeContract::Generic(0),
                TypeContract::Generic(1),
            ])),
            public,
            None,
            vec![0, 1],
        ),
        PrimitiveRole::NetworkConnectionCreate | PrimitiveRole::NetworkListenerCreate => make(
            0,
            vec![byte_pointer()],
            TypeContract::optional(network_owner()),
            private,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::NetworkTlsConnectionCreate => make(
            0,
            vec![
                byte_pointer(),
                byte_pointer(),
                byte_pointer(),
                byte_pointer(),
                usize(),
            ],
            TypeContract::optional(network_owner()),
            private,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::NetworkTlsConnectionMatchesApplicationProtocol => make(
            0,
            vec![TypeContract::readonly(network_owner()), byte_pointer()],
            boolean(),
            private,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::NetworkConnectionStart
        | PrimitiveRole::NetworkConnectionRequestCancel
        | PrimitiveRole::NetworkConnectionReleaseBarrier
        | PrimitiveRole::NetworkListenerStart
        | PrimitiveRole::NetworkListenerRequestCancel
        | PrimitiveRole::NetworkListenerReleaseBarrier => make(
            0,
            vec![TypeContract::readwrite(network_owner())],
            void(),
            private,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::NetworkConnectionEventDescriptor
        | PrimitiveRole::NetworkListenerEventDescriptor => make(
            0,
            vec![TypeContract::readonly(network_owner())],
            usize(),
            private,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::NetworkListenerPort => make(
            0,
            vec![TypeContract::readonly(network_owner())],
            u16(),
            private,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::NetworkConnectionBeginReceive => make(
            0,
            vec![TypeContract::readwrite(network_owner()), u32()],
            void(),
            private,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::NetworkConnectionBeginSend => make(
            0,
            vec![
                TypeContract::readwrite(network_owner()),
                byte_pointer(),
                usize(),
                boolean(),
            ],
            boolean(),
            private,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::NetworkConnectionReceiveEvent => make(
            0,
            vec![
                TypeContract::readwrite(network_owner()),
                byte_pointer(),
                usize(),
            ],
            TypeContract::tuple(vec![usize(), usize(), usize(), usize(), usize()]),
            private,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::NetworkConnectionTryReceiveEvent => make(
            0,
            vec![
                TypeContract::readwrite(network_owner()),
                byte_pointer(),
                usize(),
            ],
            TypeContract::tuple(vec![usize(), usize(), usize(), usize(), usize(), usize()]),
            private,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::NetworkConnectionCopyLocalAddress
        | PrimitiveRole::NetworkConnectionCopyRemoteAddress => make(
            0,
            vec![TypeContract::readonly(network_owner()), byte_pointer()],
            usize(),
            private,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::NetworkConnectionRelease
        | PrimitiveRole::NetworkConnectionDispose
        | PrimitiveRole::NetworkListenerRelease
        | PrimitiveRole::NetworkListenerDispose => make(
            0,
            vec![network_owner()],
            void(),
            private,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::NetworkListenerReceiveEvent => make(
            0,
            vec![TypeContract::readwrite(network_owner())],
            TypeContract::tuple(vec![
                usize(),
                usize(),
                usize(),
                usize(),
                TypeContract::optional(network_owner()),
            ]),
            private,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::NetworkListenerTryReceiveEvent => make(
            0,
            vec![TypeContract::readwrite(network_owner())],
            TypeContract::tuple(vec![
                usize(),
                usize(),
                usize(),
                usize(),
                usize(),
                TypeContract::optional(network_owner()),
            ]),
            private,
            arm64_darwin,
            vec![],
        ),
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
                match role {
                    PrimitiveRole::Syscall0 | PrimitiveRole::Syscall4 => private,
                    _ => package,
                },
                arm64_darwin,
                vec![],
            )
        }
        PrimitiveRole::SyscallPair0 => make(
            0,
            vec![usize()],
            syscall_pair_result(),
            package,
            arm64_darwin,
            vec![],
        ),
        PrimitiveRole::Trap => make(0, vec![], never(), package, arm64_darwin, vec![]),
        PrimitiveRole::Unreachable => make(0, vec![], never(), private, arm64_darwin, vec![]),
    }
}
