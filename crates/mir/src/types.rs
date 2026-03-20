use compact_str::CompactString;
use hir::hir_def::interned::identifier::Ident;

/// A fully resolved, concrete type with known size and alignment.
/// No generics, no ANY_*, no Type::Never, no Type::Variable indirections.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MirType {
    /// Primitive scalar type (bool, integers, reals, time types).
    Elementary(MirElementary),

    /// Fixed-size string (ptr + len representation).
    String(MirStringKind),

    /// Struct with known field layout.
    Struct(MirStructType),

    /// Array with known element type and dimensions.
    Array(MirArrayType),

    /// Enum (stored as the underlying integer type).
    Enum(MirEnumType),

    /// Subrange (stored as the underlying integer type, bounds for validation).
    Subrange(MirSubrangeType),

    /// Pointer to another type (used for REF_TO, VAR_IN_OUT).
    Pointer(Box<MirType>),

    /// Void (for functions with no return value).
    Void,
}

/// Concrete elementary types — every ANY_* has been resolved to one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MirElementary {
    Bool,
    SInt,
    Int,
    DInt,
    LInt,
    USInt,
    UInt,
    UDInt,
    ULInt,
    Byte,
    Word,
    DWord,
    LWord,
    Real,
    LReal,
    Char,
    WChar,
    Time,
    LTime,
    Date,
    LDate,
    Tod,
    LTod,
    DateAndTime,
    LDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MirStringKind {
    String,
    WString,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MirStructType {
    /// Unique name for the struct type.
    pub name: Ident,
    /// Fields in declaration order.
    pub fields: Vec<MirStructField>,
    /// Total size in bytes (with padding).
    pub size: u32,
    /// Maximum field alignment.
    pub align: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MirStructField {
    pub name: Ident,
    pub ty: MirType,
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MirArrayType {
    pub element_type: Box<MirType>,
    /// Dimensions: (lower_bound, upper_bound) per dimension.
    pub dimensions: Vec<(i64, i64)>,
    /// Total element count (product of all dimension sizes).
    pub total_elements: u32,
    /// Size of one element in bytes.
    pub element_size: u32,
    /// Total size in bytes.
    pub size: u32,
    pub align: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MirEnumType {
    pub name: Ident,
    /// Variants with their integer values.
    pub variants: Vec<(CompactString, i64)>,
    /// Underlying storage type (typically Int or DInt).
    pub storage: MirElementary,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MirSubrangeType {
    pub base: MirElementary,
    pub lower: i64,
    pub upper: i64,
}

impl MirType {
    /// Size of this type in bytes.
    pub fn size_bytes(&self) -> u32 {
        match self {
            MirType::Elementary(e) => e.size_bytes(),
            MirType::String(_) => 8, // ptr (i32) + len (i32)
            MirType::Struct(s) => s.size,
            MirType::Array(a) => a.size,
            MirType::Enum(e) => e.storage.size_bytes(),
            MirType::Subrange(s) => s.base.size_bytes(),
            MirType::Pointer(_) => 4, // i32 pointer
            MirType::Void => 0,
        }
    }

    /// Alignment of this type in bytes.
    pub fn alignment(&self) -> u32 {
        match self {
            MirType::Elementary(e) => e.alignment(),
            MirType::String(_) => 4,
            MirType::Struct(s) => s.align,
            MirType::Array(a) => a.align,
            MirType::Enum(e) => e.storage.alignment(),
            MirType::Subrange(s) => s.base.alignment(),
            MirType::Pointer(_) => 4,
            MirType::Void => 1,
        }
    }

    /// Whether this type fits in a WASM scalar (local variable).
    pub fn is_scalar(&self) -> bool {
        matches!(
            self,
            MirType::Elementary(_)
                | MirType::Enum(_)
                | MirType::Subrange(_)
                | MirType::Pointer(_)
        )
    }
}

impl MirElementary {
    pub fn size_bytes(self) -> u32 {
        match self {
            MirElementary::Bool | MirElementary::SInt | MirElementary::USInt | MirElementary::Byte | MirElementary::Char => 4,
            MirElementary::Int | MirElementary::UInt | MirElementary::Word | MirElementary::WChar => 4,
            MirElementary::DInt | MirElementary::UDInt | MirElementary::DWord | MirElementary::Real => 4,
            MirElementary::LInt | MirElementary::ULInt | MirElementary::LWord | MirElementary::LReal => 8,
            MirElementary::Time | MirElementary::LTime => 8,
            MirElementary::Date | MirElementary::LDate => 8,
            MirElementary::Tod | MirElementary::LTod => 8,
            MirElementary::DateAndTime | MirElementary::LDateTime => 8,
        }
    }

    pub fn alignment(self) -> u32 {
        self.size_bytes()
    }

    /// Whether this type maps to a 64-bit WASM value.
    pub fn is_64bit(self) -> bool {
        self.size_bytes() == 8
    }

    /// Whether this type is a floating-point type.
    pub fn is_float(self) -> bool {
        matches!(self, MirElementary::Real | MirElementary::LReal)
    }

    /// Whether this type is a signed integer type.
    pub fn is_signed(self) -> bool {
        matches!(
            self,
            MirElementary::SInt | MirElementary::Int | MirElementary::DInt | MirElementary::LInt
        )
    }
}
