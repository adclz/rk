use compact_str::CompactString;
use hir::hir_def::interned::identifier::Ident;

pub use hir::hir_ty::infer::normalize::DEFAULT_STRING_CAPACITY;

/// A fully resolved, concrete type with known size and alignment.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MirType {
    /// Primitive scalar type (bool, integers, reals, time types).
    Elementary(MirElementary),

    /// Fixed-size UTF-8 string in linear memory: a 4-byte length prefix then
    /// `capacity` bytes of buffer; writes go through `rk.str_assign`, a
    /// capacity-bounded copy.
    String { capacity: u32 },

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

/// Concrete elementary types  every ANY_* has been resolved to one of these.
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
    Time,
    LTime,
    Date,
    LDate,
    Tod,
    LTod,
    DateAndTime,
    LDateTime,
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
    /// A `Pointer` that is transparently dereferenced on access: a
    /// `VAR_IN_OUT` field holding the caller's l-value. An explicit `REF_TO`
    /// field is not.
    pub by_ref: bool,
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
            MirType::String { capacity } => 4 + capacity, // ptr (i32) + len (i32)
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
            MirType::String { .. } => 4,
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
            MirType::Elementary(_) | MirType::Enum(_) | MirType::Subrange(_) | MirType::Pointer(_)
        )
    }
}

impl MirElementary {
    pub fn size_bytes(self) -> u32 {
        match self {
            MirElementary::Bool
            | MirElementary::SInt
            | MirElementary::USInt
            | MirElementary::Byte
            | MirElementary::Char => 4,
            MirElementary::Int | MirElementary::UInt | MirElementary::Word => 4,
            MirElementary::DInt
            | MirElementary::UDInt
            | MirElementary::DWord
            | MirElementary::Real => 4,
            MirElementary::LInt
            | MirElementary::ULInt
            | MirElementary::LWord
            | MirElementary::LReal => 8,
            // Date / time encodings
            //   TIME = i32 ms,           LTIME = i64 ns
            //   DATE = i32 days-1970,    LDATE = i64 days-1970
            //   TOD  = i32 ms-of-day,    LTOD  = i64 ns-of-day
            //   DT   = i64 secs-1970 (bounded to LDT's span in hir),
            //   LDT  = i64 ns-1970
            MirElementary::Time => 4,
            MirElementary::LTime => 8,
            MirElementary::Date => 4,
            MirElementary::LDate => 8,
            MirElementary::Tod => 4,
            MirElementary::LTod => 8,
            MirElementary::DateAndTime => 8,
            MirElementary::LDateTime => 8,
        }
    }

    pub fn alignment(self) -> u32 {
        self.size_bytes()
    }

    /// IEC semantic bit width (BYTE = 8, WORD = 16), distinct from the
    /// 4-byte-aligned storage size; shifts, rotates and narrowing casts
    /// respect it.
    pub fn rk_bits(self) -> u32 {
        match self {
            MirElementary::Bool => 1,
            MirElementary::SInt | MirElementary::USInt | MirElementary::Byte => 8,
            MirElementary::Int | MirElementary::UInt | MirElementary::Word => 16,
            // A CHAR is a code point, up to U+10FFFF: the whole lane.
            MirElementary::Char
            | MirElementary::DInt
            | MirElementary::UDInt
            | MirElementary::DWord
            | MirElementary::Real
            | MirElementary::Time
            | MirElementary::Date
            | MirElementary::Tod => 32,
            MirElementary::DateAndTime => 64,
            MirElementary::LInt
            | MirElementary::ULInt
            | MirElementary::LWord
            | MirElementary::LReal
            | MirElementary::LTime
            | MirElementary::LDate
            | MirElementary::LTod
            | MirElementary::LDateTime => 64,
        }
    }

    /// Whether this type maps to a 64-bit WASM value.
    pub fn is_64bit(self) -> bool {
        self.size_bytes() == 8
    }

    /// Whether this type is an integer or a bit string, whose value is its
    /// bits: the types shifts and masks apply to.
    pub fn is_integer(self) -> bool {
        matches!(
            self,
            MirElementary::SInt
                | MirElementary::Int
                | MirElementary::DInt
                | MirElementary::LInt
                | MirElementary::USInt
                | MirElementary::UInt
                | MirElementary::UDInt
                | MirElementary::ULInt
                | MirElementary::Byte
                | MirElementary::Word
                | MirElementary::DWord
                | MirElementary::LWord
        )
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
            // Every date/time encoding is a signed integer (pre-epoch values are
            // representable), so their comparisons are signed.
            | MirElementary::Time
            | MirElementary::LTime
            | MirElementary::Date
            | MirElementary::LDate
            | MirElementary::Tod
            | MirElementary::LTod
            | MirElementary::DateAndTime
            | MirElementary::LDateTime
        )
    }
}
