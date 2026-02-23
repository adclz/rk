use compact_str::CompactString;
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

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum AnyGeneric {
    ANY,
    ANY_INT,
    ANY_UNSIGNED,
    ANY_SIGNED,
    ANY_REAL,
    ANY_BIT,
    ANY_STRING,
    ANY_DATE,
    ANY_DURATION,
}

impl std::fmt::Display for AnyGeneric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ANY => write!(f, "ANY"),
            Self::ANY_INT => write!(f, "ANY_INT"),
            Self::ANY_UNSIGNED => write!(f, "ANY_UNSIGNED"),
            Self::ANY_SIGNED => write!(f, "ANY_SIGNED"),
            Self::ANY_REAL => write!(f, "ANY_REAL"),
            Self::ANY_BIT => write!(f, "ANY_BIT"),
            Self::ANY_STRING => write!(f, "ANY_STRING"),
            Self::ANY_DATE => write!(f, "ANY_DATE"),
            Self::ANY_DURATION => write!(f, "ANY_DURATION"),
        }
    }
}

impl AnyGeneric {
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Self::ANY_INT | Self::ANY_UNSIGNED | Self::ANY_SIGNED | Self::ANY_REAL
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

    /// Check if an elementary type is a member of this ANY_* type group.
    /// This is strict membership, NOT implicit cast compatibility.
    pub fn contains(&self, spec: crate::hir_def::expressions::spec::ElementarySpec) -> bool {
        use crate::hir_def::expressions::spec::ElementarySpec;
        match self {
            Self::ANY => true,
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
            Self::ANY_STRING => matches!(
                spec,
                ElementarySpec::String
                    | ElementarySpec::WString
                    | ElementarySpec::Char
                    | ElementarySpec::WChar
            ),
            Self::ANY_DATE => matches!(spec, ElementarySpec::Date | ElementarySpec::LDate),
            Self::ANY_DURATION => matches!(spec, ElementarySpec::Time | ElementarySpec::LTime),
        }
    }

    fn any(db: &dyn WorkspaceDataBase) -> Ident {
        Ident::new(db, CompactString::new("ANY"))
    }

    fn any_int(db: &dyn WorkspaceDataBase) -> Ident {
        Ident::new(db, CompactString::new("ANY_INT"))
    }

    fn any_unsigned(db: &dyn WorkspaceDataBase) -> Ident {
        Ident::new(db, CompactString::new("ANY_UNSIGNED"))
    }

    fn any_signed(db: &dyn WorkspaceDataBase) -> Ident {
        Ident::new(db, CompactString::new("ANY_SIGNED"))
    }

    fn any_real(db: &dyn WorkspaceDataBase) -> Ident {
        Ident::new(db, CompactString::new("ANY_REAL"))
    }

    fn any_bit(db: &dyn WorkspaceDataBase) -> Ident {
        Ident::new(db, CompactString::new("ANY_BIT"))
    }

    fn any_string(db: &dyn WorkspaceDataBase) -> Ident {
        Ident::new(db, CompactString::new("ANY_STRING"))
    }

    fn any_date(db: &dyn WorkspaceDataBase) -> Ident {
        Ident::new(db, CompactString::new("ANY_DATE"))
    }

    fn any_duration(db: &dyn WorkspaceDataBase) -> Ident {
        Ident::new(db, CompactString::new("ANY_DURATION"))
    }

    pub fn is_builtin_any(db: &dyn WorkspaceDataBase, ident: &Ident) -> Option<Self> {
        if *ident == Self::any(db) {
            Some(Self::ANY)
        } else if *ident == Self::any_int(db) {
            Some(Self::ANY_INT)
        } else if *ident == Self::any_unsigned(db) {
            Some(Self::ANY_UNSIGNED)
        } else if *ident == Self::any_signed(db) {
            Some(Self::ANY_SIGNED)
        } else if *ident == Self::any_real(db) {
            Some(Self::ANY_REAL)
        } else if *ident == Self::any_bit(db) {
            Some(Self::ANY_BIT)
        } else if *ident == Self::any_string(db) {
            Some(Self::ANY_STRING)
        } else if *ident == Self::any_date(db) {
            Some(Self::ANY_DATE)
        } else if *ident == Self::any_duration(db) {
            Some(Self::ANY_DURATION)
        } else {
            None
        }
    }
}
