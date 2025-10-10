use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::interned::namespace::SpanNamespaceAccess;
use auto_lsp::default::db::BaseDatabase;

use crate::hir_def::expressions::expression::{InitExpr, VariableAccess};
use crate::hir_def::semantic_index::semantic_index;
use crate::hir_ty::name_res::resolve_namespace_access;
use crate::{AstId, TypeInfo};
use crate::{
    HirNodeInfo,
    hir_def::{
        expressions::expression::{Expr, MultibitsPart},
        interned::identifier::Ident,
        pous::pou::Pou,
        scope::FileScopeId,
    },
};

#[salsa::tracked(debug)]
pub struct Spec<'db> {
    #[returns(ref)]
    pub kind: SpecKind<'db>,

    pub id: AstId,

    pub scope_id: FileScopeId<'db>,
}

impl<'db> Spec<'db> {
    pub fn shorthand(&'db self, db: &'db dyn BaseDatabase) -> String {
        match self.kind(db) {
            SpecKind::Target(target) => {
                let sema = semantic_index(db, self.scope_id(db).file(db));
                match resolve_namespace_access(db, self.scope_id(db), target.path) {
                    Some(pou) => match pou.pou(db) {
                        Pou::Function(dt) => format!("(function) {}", pou.name(db).text(db)),
                        Pou::FunctionBlock(fb) => {
                            format!("(function_block) {}", pou.name(db).text(db))
                        }
                        Pou::DataType(dt) => dt.spec(db).shorthand(db),
                        Pou::Class(class) => format!("(class) {}", pou.name(db).text(db)),
                        Pou::Interface(it) => format!("(interface) {}", pou.name(db).text(db)),
                    },
                    None => "{unknown}".to_string(),
                }
            }
            SpecKind::Ref(ref_name) => {
                format!("(*ref*) {}", ref_name.shorthand(db))
            }
            _ => "".into(),
        }
    }
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
    Dt,
    Ldt,
    Time,
    LTime,
    Tod,
    LTod,
}

impl<'db> TypeInfo<'db> for ElementarySpec {
    fn type_name(&self, db: &'db dyn BaseDatabase) -> String {
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
            ElementarySpec::Dt => "DT",
            ElementarySpec::Ldt => "LDT",
            ElementarySpec::Time => "TIME",
            ElementarySpec::LTime => "LTIME",
            ElementarySpec::Tod => "TOD",
            ElementarySpec::LTod => "LTOD",
        }.into()
    }
}

impl<'db> HirNodeInfo<'db> for Spec<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id(db)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
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

    pub id: AstId,

    pub name_id: AstId,

    pub scope_id: FileScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for StructElement<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_name_id(&'db self, db: &'db dyn BaseDatabase) -> Option<AstId> {
        Some(self.name_id(db))
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id(db)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Enum<'db> {
    pub typ: Option<Spec<'db>>,
    pub variants: Vec<EnumVariant<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct EnumVariant<'db> {
    pub name: SpanIdent<'db>,
    pub value: Option<Expr<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Array<'db> {
    // lower - upper bounds
    pub subranges: Vec<(Expr<'db>, Expr<'db>)>,
    pub of_type: Box<Spec<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct SubRange<'db> {
    // Should be a INT
    pub _type: Box<Spec<'db>>,
    pub lower: Expr<'db>,
    pub upper: Expr<'db>,
}
