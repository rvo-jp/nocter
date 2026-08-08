use super::layout::{classify_value, layout_of};
use crate::integer::IntegerType;

pub const ABI_WORD_SIZE: u64 = 8;
pub const ARGUMENT_REGISTER_COUNT: usize = 8;
pub const DIRECT_VALUE_MAX_SIZE: u64 = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiType {
    Bool,
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    Usize,
    Isize,
    Pointer,
    Borrow,
    StrView,
    SliceView,
    Array { element: Box<AbiType>, length: u64 },
    Struct(Vec<AbiField>),
    Enum(AbiEnum),
    Outcome { layout: ValueLayout },
}

impl AbiType {
    pub(crate) const fn integer_type(&self) -> Option<IntegerType> {
        Some(match self {
            Self::I8 => IntegerType::I8,
            Self::I16 => IntegerType::I16,
            Self::I32 => IntegerType::I32,
            Self::I64 => IntegerType::I64,
            Self::Isize => IntegerType::Isize,
            Self::U8 => IntegerType::U8,
            Self::U16 => IntegerType::U16,
            Self::U32 => IntegerType::U32,
            Self::U64 => IntegerType::U64,
            Self::Usize => IntegerType::Usize,
            _ => return None,
        })
    }

    /// Returns whether a value of this ABI type can carry an address whose
    /// validity depends on the caller's current stack frame.
    pub fn contains_borrow(&self) -> bool {
        match self {
            Self::Borrow => true,
            Self::Array { element, .. } => element.contains_borrow(),
            Self::Struct(fields) => fields.iter().any(|field| field.ty.contains_borrow()),
            Self::Enum(value) => value
                .variants
                .iter()
                .filter_map(|variant| variant.payload.as_ref())
                .any(Self::contains_borrow),
            Self::Bool
            | Self::U8
            | Self::I8
            | Self::U16
            | Self::I16
            | Self::U32
            | Self::I32
            | Self::U64
            | Self::I64
            | Self::Usize
            | Self::Isize
            | Self::Pointer
            | Self::StrView
            | Self::SliceView
            | Self::Outcome { .. } => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiField {
    pub name: String,
    pub ty: AbiType,
}

impl AbiField {
    pub fn new(name: impl Into<String>, ty: AbiType) -> Self {
        Self {
            name: name.into(),
            ty,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiEnum {
    pub variants: Vec<AbiEnumVariant>,
    pub payload_offset: u64,
    pub payload_layout: ValueLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiEnumVariant {
    pub name: String,
    pub tag: u8,
    pub payload: Option<AbiType>,
}

impl AbiEnumVariant {
    pub fn new(name: impl Into<String>, tag: u8, payload: Option<AbiType>) -> Self {
        Self {
            name: name.into(),
            tag,
            payload,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueLayout {
    pub size: u64,
    pub align: u64,
}

impl ValueLayout {
    pub fn new(size: u64, align: u64) -> Self {
        Self { size, align }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructLayout {
    pub size: u64,
    pub align: u64,
    pub fields: Vec<FieldLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLayout {
    pub name: String,
    pub offset: u64,
    pub layout: ValueLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueClassification {
    Direct { words: usize },
    Indirect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterPassing {
    Direct { words: usize },
    IndirectPointer,
}

impl ParameterPassing {
    pub fn abi_word_count(self) -> usize {
        match self {
            Self::Direct { words } => words,
            Self::IndirectPointer => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnPassing {
    Void,
    Never,
    Direct { words: usize },
    IndirectPointer,
}

impl ReturnPassing {
    pub fn description(self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::Never => "never",
            Self::Direct { words: 1 } => "1 direct ABI word",
            Self::Direct { words: 2 } => "2 direct ABI words",
            Self::Direct { .. } => "direct ABI words",
            Self::IndirectPointer => "an indirect return pointer",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiValue {
    pub ty: AbiType,
    pub layout: ValueLayout,
    pub classification: ValueClassification,
}

impl AbiValue {
    pub(in crate::abi) fn from_abi_type(ty: AbiType) -> Result<Self, AbiTypeError> {
        let layout = layout_of(&ty)?;
        let classification = classify_value(&ty)?;
        Ok(Self {
            ty,
            layout,
            classification,
        })
    }

    pub fn parameter_passing(&self) -> ParameterPassing {
        match self.classification {
            ValueClassification::Direct { words } => ParameterPassing::Direct { words },
            ValueClassification::Indirect => ParameterPassing::IndirectPointer,
        }
    }

    pub fn parameter_abi_word_count(&self) -> usize {
        self.parameter_passing().abi_word_count()
    }

    pub fn is_indirect(&self) -> bool {
        self.classification == ValueClassification::Indirect
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiParameter {
    pub name: String,
    pub value: AbiValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiReturn {
    Void,
    Never,
    Value(AbiValue),
}

impl AbiReturn {
    pub fn passing(&self) -> ReturnPassing {
        match self {
            Self::Void => ReturnPassing::Void,
            Self::Never => ReturnPassing::Never,
            Self::Value(value) => match value.classification {
                ValueClassification::Direct { words } => ReturnPassing::Direct { words },
                ValueClassification::Indirect => ReturnPassing::IndirectPointer,
            },
        }
    }

    pub fn uses_indirect_pointer(&self) -> bool {
        self.passing() == ReturnPassing::IndirectPointer
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionAbi {
    pub parameters: Vec<AbiParameter>,
    pub return_value: AbiReturn,
}

impl FunctionAbi {
    pub fn parameter_abi_word_count(&self) -> usize {
        self.parameters
            .iter()
            .map(|parameter| parameter.value.parameter_abi_word_count())
            .sum()
    }

    pub fn parameters_fit_registers(&self) -> bool {
        self.parameter_abi_word_count() <= ARGUMENT_REGISTER_COUNT
    }

    pub fn uses_indirect_return_pointer(&self) -> bool {
        self.return_value.uses_indirect_pointer()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    SizeOverflow,
    InvalidAlignment(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiTypeError {
    Layout(LayoutError),
    RecursiveType(String),
    UnsupportedType(String),
    UnresolvedType(String),
    UnsizedValue(String),
}

impl From<LayoutError> for AbiTypeError {
    fn from(error: LayoutError) -> Self {
        Self::Layout(error)
    }
}
