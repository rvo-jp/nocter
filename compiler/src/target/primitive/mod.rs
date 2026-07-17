//! Validation and lowering for compiler-owned primitive boundaries.

use crate::ast::{PrimitiveDecl, TypeExpr, Visibility};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrimitiveValidationError {
    pub(crate) message: String,
    pub(crate) help: Option<String>,
}

pub(crate) fn validate_primitive_declaration(
    module_path: &str,
    target: &str,
    primitive: &PrimitiveDecl,
) -> Result<(), PrimitiveValidationError> {
    let actual = PrimitiveSignature::from_decl(primitive);
    let candidates = primitive_set()
        .iter()
        .copied()
        .filter(|expected| {
            expected.module_path == module_path
                && expected.name == primitive.name
                && expected.applies_to_target(target)
        })
        .collect::<Vec<_>>();

    if candidates.iter().any(|expected| expected.matches(&actual)) {
        return Ok(());
    }

    let qualified_name = format!("{module_path}.{}", primitive.name);
    if let Some(expected) = candidates.first() {
        return Err(PrimitiveValidationError {
            message: format!(
                "`primitive` declaration `{qualified_name}` does not match the compiler-owned primitive signature"
            ),
            help: Some(format!(
                "declare it exactly as `{}`",
                expected.source_signature()
            )),
        });
    }

    Err(PrimitiveValidationError {
        message: format!(
            "`primitive` declaration `{qualified_name}` is not part of the closed primitive set for target `{target}`"
        ),
        help: Some(
            "remove the declaration, or add the primitive to the compiler target registry before using it in the standard library"
                .to_string(),
        ),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PrimitiveSpec {
    module_path: &'static str,
    target: Option<&'static str>,
    visibility: Visibility,
    name: &'static str,
    generics: &'static [&'static str],
    parameters: &'static [PrimitiveParameterSpec],
    return_type: &'static str,
}

impl PrimitiveSpec {
    fn applies_to_target(self, target: &str) -> bool {
        self.target.is_none_or(|expected| expected == target)
    }

    fn matches(self, actual: &PrimitiveSignature) -> bool {
        self.visibility == actual.visibility
            && self.target == actual.target.as_deref()
            && self.generics == actual.generics
            && self.parameters.len() == actual.parameters.len()
            && self
                .parameters
                .iter()
                .zip(&actual.parameters)
                .all(|(expected, actual)| expected.name == actual.name && expected.ty == actual.ty)
            && self.return_type == actual.return_type
    }

    fn source_signature(self) -> String {
        let target = self
            .target
            .map(|target| format!("#target(\"{target}\")\n"))
            .unwrap_or_default();
        let visibility = match self.visibility {
            Visibility::Private => "",
            Visibility::Public => "pub ",
            Visibility::Nocter => "pub(nocter) ",
        };
        let generics = if self.generics.is_empty() {
            String::new()
        } else {
            format!("<{}>", self.generics.join(", "))
        };
        let parameters = self
            .parameters
            .iter()
            .map(|parameter| format!("{}: {}", parameter.name, parameter.ty))
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            "{target}{visibility}primitive {}{generics}({parameters}): {}",
            self.name, self.return_type
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PrimitiveParameterSpec {
    name: &'static str,
    ty: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrimitiveSignature {
    visibility: Visibility,
    target: Option<String>,
    generics: Vec<String>,
    parameters: Vec<PrimitiveParameter>,
    return_type: String,
}

impl PrimitiveSignature {
    fn from_decl(primitive: &PrimitiveDecl) -> Self {
        Self {
            visibility: primitive.visibility,
            target: primitive
                .target
                .as_ref()
                .map(|directive| directive.target.clone()),
            generics: primitive
                .generics
                .parameters
                .iter()
                .map(|parameter| parameter.name.clone())
                .collect(),
            parameters: primitive
                .parameters
                .parameters
                .iter()
                .map(|parameter| PrimitiveParameter {
                    name: parameter.name.clone(),
                    ty: type_expr_display(&parameter.ty),
                })
                .collect(),
            return_type: type_expr_display(&primitive.return_type),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrimitiveParameter {
    name: String,
    ty: String,
}

const ERROR_PARAMETERS: &[PrimitiveParameterSpec] = &[
    PrimitiveParameterSpec {
        name: "code",
        ty: "&str",
    },
    PrimitiveParameterSpec {
        name: "message",
        ty: "&str",
    },
];

const PTR_ADDR_PARAMETERS: &[PrimitiveParameterSpec] = &[PrimitiveParameterSpec {
    name: "pointer",
    ty: "*T",
}];
const PTR_FROM_REF_PARAMETERS: &[PrimitiveParameterSpec] = &[PrimitiveParameterSpec {
    name: "value",
    ty: "&T",
}];
const PTR_FROM_REF_MUT_PARAMETERS: &[PrimitiveParameterSpec] = &[PrimitiveParameterSpec {
    name: "value",
    ty: "&+T",
}];
const PTR_FROM_ADDR_PARAMETERS: &[PrimitiveParameterSpec] = &[PrimitiveParameterSpec {
    name: "address",
    ty: "usize",
}];
const PTR_COPY_STR_TO_PTR_PARAMETERS: &[PrimitiveParameterSpec] = &[
    PrimitiveParameterSpec {
        name: "destination",
        ty: "*u8",
    },
    PrimitiveParameterSpec {
        name: "offset",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "text",
        ty: "&str",
    },
];
const PTR_STORE_U8_TO_PTR_PARAMETERS: &[PrimitiveParameterSpec] = &[
    PrimitiveParameterSpec {
        name: "destination",
        ty: "*u8",
    },
    PrimitiveParameterSpec {
        name: "offset",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "value",
        ty: "u8",
    },
];
const PTR_STR_FROM_RAW_PARTS_PARAMETERS: &[PrimitiveParameterSpec] = &[
    PrimitiveParameterSpec {
        name: "pointer",
        ty: "*u8",
    },
    PrimitiveParameterSpec {
        name: "len",
        ty: "usize",
    },
];
const PTR_SLICE_FROM_RAW_PARTS_PARAMETERS: &[PrimitiveParameterSpec] = &[
    PrimitiveParameterSpec {
        name: "pointer",
        ty: "*u8",
    },
    PrimitiveParameterSpec {
        name: "len",
        ty: "usize",
    },
];
const STRING_BYTES_FROM_STR_PARAMETERS: &[PrimitiveParameterSpec] = &[PrimitiveParameterSpec {
    name: "value",
    ty: "&str",
}];
const SYSCALL0_PARAMETERS: &[PrimitiveParameterSpec] = &[PrimitiveParameterSpec {
    name: "number",
    ty: "usize",
}];
const SYSCALL1_PARAMETERS: &[PrimitiveParameterSpec] = &[
    PrimitiveParameterSpec {
        name: "number",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a0",
        ty: "usize",
    },
];
const SYSCALL2_PARAMETERS: &[PrimitiveParameterSpec] = &[
    PrimitiveParameterSpec {
        name: "number",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a0",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a1",
        ty: "usize",
    },
];
const SYSCALL3_PARAMETERS: &[PrimitiveParameterSpec] = &[
    PrimitiveParameterSpec {
        name: "number",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a0",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a1",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a2",
        ty: "usize",
    },
];
const SYSCALL4_PARAMETERS: &[PrimitiveParameterSpec] = &[
    PrimitiveParameterSpec {
        name: "number",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a0",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a1",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a2",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a3",
        ty: "usize",
    },
];
const SYSCALL5_PARAMETERS: &[PrimitiveParameterSpec] = &[
    PrimitiveParameterSpec {
        name: "number",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a0",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a1",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a2",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a3",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a4",
        ty: "usize",
    },
];
const SYSCALL6_PARAMETERS: &[PrimitiveParameterSpec] = &[
    PrimitiveParameterSpec {
        name: "number",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a0",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a1",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a2",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a3",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a4",
        ty: "usize",
    },
    PrimitiveParameterSpec {
        name: "a5",
        ty: "usize",
    },
];
const WRITE_TEXT_RAW_PARAMETERS: &[PrimitiveParameterSpec] = &[
    PrimitiveParameterSpec {
        name: "fd",
        ty: "i32",
    },
    PrimitiveParameterSpec {
        name: "text",
        ty: "&str",
    },
];
const OPEN_READ_RAW_PARAMETERS: &[PrimitiveParameterSpec] = &[PrimitiveParameterSpec {
    name: "path",
    ty: "*u8",
}];
const WRITE_BYTES_RAW_PARAMETERS: &[PrimitiveParameterSpec] = &[
    PrimitiveParameterSpec {
        name: "fd",
        ty: "i32",
    },
    PrimitiveParameterSpec {
        name: "bytes",
        ty: "&[u8]",
    },
];
const READ_BYTES_RAW_PARAMETERS: &[PrimitiveParameterSpec] = &[
    PrimitiveParameterSpec {
        name: "fd",
        ty: "i32",
    },
    PrimitiveParameterSpec {
        name: "buffer",
        ty: "&+[u8]",
    },
];
const CLOSE_FD_RAW_PARAMETERS: &[PrimitiveParameterSpec] = &[PrimitiveParameterSpec {
    name: "fd",
    ty: "i32",
}];
const PROCESS_EXIT_RAW_PARAMETERS: &[PrimitiveParameterSpec] = &[PrimitiveParameterSpec {
    name: "code",
    ty: "i32",
}];

fn primitive_set() -> &'static [PrimitiveSpec] {
    &[
        PrimitiveSpec {
            module_path: "std/error",
            target: None,
            visibility: Visibility::Nocter,
            name: "new_error",
            generics: &[],
            parameters: ERROR_PARAMETERS,
            return_type: "error",
        },
        PrimitiveSpec {
            module_path: "std/ptr",
            target: None,
            visibility: Visibility::Public,
            name: "addr",
            generics: &["T"],
            parameters: PTR_ADDR_PARAMETERS,
            return_type: "usize",
        },
        PrimitiveSpec {
            module_path: "std/ptr",
            target: None,
            visibility: Visibility::Public,
            name: "from_ref",
            generics: &["T"],
            parameters: PTR_FROM_REF_PARAMETERS,
            return_type: "*T",
        },
        PrimitiveSpec {
            module_path: "std/ptr",
            target: None,
            visibility: Visibility::Public,
            name: "from_ref_mut",
            generics: &["T"],
            parameters: PTR_FROM_REF_MUT_PARAMETERS,
            return_type: "*T",
        },
        PrimitiveSpec {
            module_path: "std/ptr",
            target: None,
            visibility: Visibility::Nocter,
            name: "from_addr",
            generics: &["T"],
            parameters: PTR_FROM_ADDR_PARAMETERS,
            return_type: "*T",
        },
        PrimitiveSpec {
            module_path: "std/ptr",
            target: None,
            visibility: Visibility::Nocter,
            name: "copy_str_to_ptr",
            generics: &[],
            parameters: PTR_COPY_STR_TO_PTR_PARAMETERS,
            return_type: "void",
        },
        PrimitiveSpec {
            module_path: "std/ptr",
            target: None,
            visibility: Visibility::Nocter,
            name: "store_u8_to_ptr",
            generics: &[],
            parameters: PTR_STORE_U8_TO_PTR_PARAMETERS,
            return_type: "void",
        },
        PrimitiveSpec {
            module_path: "std/ptr",
            target: None,
            visibility: Visibility::Nocter,
            name: "str_from_raw_parts",
            generics: &[],
            parameters: PTR_STR_FROM_RAW_PARTS_PARAMETERS,
            return_type: "&str",
        },
        PrimitiveSpec {
            module_path: "std/ptr",
            target: None,
            visibility: Visibility::Nocter,
            name: "slice_from_raw_parts",
            generics: &[],
            parameters: PTR_SLICE_FROM_RAW_PARTS_PARAMETERS,
            return_type: "&[u8]",
        },
        PrimitiveSpec {
            module_path: "std/ptr",
            target: None,
            visibility: Visibility::Nocter,
            name: "slice_from_raw_parts_mut",
            generics: &[],
            parameters: PTR_SLICE_FROM_RAW_PARTS_PARAMETERS,
            return_type: "&+[u8]",
        },
        PrimitiveSpec {
            module_path: "std/string",
            target: None,
            visibility: Visibility::Nocter,
            name: "bytes_from_str",
            generics: &[],
            parameters: STRING_BYTES_FROM_STR_PARAMETERS,
            return_type: "&[u8]",
        },
        PrimitiveSpec {
            module_path: "std/os/macos",
            target: Some("arm64-darwin"),
            visibility: Visibility::Nocter,
            name: "syscall0",
            generics: &[],
            parameters: SYSCALL0_PARAMETERS,
            return_type: "SyscallResult",
        },
        PrimitiveSpec {
            module_path: "std/os/macos",
            target: Some("arm64-darwin"),
            visibility: Visibility::Nocter,
            name: "syscall1",
            generics: &[],
            parameters: SYSCALL1_PARAMETERS,
            return_type: "SyscallResult",
        },
        PrimitiveSpec {
            module_path: "std/os/macos",
            target: Some("arm64-darwin"),
            visibility: Visibility::Nocter,
            name: "syscall2",
            generics: &[],
            parameters: SYSCALL2_PARAMETERS,
            return_type: "SyscallResult",
        },
        PrimitiveSpec {
            module_path: "std/os/macos",
            target: Some("arm64-darwin"),
            visibility: Visibility::Nocter,
            name: "syscall3",
            generics: &[],
            parameters: SYSCALL3_PARAMETERS,
            return_type: "SyscallResult",
        },
        PrimitiveSpec {
            module_path: "std/os/macos",
            target: Some("arm64-darwin"),
            visibility: Visibility::Nocter,
            name: "syscall4",
            generics: &[],
            parameters: SYSCALL4_PARAMETERS,
            return_type: "SyscallResult",
        },
        PrimitiveSpec {
            module_path: "std/os/macos",
            target: Some("arm64-darwin"),
            visibility: Visibility::Nocter,
            name: "syscall5",
            generics: &[],
            parameters: SYSCALL5_PARAMETERS,
            return_type: "SyscallResult",
        },
        PrimitiveSpec {
            module_path: "std/os/macos",
            target: Some("arm64-darwin"),
            visibility: Visibility::Nocter,
            name: "syscall6",
            generics: &[],
            parameters: SYSCALL6_PARAMETERS,
            return_type: "SyscallResult",
        },
        PrimitiveSpec {
            module_path: "std/os/macos",
            target: Some("arm64-darwin"),
            visibility: Visibility::Nocter,
            name: "trap",
            generics: &[],
            parameters: &[],
            return_type: "never",
        },
        PrimitiveSpec {
            module_path: "std/os/macos",
            target: Some("arm64-darwin"),
            visibility: Visibility::Nocter,
            name: "unreachable",
            generics: &[],
            parameters: &[],
            return_type: "never",
        },
        PrimitiveSpec {
            module_path: "std/io",
            target: Some("arm64-darwin"),
            visibility: Visibility::Nocter,
            name: "open_read_raw",
            generics: &[],
            parameters: OPEN_READ_RAW_PARAMETERS,
            return_type: "i32!",
        },
        PrimitiveSpec {
            module_path: "std/io",
            target: Some("arm64-darwin"),
            visibility: Visibility::Nocter,
            name: "write_text_raw",
            generics: &[],
            parameters: WRITE_TEXT_RAW_PARAMETERS,
            return_type: "void!",
        },
        PrimitiveSpec {
            module_path: "std/io",
            target: Some("arm64-darwin"),
            visibility: Visibility::Nocter,
            name: "write_bytes_raw",
            generics: &[],
            parameters: WRITE_BYTES_RAW_PARAMETERS,
            return_type: "void!",
        },
        PrimitiveSpec {
            module_path: "std/io",
            target: Some("arm64-darwin"),
            visibility: Visibility::Nocter,
            name: "read_bytes_raw",
            generics: &[],
            parameters: READ_BYTES_RAW_PARAMETERS,
            return_type: "usize!",
        },
        PrimitiveSpec {
            module_path: "std/io",
            target: Some("arm64-darwin"),
            visibility: Visibility::Nocter,
            name: "close_fd_raw",
            generics: &[],
            parameters: CLOSE_FD_RAW_PARAMETERS,
            return_type: "void",
        },
        PrimitiveSpec {
            module_path: "std/process",
            target: Some("arm64-darwin"),
            visibility: Visibility::Nocter,
            name: "exit_raw",
            generics: &[],
            parameters: PROCESS_EXIT_RAW_PARAMETERS,
            return_type: "never",
        },
    ]
}

fn type_expr_display(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Reference(reference) => reference.name.clone(),
        TypeExpr::Generic(generic) => {
            let arguments = generic
                .arguments
                .iter()
                .map(type_expr_display)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}<{arguments}>", generic.name)
        }
        TypeExpr::Pointer(pointer) => format!("*{}", type_expr_display(&pointer.inner)),
        TypeExpr::Borrow(borrow) if borrow.is_readwrite => {
            format!("&+{}", type_expr_display(&borrow.inner))
        }
        TypeExpr::Borrow(borrow) => format!("&{}", type_expr_display(&borrow.inner)),
        TypeExpr::View(view) if view.is_readwrite => {
            format!("&+[{}]", type_expr_display(&view.element))
        }
        TypeExpr::View(view) => format!("[{}]", type_expr_display(&view.element)),
        TypeExpr::Array(array) => {
            format!(
                "[{}; {}]",
                type_expr_display(&array.element),
                array.length.value
            )
        }
        TypeExpr::Optional(optional) => format!("{}?", type_expr_display(&optional.inner)),
        TypeExpr::Fallible(fallible) => format!("{}!", type_expr_display(&fallible.success)),
    }
}
