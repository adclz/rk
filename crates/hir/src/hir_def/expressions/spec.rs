use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::interned::namespace::SpanNamespaceAccess;
use auto_lsp::core::span::Span;
use auto_lsp::default::db::BaseDatabase;

use crate::hir_def::expressions::expression::{InitExpr, VariableAccess};
use crate::hir_ty::name_res::resolve_namespace_access; 
use crate::{AstId};
use crate::{
    HirNodeInfo,
    hir_def::{
        expressions::expression::{Expr, MultibitsPart},
        interned::identifier::Ident,
        pous::pou::Pou,
        scope::ScopeId,
    },
};

#[salsa::tracked(debug)]
pub struct Spec<'db> {
    #[returns(ref)]
    pub kind: SpecKind<'db>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    pub scope_id: ScopeId<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum SpecKind<'db> {
    // Simple types
    Simple(ElementarySpec),

    // Composite types
    Struct(Struct<'db>),
    Array(Array<'db>),
    ArrayConformand(Spec<'db>),
    Subrange(SubRange<'db>),
    Enum(Enum<'db>),

    // Reference to another spec
    Ref(Spec<'db>),

    // Targeting a POU or namespace (has to be resolved)
    Target(SpanNamespaceAccess<'db>),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ElementarySpec {
    Bool,
    REDGEBool,
    FEDGEBool,
    Byte,
    Word,
    DWord,
    LWord,
    SInt,
    USInt,
    UInt,
    Int,
    DInt,
    UDInt,
    LInt,
    ULInt,
    Real,
    LReal,
    String,
    WString,
    Char,
    WChar,
    Date,
    LDate,
    DateAndTime,
    LDateTime,
    Time,
    LTime,
    Tod,
    LTod,
}

impl<'db> ElementarySpec {
    pub fn type_name(&self, db: &'db dyn BaseDatabase) -> String {
        match self {
            ElementarySpec::Bool => "BOOL",
            ElementarySpec::REDGEBool => "REDGE_BOOL",
            ElementarySpec::FEDGEBool => "FEDGE_BOOL",
            ElementarySpec::Byte => "BYTE",
            ElementarySpec::Word => "WORD",
            ElementarySpec::DWord => "DWORD",
            ElementarySpec::LWord => "LWORD",
            ElementarySpec::SInt => "SINT",
            ElementarySpec::USInt => "USINT",
            ElementarySpec::UInt => "UINT",
            ElementarySpec::Int => "INT",
            ElementarySpec::DInt => "DINT",
            ElementarySpec::UDInt => "UDINT",
            ElementarySpec::LInt => "LINT",
            ElementarySpec::ULInt => "ULINT",
            ElementarySpec::Real => "REAL",
            ElementarySpec::LReal => "LREAL",
            ElementarySpec::String => "STRING",
            ElementarySpec::WString => "WSTRING",
            ElementarySpec::Char => "CHAR",
            ElementarySpec::WChar => "WCHAR",
            ElementarySpec::Date => "DATE",
            ElementarySpec::LDate => "LDATE",
            ElementarySpec::DateAndTime => "DT",
            ElementarySpec::LDateTime => "LDT",
            ElementarySpec::Time => "TIME",
            ElementarySpec::LTime => "LTIME",
            ElementarySpec::Tod => "TOD",
            ElementarySpec::LTod => "LTOD",
        }
        .into()
    }
}

impl<'db> HirNodeInfo<'db> for Spec<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

#[salsa::tracked(debug)]
pub struct Struct<'db> {
    pub overlap: bool,
    pub elements: Vec<StructElement<'db>>,
}

#[salsa::tracked(debug)]
pub struct StructElement<'db> {
    #[returns(ref)]
    pub name: Ident,

    pub located: Option<VariableAccess<'db>>,
    pub multibits: Option<MultibitsPart>,
    pub spec: Spec<'db>,
    pub init: Option<InitExpr<'db>>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    #[tracked]
    #[no_eq]
    pub name_id: AstId,

    pub scope_id: ScopeId<'db>,
}

impl<'db> StructElement<'db> {
    pub fn name_span(&self, db: &'db dyn BaseDatabase) -> Span {
        self.get_name_span(db).unwrap()
    }
}

impl<'db> HirNodeInfo<'db> for StructElement<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_name_id(&'db self, db: &'db dyn BaseDatabase) -> Option<AstId> {
        Some(self.name_id(db))
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

#[salsa::tracked(debug)]
pub struct Enum<'db> {
    pub typ: Option<Spec<'db>>,
    pub variants: Vec<EnumVariant<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct EnumVariant<'db> {
    pub name: SpanIdent<'db>,
    pub value: Option<Expr<'db>>,
}

#[salsa::tracked(debug)]
pub struct Array<'db> {
    // lower - upper bounds
    pub subranges: Vec<(Expr<'db>, Expr<'db>)>,
    pub of_type: Spec<'db>,
}

#[salsa::tracked(debug)]
pub struct SubRange<'db> {
    // Should be a INT
    pub _type: Spec<'db>,
    pub lower: Expr<'db>,
    pub upper: Expr<'db>,
}

/* 
impl<'db> Spec<'db> {
    pub fn type_name(&self, db: &'db dyn BaseDatabase) -> String {
        match self.kind(db) {
            SpecKind::Simple(elem) => match elem {
                ElementarySpec::Bool => "BOOL",
                ElementarySpec::REDGEBool => "BOOL (RISING EDGE)",
                ElementarySpec::FEDGEBool => "BOOl (FALLING EDGE)",
                ElementarySpec::Byte => "BYTE",
                ElementarySpec::Word => "WORD",
                ElementarySpec::DWord => "DWORD",
                ElementarySpec::LWord => "LWORD",
                ElementarySpec::SInt => "SINT",
                ElementarySpec::USInt => "USINT",
                ElementarySpec::UInt => "UINT",
                ElementarySpec::Int => "INT",
                ElementarySpec::DInt => "DINT",
                ElementarySpec::UDInt => "UDINT",
                ElementarySpec::LInt => "LINT",
                ElementarySpec::ULInt => "ULINT",
                ElementarySpec::Real => "REAL",
                ElementarySpec::LReal => "LREAL",
                ElementarySpec::String => "STRING",
                ElementarySpec::WString => "WSTRING",
                ElementarySpec::Char => "CHAR",
                ElementarySpec::WChar => "WCHAR",
                ElementarySpec::Date => "DATE",
                ElementarySpec::LDate => "LDATE",
                ElementarySpec::DateAndTime => "DT",
                ElementarySpec::LDateTime => "LDT",
                ElementarySpec::Time => "TIME",
                ElementarySpec::LTime => "LTIME",
                ElementarySpec::Tod => "TOD",
                ElementarySpec::LTod => "LTOD",
            }
            .into(),
            SpecKind::Enum(_) => "ENUM".into(),
            SpecKind::Struct(_) => "STRUCT".into(),

            _ => self.full_type_name(db),
        }
    }

    pub fn full_type_name(&self, db: &'db dyn BaseDatabase) -> String {
        match self.kind(db) {
            SpecKind::Simple(elem) => match elem {
                ElementarySpec::Bool => "BOOL",
                ElementarySpec::REDGEBool => "BOOL (RISING EDGE)",
                ElementarySpec::FEDGEBool => "BOOl (FALLING EDGE)",
                ElementarySpec::Byte => "BYTE (0..255)",
                ElementarySpec::Word => "WORD (0..65535)",
                ElementarySpec::DWord => "DWORD (0..4294967295)",
                ElementarySpec::LWord => "LWORD (0..18446744073709551615)",
                ElementarySpec::SInt => "SINT (-128..127)",
                ElementarySpec::USInt => "USINT (0..255)",
                ElementarySpec::UInt => "UINT (0..65535)",
                ElementarySpec::Int => "INT (-32768..32767)",
                ElementarySpec::DInt => "DINT (-2147483648..2147483647)",
                ElementarySpec::UDInt => "UDINT (0..4294967295)",
                ElementarySpec::LInt => "LINT (-9223372036854775808..9223372036854775807)",
                ElementarySpec::ULInt => "ULINT (0..18446744073709551615)",
                ElementarySpec::Real => "REAL (approx. ±1.5 x 10^-45 to ±3.4 x 10^38)",
                ElementarySpec::LReal => "LREAL (approx. ±5.0 x 10^-324 to ±1.7 x 10^308)",
                ElementarySpec::String => "STRING (0 to 255 characters)",
                ElementarySpec::WString => "WSTRING (0 to 255 wide characters)",
                ElementarySpec::Char => "CHAR (single 8-bit character)",
                ElementarySpec::WChar => "WCHAR (single 16-bit wide character)",
                ElementarySpec::Date => "DATE (January 1, 1970 to December 31, 2262)",
                ElementarySpec::LDate => "LDATE (January 1, 0001 to December 31, 9999)",
                ElementarySpec::DateAndTime => "DT (Date and Time)",
                ElementarySpec::LDateTime => "LDT (Long Date and Time)",
                ElementarySpec::Time => {
                    "TIME (0 to 24 days, 20 hours, 31 minutes, 23 seconds, and 647 milliseconds)"
                }
                ElementarySpec::LTime => {
                    "LTIME (0 to 49 days, 17 hours, 27 minutes, 15 seconds, and 808 milliseconds)"
                }
                ElementarySpec::Tod => "TOD (Time of Day)",
                ElementarySpec::LTod => "LTOD (Long Time of Day)",
            }
            .into(),
            SpecKind::Array(array) => {
                let elem_type = array.of_type(db).type_name(db);
                let dimensions: Vec<String> = array
                    .subranges(db)
                    .iter()
                    .map(|(lower, upper)| {
                        let lower = lower.as_range(db)
                            .map(|n| n.to_string())
                            .unwrap_or_default();
                        let upper = upper.as_range(db)
                            .map(|n| n.to_string())
                            .unwrap_or_default();
                        format!("[{lower}..{upper}]")
                    })
                    .collect();
                format!("ARRAY {} OF {}", dimensions.join(" "), elem_type)
            }
            SpecKind::Enum(enm) => format!("ENUM ({} members)", enm.variants(db).len()),
            SpecKind::Subrange(subrange) => {
                let lower = subrange.lower(db).as_range(db)
                    .map(|n| n.to_string())
                    .unwrap_or_default();

                let upper = subrange.upper(db).as_range(db)
                    .map(|n| n.to_string())
                    .unwrap_or_default();

                format!("SUBRANGE ({lower}..{upper})")
            }
            SpecKind::Struct(ztruct) => {
                format!("STRUCT ({} fields)", ztruct.elements(db).len())
            }
            SpecKind::Target(target) => {
                match resolve_namespace_access(db, &target.path) {
                    Some(pou) => {
                        format!(
                            "{}: {}",
                            pou.name(db).text(db),
                            match pou.pou(db) {
                                Pou::Function(_) => "FUNCTION".into(),
                                Pou::FunctionBlock(_) => "FUNCTION_BLOCK".into(),
                                Pou::Class(_) => "CLASS".into(),
                                Pou::Interface(_) => "INTERFACE".into(),
                                Pou::DataType(dt) => dt.spec(db).type_name(db),
                            }
                        )
                    }
                    None => "{unknown}".into(),
                }
            }
            SpecKind::ArrayConformand(_) => "ARRAY*".into(),
            SpecKind::Ref(_ref) => format!("REF_TO {}", _ref.type_name(db)),
        }
    }
}
*/