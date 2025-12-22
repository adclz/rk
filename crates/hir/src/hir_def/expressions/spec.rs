use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::interned::namespace::SpanNamespaceAccess;
use auto_lsp::core::span::Span;
use auto_lsp::default::db::BaseDatabase;

use crate::hir_def::expressions::expression::{InitExpr, VariableAccess};
use crate::hir_ty::name_res::resolve_namespace_access; 
use crate::{AstId, HasName};
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
    pub name: Ident,

    #[tracked]
    #[no_eq]
    pub name_id: AstId,

    pub located: Option<VariableAccess<'db>>,
    pub multibits: Option<MultibitsPart>,
    pub spec: Spec<'db>,
    pub init: Option<InitExpr<'db>>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for StructElement<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

impl<'db> HasName<'db> for StructElement<'db> {
    fn get_name_ident(&self, db: &'db dyn BaseDatabase) -> Ident {
        self.name(db)
    }

    fn get_name_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.name_id(db)
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