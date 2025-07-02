use auto_lsp::default::db::BaseDatabase;

use crate::{diagnostics::diagnostic_builder::RangeKind, hir::expression::Expr, ident::Ident, to_proto::{SymbolInfo, ToProto}};


#[salsa::tracked(debug)]
pub struct Variable<'db> {
    #[returns(ref)]
    name: Ident,

    #[returns(ref)]
    pub range: auto_lsp::tree_sitter::Range,

    #[returns(ref)]
    pub name_span: auto_lsp::tree_sitter::Range,

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
    fn spanned(&'db self, db: &'db dyn BaseDatabase) -> RangeKind<'db> {
        self.range(db).into()
    }

    fn named_span(&'db self, db: &'db dyn BaseDatabase) -> RangeKind<'db> {
        self.name_span(db).into()
    }

    fn symbol_info(&'db self, db: &'db dyn BaseDatabase) -> SymbolInfo<'db> {
        SymbolInfo::builder()
        .kind(auto_lsp::lsp_types::SymbolKind::VARIABLE)
        .name(self.name(db).text(db))
        .range(self.range(db).into())
        .spec(self.spec(db))
        .maybe_init(self.init(db))
        .name_range(self.name_span(db).into())
        .build()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Spec<'db> {
    Target(Ident),
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
    subrange: Vec<[Ident; 2]>
}

#[salsa::tracked(debug)]
pub struct Subrange<'db> {
    spec: Spec<'db>,
    lower: Expr<'db>,
    upper: Expr<'db>,
}