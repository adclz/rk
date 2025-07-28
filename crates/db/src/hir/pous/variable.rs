use auto_lsp::{
    core::span::Span,
    default::db::{file::File, BaseDatabase}, lsp_types::{MarkupContent, MarkupKind},
};

use crate::{
    hir::{expressions::expression::Expr, interned::{identifier::Ident, namespace::NamespaceAccess}, semantic_index::SemanticIndex}, to_proto::{self_iter, IterToProto, SymbolInfo, ToProto}
};

#[salsa::tracked(debug)]
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
    #[returns(ref)]
    pub spec: Spec<'db>,

    #[tracked]
    #[returns(as_ref)]
    pub init: Option<Expr<'db>>
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
    fn get_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.range(db).into()
    }

    fn get_named_span(&'db self, db: &'db dyn crate::BaseDatabase) -> Option<&'db Span> {
        Some(self.name_span(db))
    }

    fn symbol_info(&'db self, db: &'db dyn BaseDatabase) -> Option<SymbolInfo<'db>> {
        Some(
            SymbolInfo::builder()
                .kind(auto_lsp::lsp_types::SymbolKind::VARIABLE)
                .name(self.name(db).text(db))
                .range(self.range(db).clone())
                .name_range(self.name_span(db).clone())
                .spec(self.spec(db).clone())
                .maybe_init(self.init(db).cloned())
                .build(),
        )
    }

    fn hover(&'db self, db: &'db dyn crate::BaseDatabase) -> Option<auto_lsp::lsp_types::Hover> {
        Some(auto_lsp::lsp_types::Hover {
            contents: auto_lsp::lsp_types::HoverContents::Markup(
                MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("Variable {}", self.name(db).text(db)).to_string()
                }
            ),
            range: Some(self.name_span(db).into())
        })
    }
}

impl<'db> IterToProto<'db> for Variable<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase, sema: &'db SemanticIndex) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        self_iter(self)
            .chain(self.spec(db).iter(db, sema))
            .chain(self.init(db).into_iter().map(|i| i as _))
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Spec<'db> {
    pub span: Span,
    pub kind: SpecKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum SpecKind<'db> {
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
impl<'db> Spec<'db> {
    pub fn to_string(&self, db: &'db dyn BaseDatabase) -> &str {
        match self.kind {
            SpecKind::SInt => "SINT",
            SpecKind::Int => "INT",
            SpecKind::DInt => "DINT",
            SpecKind::LInt => "LINT",
            SpecKind::USInt => "USINT",
            SpecKind::UInt => "UINT",
            SpecKind::UDInt => "UDINT",
            SpecKind::ULInt => "ULINT",
            SpecKind::Byte => "BYTE",
            SpecKind::Word => "WORD",
            SpecKind::DWord => "DWORD",
            SpecKind::LWord => "LWORD",
            SpecKind::Date => "DATE",
            SpecKind::LDate => "LDATE",
            SpecKind::Dt => "DATE_AND_TIME",
            SpecKind::Ldt => "LDATE_AND_TIME",
            SpecKind::Tod => "TIME_OF_DAY",
            SpecKind::LTod => "LTIME_OF_DAY",
            SpecKind::Time => "TIME",
            SpecKind::LTime => "LTIME",
            _ => "?"
        }
    }
}

impl<'db> ToProto<'db> for Spec<'db> {
    fn get_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        &self.span
    }

    fn hover(&'db self, db: &'db dyn crate::BaseDatabase) -> Option<auto_lsp::lsp_types::Hover> {
        Some(auto_lsp::lsp_types::Hover {
            contents: auto_lsp::lsp_types::HoverContents::Scalar(
                auto_lsp::lsp_types::MarkedString::String(self.to_string(db).to_string()),
            ),
            range: None,
        })
    }
}

impl<'db> IterToProto<'db> for Spec<'db> {
    #[auto_enums::auto_enum(Iterator)]
    fn iter(&'db self, db: &'db dyn BaseDatabase, sema: &'db SemanticIndex) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        match &self.kind {
            SpecKind::Expr(expr) => self_iter(self).chain(expr.iter(db, sema)),
            _ => self_iter(self),
        }
    }
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
