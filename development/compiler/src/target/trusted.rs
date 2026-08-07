//! Registry for semantic roles owned by trusted standard-library declarations.

use crate::ast::{AstFile, Item, canonical_type_expr};
use crate::semantics::{
    AllocationFailurePolicy, AllocationSource, AllocatorCapabilityKind, TrustedDeclarationFacts,
    TrustedDeclarationRole,
};

pub(crate) fn trusted_declarations_for_module(
    module_path: &str,
    ast: &AstFile,
) -> TrustedDeclarationFacts {
    if module_path == "std/io" {
        return super::trusted_io::trusted_io_declarations(ast);
    }
    let mut facts = TrustedDeclarationFacts::default();
    if module_path != "std/mem" {
        return facts;
    }

    for item in &ast.items {
        match item {
            Item::Struct(struct_) => {
                let kind = match struct_.name.as_str() {
                    "Allocator" if allocator_shape_matches(struct_) => {
                        AllocatorCapabilityKind::Aborting
                    }
                    "TryAllocator" if allocator_shape_matches(struct_) => {
                        AllocatorCapabilityKind::Recoverable
                    }
                    _ => continue,
                };
                facts.insert(
                    struct_.span,
                    TrustedDeclarationRole::AllocatorCapability(kind),
                );
            }
            Item::Primitive(primitive) => {
                let role = match primitive.name.as_str() {
                    "current_allocator_state" | "current_allocator_kind" => {
                        TrustedDeclarationRole::CurrentAllocationContext
                    }
                    "allocation_abort_raw" => TrustedDeclarationRole::AllocationAbort,
                    "alloc_current" => TrustedDeclarationRole::AllocationOperation {
                        source: AllocationSource::CurrentContext,
                        failure_policy: AllocationFailurePolicy::Abort,
                    },
                    "try_alloc_current" => TrustedDeclarationRole::AllocationOperation {
                        source: AllocationSource::CurrentContext,
                        failure_policy: AllocationFailurePolicy::Recoverable,
                    },
                    "region_enter" => TrustedDeclarationRole::RegionEnter,
                    "region_release" => TrustedDeclarationRole::RegionRelease,
                    _ => continue,
                };
                facts.insert(primitive.name_span, role);
            }
            Item::Function(function) => {
                let role = match function.name.as_str() {
                    "alloc"
                        if function_shape_matches(
                            function,
                            &[
                                ("allocator", "&+Allocator"),
                                ("size", "usize"),
                                ("align", "usize"),
                            ],
                            "RawBuffer",
                        ) =>
                    {
                        TrustedDeclarationRole::AllocationOperation {
                            source: AllocationSource::Input(0),
                            failure_policy: AllocationFailurePolicy::Abort,
                        }
                    }
                    "alloc_layout"
                        if function_shape_matches(
                            function,
                            &[("allocator", "&+Allocator"), ("requested", "Layout")],
                            "RawBuffer",
                        ) =>
                    {
                        TrustedDeclarationRole::AllocationOperation {
                            source: AllocationSource::Input(0),
                            failure_policy: AllocationFailurePolicy::Abort,
                        }
                    }
                    "try_alloc"
                        if function_shape_matches(
                            function,
                            &[
                                ("allocator", "&+TryAllocator"),
                                ("size", "usize"),
                                ("align", "usize"),
                            ],
                            "RawBuffer!",
                        ) =>
                    {
                        TrustedDeclarationRole::AllocationOperation {
                            source: AllocationSource::Input(0),
                            failure_policy: AllocationFailurePolicy::Recoverable,
                        }
                    }
                    "try_alloc_layout"
                        if function_shape_matches(
                            function,
                            &[("allocator", "&+TryAllocator"), ("requested", "Layout")],
                            "RawBuffer!",
                        ) =>
                    {
                        TrustedDeclarationRole::AllocationOperation {
                            source: AllocationSource::Input(0),
                            failure_policy: AllocationFailurePolicy::Recoverable,
                        }
                    }
                    "try_grow"
                        if function_shape_matches(
                            function,
                            &[
                                ("allocator", "&+TryAllocator"),
                                ("buffer", "&+RawBuffer"),
                                ("new_size", "usize"),
                            ],
                            "void!",
                        ) =>
                    {
                        TrustedDeclarationRole::AllocationMutation {
                            target: 1,
                            source: AllocationSource::Input(0),
                            fallback_to_current: false,
                        }
                    }
                    "try_grow_owned"
                        if function_shape_matches(
                            function,
                            &[("buffer", "&+RawBuffer"), ("new_size", "usize")],
                            "void!",
                        ) =>
                    {
                        TrustedDeclarationRole::AllocationMutation {
                            target: 0,
                            source: AllocationSource::Input(0),
                            fallback_to_current: true,
                        }
                    }
                    "grow"
                        if function_shape_matches(
                            function,
                            &[
                                ("allocator", "&+Allocator"),
                                ("buffer", "&+RawBuffer"),
                                ("new_size", "usize"),
                            ],
                            "void",
                        ) =>
                    {
                        TrustedDeclarationRole::AllocationMutation {
                            target: 1,
                            source: AllocationSource::Input(0),
                            fallback_to_current: false,
                        }
                    }
                    "grow_owned"
                        if function_shape_matches(
                            function,
                            &[("buffer", "&+RawBuffer"), ("new_size", "usize")],
                            "void",
                        ) =>
                    {
                        TrustedDeclarationRole::AllocationMutation {
                            target: 0,
                            source: AllocationSource::Input(0),
                            fallback_to_current: true,
                        }
                    }
                    _ => continue,
                };
                facts.insert(function.name_span, role);
            }
            Item::Import(_)
            | Item::Test(_)
            | Item::FromImport(_)
            | Item::TypeAlias(_)
            | Item::Enum(_)
            | Item::Interface(_)
            | Item::Impl(_)
            | Item::Construct(_) => {}
        }
    }

    facts
}

fn allocator_shape_matches(struct_: &crate::ast::StructDecl) -> bool {
    !struct_.is_copy
        && struct_.generics.parameters.is_empty()
        && struct_.fields.len() == 2
        && field_matches(&struct_.fields[0], "state", "usize")
        && field_matches(&struct_.fields[1], "kind", "usize")
}

fn field_matches(field: &crate::ast::StructField, name: &str, ty: &str) -> bool {
    field.name == name && canonical_type_expr(&field.ty) == ty
}

fn function_shape_matches(
    function: &crate::ast::FunctionDecl,
    parameters: &[(&str, &str)],
    return_type: &str,
) -> bool {
    function.owner.is_none()
        && function.generics.parameters.is_empty()
        && function.parameters.parameters.len() == parameters.len()
        && function
            .parameters
            .parameters
            .iter()
            .zip(parameters)
            .all(|(actual, (name, ty))| {
                actual.name == *name && canonical_type_expr(&actual.ty) == *ty
            })
        && canonical_type_expr(&function.return_type) == return_type
}
