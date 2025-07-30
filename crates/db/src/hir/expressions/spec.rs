use auto_lsp::core::span::Span;
use auto_lsp::default::db::BaseDatabase;

use crate::{completions::snippets::elem_type_names, hir::{expressions::expression::{Expr, MultibitsPart}, interned::{identifier::Ident, namespace::NamespaceAccess}, pous::pou::Pou, scopes::scope::ScopeId, semantic_index::SemanticIndex}, to_proto::{self_iter, IterToProto, ToProto}};


#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Spec<'db> {
    pub span: Span,
    pub kind: SpecKind<'db>,
    pub scope_id: ScopeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum SpecKind<'db> {
    Target(NamespaceAccess),
    Array(Array<'db>),
    Subrange(SubRangeType<'db>),
    Expr(Expr<'db>),
    Enum(Enum<'db>),
    // StructLike is also used to represent FBs, interfaces, and classes
    StructLike(Struct<'db>),
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
            _ => "?",
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

    fn completion(
        &'db self,
        db: &'db dyn crate::BaseDatabase,
        sema: &'db SemanticIndex<'db>,
        _offset: usize,
    ) -> Option<Vec<auto_lsp::lsp_types::CompletionItem>> {
        let mut primary = elem_type_names();
        primary.extend(
            sema.pou_iterator(db, self.scope_id)
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
    #[auto_enums::auto_enum(Iterator)]
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        match &self.kind {
            SpecKind::Expr(expr) => self_iter(self).chain(expr.iter(db, sema)),
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
    Named(Vec<(Ident, Expr<'db>)>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Array<'db> {
    pub subranges: Vec<SubRange<'db>>,
    pub of_type: Box<Spec<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct SubRange<'db> {
    pub lower: Expr<'db>,
    pub upper: Expr<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct SubRangeType<'db> {
    pub _type: Box<Spec<'db>>,
    pub lower: Expr<'db>,
    pub upper: Expr<'db>,
}