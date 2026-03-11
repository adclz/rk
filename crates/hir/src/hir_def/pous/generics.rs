use db::WorkspaceDataBase;

use crate::{
    AstId, HasName, HirNodeInfo,
    hir_def::{
        expressions::spec::Spec,
        interned::identifier::{Ident, SpanIdent},
        scope::ScopeId,
    },
};

#[salsa::tracked(debug)]
pub struct GenericParam<'db> {
    pub name: Ident,
    pub name_id: AstId,

    pub generic_contraint: GenericContraint<'db>,
    pub spec_constraints: Vec<SpecContraint<'db>>,

    pub scope: ScopeId<'db>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct GenericContraint<'db> {
    pub value: SpanIdent<'db>,
    pub ast_id: AstId,
}

/// INTO<T> constraint: references another generic parameter
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct SpecContraint<'db> {
    /// The target generic parameter name (the T in INTO<T>)
    pub spec: Spec<'db>,
    pub ast_id: AstId,
}

impl<'db> HirNodeInfo<'db> for GenericParam<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.name_id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope(db)
    }
}

impl<'db> HasName<'db> for GenericParam<'db> {
    fn get_name_ident(&self, db: &'db dyn WorkspaceDataBase) -> Ident {
        self.name(db)
    }

    fn get_name_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.name_id(db)
    }
}

impl<'db> GenericParam<'db> {
    pub fn as_builtin_generic(&self, db: &'db dyn WorkspaceDataBase) -> Option<AnyGeneric> {
        AnyGeneric::is_builtin_any(db, &self.generic_contraint(db).value)
    }

    pub fn is_numeric(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        self.as_builtin_generic(db)
            .map(|g| g.is_numeric())
            .unwrap_or(false)
    }

    pub fn is_binary_integer(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        self.as_builtin_generic(db)
            .map(|g| g.is_binary_integer())
            .unwrap_or(false)
    }

    pub fn is_signed_integer(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        self.as_builtin_generic(db)
            .map(|g| g.is_signed_integer())
            .unwrap_or(false)
    }

    pub fn is_unsigned_integer(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        self.as_builtin_generic(db)
            .map(|g| g.is_unsigned_integer())
            .unwrap_or(false)
    }

    pub fn is_float(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        self.as_builtin_generic(db)
            .map(|g| g.is_float())
            .unwrap_or(false)
    }

    pub fn is_time(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        self.as_builtin_generic(db)
            .map(|g| g.is_time())
            .unwrap_or(false)
    }
}

/// IEC 61131-3 generic data type hierarchy (Table 11)
///
/// ```text
/// ANY
/// ├── ANY_MAGNITUDE
/// │   ├── ANY_NUM
/// │   │   ├── ANY_REAL (REAL, LREAL)
/// │   │   └── ANY_INT
/// │   │       ├── ANY_UNSIGNED (USINT, UINT, UDINT, ULINT)
/// │   │       └── ANY_SIGNED (SINT, INT, DINT, LINT)
/// │   └── ANY_DURATION (TIME, LTIME)
/// ├── ANY_BIT (BOOL, BYTE, WORD, DWORD, LWORD)
/// ├── ANY_CHARS
/// │   ├── ANY_STRING (STRING, WSTRING)
/// │   └── ANY_CHAR (CHAR, WCHAR)
/// └── ANY_DATE (DATE, LDATE, DT, LDT, TOD, LTOD)
/// ```
///
/// Note: ANY_DERIVED and ANY_ELEMENTARY are omitted - per the standard,
/// generic types are for stdlib specification and these two add no practical constraint value.
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum AnyGeneric {
    ANY,
    ANY_MAGNITUDE,
    ANY_NUM,
    ANY_INT,
    ANY_UNSIGNED,
    ANY_SIGNED,
    ANY_REAL,
    ANY_BIT,
    ANY_CHARS,
    ANY_STRING,
    ANY_CHAR,
    ANY_DATE,
    ANY_DURATION,
}

impl std::fmt::Display for AnyGeneric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ANY => write!(f, "ANY"),
            Self::ANY_MAGNITUDE => write!(f, "ANY_MAGNITUDE"),
            Self::ANY_NUM => write!(f, "ANY_NUM"),
            Self::ANY_INT => write!(f, "ANY_INT"),
            Self::ANY_UNSIGNED => write!(f, "ANY_UNSIGNED"),
            Self::ANY_SIGNED => write!(f, "ANY_SIGNED"),
            Self::ANY_REAL => write!(f, "ANY_REAL"),
            Self::ANY_BIT => write!(f, "ANY_BIT"),
            Self::ANY_CHARS => write!(f, "ANY_CHARS"),
            Self::ANY_STRING => write!(f, "ANY_STRING"),
            Self::ANY_CHAR => write!(f, "ANY_CHAR"),
            Self::ANY_DATE => write!(f, "ANY_DATE"),
            Self::ANY_DURATION => write!(f, "ANY_DURATION"),
        }
    }
}

