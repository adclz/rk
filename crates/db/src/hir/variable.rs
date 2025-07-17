use auto_lsp::{
    core::span::Span,
    default::db::{file::File, BaseDatabase},
};

use crate::{
    hir::expression::Expr,
    ident::Ident,
    solver::fq_name::NamespaceAccess,
    to_proto::{SymbolInfo, ToProto},
};

#[salsa::tracked]
pub struct Variable<'db> {
    pub file: File,

    #[returns(ref)]
    pub name: Ident,

    #[returns(ref)]
    pub range: Span,

    #[returns(ref)]
    pub name_span: Span,

    pub kind: VariableKind,

    #[tracked]
    pub spec: Spec<'db>,

    #[tracked]
    pub init: Option<Expr<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum VariableKind {
    Input,
    Output,
    InOut,
    Temp,
    Local,
    External,
    Global,
    Retain,
    NoRetain,
    LocPartly,
}

impl<'db> ToProto<'db> for Variable<'db> {
    fn spanned(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.range(db).into()
    }

    fn named_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.name_span(db).into()
    }

    fn symbol_info(&'db self, db: &'db dyn BaseDatabase) -> Option<SymbolInfo<'db>> {
        Some(
            SymbolInfo::builder()
                .kind(auto_lsp::lsp_types::SymbolKind::VARIABLE)
                .name(self.name(db).text(db))
                .range(self.range(db).clone())
                .name_range(self.name_span(db).clone())
                .spec(self.spec(db))
                .maybe_init(self.init(db))
                .build(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Spec<'db> {
    Target(NamespaceAccess),
    Array(Array),
    Subrange(Subrange<'db>),
    Expr(Expr<'db>),
    Enum,
    Struct,
    Edge,
    Bool,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Array {
    subrange: Vec<[Ident; 2]>,
}

#[salsa::tracked(debug)]
pub struct Subrange<'db> {
    spec: Spec<'db>,
    lower: Expr<'db>,
    upper: Expr<'db>,
}
