use auto_lsp::default::db::BaseDatabase;
use auto_lsp::{core::span::Span, default::db::file::File};

use crate::hir::semantic_index::semantic_index;
use crate::hir::signature::signature_for_pou;
use crate::{
    completions::snippets::elem_type_names,
    hir::{
        expressions::expression::{Expr, MultibitsPart},
        interned::{identifier::Ident, namespace::NamespaceAccess},
        pous::pou::Pou,
        scopes::{scope::ScopeId, solver::resolve_access},
        semantic_index::SemanticIndex,
    },
    to_proto::{self_iter, IterToProto, ToProto},
};

#[salsa::tracked(debug)]
pub struct Spec<'db> {
    #[returns(ref)]
    pub span: Span,

    #[tracked]
    #[returns(ref)]
    pub kind: SpecKind<'db>,

    pub scope_id: ScopeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum SpecKind<'db> {
    Simple(SimpleSpecKind),
    Composite(CompositeSpecKind<'db>),
    //ConstantExpr(Expr<'db>),
    Target(NamespaceAccess),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum SimpleSpecKind {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum CompositeSpecKind<'db> {
    Struct(Struct<'db>),
    Array(Array<'db>),
    Subrange(SubRange<'db>),
    Enum(Enum<'db>),
}

impl<'db> Spec<'db> {
    pub fn to_string(&self, db: &'db dyn BaseDatabase, sema: &'db SemanticIndex<'db>) -> &str {
        match self.kind(db) {
            SpecKind::Simple(simple_kind) => match simple_kind {
                SimpleSpecKind::SInt => "SINT",
                SimpleSpecKind::Int => "INT",
                SimpleSpecKind::DInt => "DINT",
                SimpleSpecKind::LInt => "LINT",
                SimpleSpecKind::USInt => "USINT",
                SimpleSpecKind::UInt => "UINT",
                SimpleSpecKind::UDInt => "UDINT",
                SimpleSpecKind::ULInt => "ULINT",
                SimpleSpecKind::REDGEBool => "BOOL (Rising Edge)",
                SimpleSpecKind::FEDGEBool => "BOOL (Falling Edge)",
                SimpleSpecKind::Byte => "BYTE",
                SimpleSpecKind::Word => "WORD",
                SimpleSpecKind::DWord => "DWORD",
                SimpleSpecKind::LWord => "LWORD",
                SimpleSpecKind::Date => "DATE",
                SimpleSpecKind::LDate => "LDATE",
                SimpleSpecKind::Dt => "DATE_AND_TIME",
                SimpleSpecKind::Ldt => "LDATE_AND_TIME",
                SimpleSpecKind::Tod => "TIME_OF_DAY",
                SimpleSpecKind::LTod => "LTIME_OF_DAY",
                SimpleSpecKind::Time => "TIME",
                SimpleSpecKind::LTime => "LTIME",
                SimpleSpecKind::Bool => "BOOL",
                SimpleSpecKind::Real => "REAL",
                SimpleSpecKind::LReal => "LREAL",
                SimpleSpecKind::String => "STRING",
                SimpleSpecKind::WString => "WSTRING",
                SimpleSpecKind::Char => "CHAR",
                SimpleSpecKind::WChar => "WCHAR",
            },
            SpecKind::Composite(composite_kind) => match composite_kind {
                CompositeSpecKind::Struct(_) => "STRUCT",
                CompositeSpecKind::Array(_) => "ARRAY",
                CompositeSpecKind::Subrange(_) => "SUBRANGE",
                CompositeSpecKind::Enum(_) => "ENUM",
            },
            SpecKind::Target(target) => {
                match resolve_access(db, sema.file, self.scope_id(db), *target) {
                    Some(pou) => Box::leak(
                        format!("{:?}", signature_for_pou(db, sema.pou_keys[&pou.0]))
                            .into_boxed_str(),
                    ),
                    None => "{unknown}",
                }
            }
        }
    }
}

impl<'db> ToProto<'db> for Spec<'db> {
    fn get_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.span(db)
    }

    fn hover(&'db self, db: &'db dyn crate::BaseDatabase, sema: &'db SemanticIndex<'db>,) -> Option<auto_lsp::lsp_types::Hover> {
        Some(auto_lsp::lsp_types::Hover {
            contents: auto_lsp::lsp_types::HoverContents::Scalar(
                auto_lsp::lsp_types::MarkedString::String(self.to_string(db, sema).to_string()),
            ),
            range: None,
        })
    }

    fn completion(
        &'db self,
        db: &'db dyn crate::BaseDatabase,
        sema: &'db SemanticIndex<'db>,
        _offset: usize,
    ) -> Option<Vec<auto_lsp::lsp_types::CompletionItem>> {
        let mut primary = elem_type_names();
        primary.extend(
            sema.pou_iterator(db, self.scope_id(db))
                .filter_map(|(name, pou)| {
                    let pou = sema.get_pou(pou.0);
                    match pou.pou(db) {
                        Pou::DataType(fb) => Some(auto_lsp::lsp_types::CompletionItem {
                            label: pou.name(db).text(db).to_string(),
                            kind: Some(auto_lsp::lsp_types::CompletionItemKind::TYPE_PARAMETER),
                            detail: Some("TYPE".to_string()),
                            documentation: None,
                            ..Default::default()
                        }),
                        Pou::Function(dt) => Some(auto_lsp::lsp_types::CompletionItem {
                            label: pou.name(db).text(db).to_string(),
                            kind: Some(auto_lsp::lsp_types::CompletionItemKind::FUNCTION),
                            detail: Some("FUNCTION".to_string()),
                            documentation: None,
                            ..Default::default()
                        }),
                        _ => None,
                    }
                }),
        );
        Some(primary)
    }
}

impl<'db> IterToProto<'db> for Spec<'db> {
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        match self.kind(db) {
            _ => self_iter(self),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Struct<'db> {
    pub overlap: bool,
    pub elements: Vec<StructElement<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct StructElement<'db> {
    pub name: Ident,
    pub located: Located,
    // Parameters can be of any type
    pub spec: Spec<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Located {
    adress: Ident,
    partly: bool,
    offset: Option<Ident>,
    multibits: Option<MultibitsPart>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Enum<'db> {
    Anonymous(Vec<Ident>),
    // Each enum variant has a value
    // Values must be integers
    Named(Vec<(Ident, Expr<'db>)>),
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