impl AnyGeneric {
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Self::ANY_NUM
                | Self::ANY_INT
                | Self::ANY_UNSIGNED
                | Self::ANY_SIGNED
                | Self::ANY_REAL
        )
    }

    pub fn is_binary_integer(&self) -> bool {
        matches!(self, Self::ANY_BIT)
    }

    pub fn is_signed_integer(&self) -> bool {
        matches!(self, Self::ANY_SIGNED)
    }

    pub fn is_unsigned_integer(&self) -> bool {
        matches!(self, Self::ANY_UNSIGNED)
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Self::ANY_REAL)
    }

    pub fn is_time(&self) -> bool {
        matches!(self, Self::ANY_DATE | Self::ANY_DURATION)
    }

    /// All types in this group support addition/subtraction (IEC 61131-3: ANY_MAGNITUDE)
    pub fn supports_add(&self) -> bool {
        matches!(
            self,
            Self::ANY_MAGNITUDE
                | Self::ANY_NUM
                | Self::ANY_INT
                | Self::ANY_UNSIGNED
                | Self::ANY_SIGNED
                | Self::ANY_REAL
                | Self::ANY_DURATION
        )
    }

    /// All types in this group support multiplication/division (IEC 61131-3: ANY_NUM)
    pub fn supports_mul(&self) -> bool {
        matches!(
            self,
            Self::ANY_NUM
                | Self::ANY_INT
                | Self::ANY_UNSIGNED
                | Self::ANY_SIGNED
                | Self::ANY_REAL
        )
    }

    /// All types in this group support modulo (IEC 61131-3: ANY_INT)
    pub fn supports_mod(&self) -> bool {
        matches!(
            self,
            Self::ANY_INT | Self::ANY_UNSIGNED | Self::ANY_SIGNED
        )
    }

    /// All types in this group support exponentiation (IEC 61131-3: ANY_REAL)
    pub fn supports_power(&self) -> bool {
        matches!(self, Self::ANY_REAL)
    }

    /// All types in this group support boolean operations (ANY_BIT)
    pub fn supports_bool_op(&self) -> bool {
        matches!(self, Self::ANY_BIT)
    }

    /// Check if an elementary type is a member of this ANY_* type group.
    /// This is strict membership, NOT implicit cast compatibility.
    pub fn contains(&self, spec: crate::hir_def::expressions::spec::ElementarySpec) -> bool {
        use crate::hir_def::expressions::spec::ElementarySpec;
        match self {
            Self::ANY => true,
            Self::ANY_MAGNITUDE => {
                Self::ANY_NUM.contains(spec) || Self::ANY_DURATION.contains(spec)
            }
            Self::ANY_NUM => Self::ANY_REAL.contains(spec) || Self::ANY_INT.contains(spec),
            Self::ANY_INT => matches!(
                spec,
                ElementarySpec::SInt
                    | ElementarySpec::Int
                    | ElementarySpec::DInt
                    | ElementarySpec::LInt
                    | ElementarySpec::USInt
                    | ElementarySpec::UInt
                    | ElementarySpec::UDInt
                    | ElementarySpec::ULInt
            ),
            Self::ANY_SIGNED => matches!(
                spec,
                ElementarySpec::SInt
                    | ElementarySpec::Int
                    | ElementarySpec::DInt
                    | ElementarySpec::LInt
            ),
            Self::ANY_UNSIGNED => matches!(
                spec,
                ElementarySpec::USInt
                    | ElementarySpec::UInt
                    | ElementarySpec::UDInt
                    | ElementarySpec::ULInt
            ),
            Self::ANY_REAL => matches!(spec, ElementarySpec::Real | ElementarySpec::LReal),
            Self::ANY_BIT => matches!(
                spec,
                ElementarySpec::Bool
                    | ElementarySpec::Byte
                    | ElementarySpec::Word
                    | ElementarySpec::DWord
                    | ElementarySpec::LWord
            ),
            Self::ANY_CHARS => Self::ANY_STRING.contains(spec) || Self::ANY_CHAR.contains(spec),
            Self::ANY_STRING => matches!(spec, ElementarySpec::String | ElementarySpec::WString),
            Self::ANY_CHAR => matches!(spec, ElementarySpec::Char | ElementarySpec::WChar),
            Self::ANY_DATE => matches!(
                spec,
                ElementarySpec::Date
                    | ElementarySpec::LDate
                    | ElementarySpec::DateAndTime
                    | ElementarySpec::LDateTime
                    | ElementarySpec::Tod
                    | ElementarySpec::LTod
            ),
            Self::ANY_DURATION => matches!(spec, ElementarySpec::Time | ElementarySpec::LTime),
        }
    }
    
    // todo: we could benefit from string interning insteaad of doing string comparisons for builtin generic recognition
    // this implies interning the generic constraint identifiers at the parser level and storing interned ids in the GenericContraint struct, 
    // then matching on those interned ids here instead of doing string lookups.
    pub fn is_builtin_any(db: &dyn WorkspaceDataBase, ident: &Ident) -> Option<Self> {
        let text = ident.text(db);
        match text.as_str() {
            "ANY" => Some(Self::ANY),
            "ANY_MAGNITUDE" => Some(Self::ANY_MAGNITUDE),
            "ANY_NUM" => Some(Self::ANY_NUM),
            "ANY_INT" => Some(Self::ANY_INT),
            "ANY_UNSIGNED" => Some(Self::ANY_UNSIGNED),
            "ANY_SIGNED" => Some(Self::ANY_SIGNED),
            "ANY_REAL" => Some(Self::ANY_REAL),
            "ANY_BIT" => Some(Self::ANY_BIT),
            "ANY_CHARS" => Some(Self::ANY_CHARS),
            "ANY_STRING" => Some(Self::ANY_STRING),
            "ANY_CHAR" => Some(Self::ANY_CHAR),
            "ANY_DATE" => Some(Self::ANY_DATE),
            "ANY_DURATION" => Some(Self::ANY_DURATION),
            _ => None,
        }
    }
}
