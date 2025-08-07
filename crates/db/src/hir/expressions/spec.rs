use auto_lsp::core::span::Span;
use auto_lsp::default::db::file::File;
use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind};

use crate::hir::scopes::solver::pous_in_scope;
use crate::hir::semantic_index::semantic_index;
use crate::{
    completions::snippets::elem_type_names,
    hir::{
        expressions::expression::{Expr, MultibitsPart},
        interned::{identifier::Ident, namespace::NamespaceAccess},
        pous::pou::Pou,
        scopes::{scope::ScopeId, solver::resolve_namespace_access},
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

    pub file: File,
}

impl<'db> Spec<'db> {
    pub fn shorthand(&'db self, db: &'db dyn BaseDatabase) -> String {
        match self.kind(db) {
            SpecKind::Simple(simple_kind) => simple_kind.to_string(),
            SpecKind::Target(target) => {
                let sema = semantic_index(db, self.file(db));
                match resolve_namespace_access(db, sema.file, self.scope_id(db), *target) {
                    Some(pou) => {
                        match pou.pou(db) {
                            Pou::Function(dt) => format!("(function) {}", pou.name(db).text(db)),
                            Pou::FunctionBlock(fb) => {
                                format!("(function_block) {}", pou.name(db).text(db))
                            }
                            Pou::DataType(dt) => dt.spec(db).shorthand(db),
                            Pou::Class(class) => format!("(class) {}", pou.name(db).text(db)),
                            Pou::Interface(it) => format!("(interface) {}", pou.name(db).text(db)),
                        }
                    }
                    None => "{unknown}".to_string(),
                }
            }
            SpecKind::Composite(cmp) => match cmp {
                CompositeSpecKind::Array(arr) => {
                    let of_type = arr.of_type.shorthand(db);
                    format!("(array) {of_type}")
                }
                CompositeSpecKind::Struct(st) => {
                    let elements = st
                        .elements
                        .iter()
                        .map(|e| format!("{}: {}", e.name.text(db), e.spec.shorthand(db)))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("(struct) {{{elements}}}")
                }
                CompositeSpecKind::Subrange(sr) => "(subrange)".to_string(),
                CompositeSpecKind::Enum(en) => match en {
                    Enum::Anonymous(variants) => "(enum)".to_string(),
                    Enum::Named(variants) => "(enum)".to_string(),
                },
            },
            SpecKind::Ref(ref_name) => {
                format!("(*ref*) {}", ref_name.shorthand(db))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum SpecKind<'db> {
    Simple(SimpleSpecKind),
    Composite(CompositeSpecKind<'db>),
    Ref(Spec<'db>),
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

impl SimpleSpecKind {
    pub fn to_string(&self) -> String {
        match self {
            SimpleSpecKind::Bool => "BOOL",
            SimpleSpecKind::REDGEBool => "BOOL (Rising Edge)",
            SimpleSpecKind::FEDGEBool => "BOOL (Falling Edge)",
            SimpleSpecKind::Byte => "BYTE",
            SimpleSpecKind::Word => "WORD",
            SimpleSpecKind::DWord => "DWORD",
            SimpleSpecKind::LWord => "LWORD",
            SimpleSpecKind::SInt => "SINT",
            SimpleSpecKind::USInt => "USINT",
            SimpleSpecKind::UInt => "UINT",
            SimpleSpecKind::Int => "INT",
            SimpleSpecKind::DInt => "DINT",
            SimpleSpecKind::UDInt => "UDINT",
            SimpleSpecKind::LInt => "LINT",
            SimpleSpecKind::ULInt => "ULINT",
            SimpleSpecKind::Real => "REAL",
            SimpleSpecKind::LReal => "LREAL",
            SimpleSpecKind::String => "STRING",
            SimpleSpecKind::WString => "WSTRING",
            SimpleSpecKind::Char => "CHAR",
            SimpleSpecKind::WChar => "WCHAR",
            SimpleSpecKind::Date => "DATE",
            SimpleSpecKind::LDate => "LDATE",
            SimpleSpecKind::Dt => "DATE_AND_TIME",
            SimpleSpecKind::Ldt => "LDATE_AND_TIME",
            SimpleSpecKind::Time => "TIME",
            SimpleSpecKind::LTime => "LTIME",
            SimpleSpecKind::Tod => "TIME_OF_DAY",
            SimpleSpecKind::LTod => "LTIME_OF_DAY",
        }
        .to_string()
    }

    pub fn with_details(&self) -> String {
        match self {
            SimpleSpecKind::Bool => "BOOL",
            SimpleSpecKind::REDGEBool => "BOOL (Rising Edge)",
            SimpleSpecKind::FEDGEBool => "BOOL (Falling Edge)",
            SimpleSpecKind::Byte => "BYTE (8-bit)",
            SimpleSpecKind::Word => "WORD (16-bit)",
            SimpleSpecKind::DWord => "DWORD (32-bit)",
            SimpleSpecKind::LWord => "LWORD (64-bit)",
            SimpleSpecKind::SInt => "SINT (-128 to 127)",
            SimpleSpecKind::USInt => "USINT (0 to 255)",
            SimpleSpecKind::UInt => "UINT (0 to 65535)",
            SimpleSpecKind::Int => "INT (-32768 to 32767)",
            SimpleSpecKind::DInt => "DINT (-2147483648 to 2147483647)",
            SimpleSpecKind::UDInt => "UDINT (0 to 4294967295)",
            SimpleSpecKind::LInt => "LINT (-9223372036854775808 to 9223372036854775807)",
            SimpleSpecKind::ULInt => "ULINT (0 to 18446744073709551615)",
            SimpleSpecKind::Real => "REAL (32-bit floating point)",
            SimpleSpecKind::LReal => "LREAL (64-bit floating point)",
            SimpleSpecKind::String => "STRING (UTF-8)",
            SimpleSpecKind::WString => "WSTRING (UTF-16)",
            SimpleSpecKind::Char => "CHAR (8-bit character)",
            SimpleSpecKind::WChar => "WCHAR (16-bit character)",
            SimpleSpecKind::Date => "DATE (YYYY-MM-DD)",
            SimpleSpecKind::LDate => "LDATE (YYYY-MM-DD)",
            SimpleSpecKind::Dt => "DATE_AND_TIME (YYYY-MM-DD HH:MM:SS)",
            SimpleSpecKind::Ldt => "LDATE_AND_TIME (YYYY-MM-DD HH:MM:SS)",
            SimpleSpecKind::Time => "TIME (DD:HH:MM:SS)",
            SimpleSpecKind::LTime => "LTIME (DD:HH:MM:SS)",
            SimpleSpecKind::Tod => "TIME_OF_DAY (HH:MM:SS)",
            SimpleSpecKind::LTod => "LTIME_OF_DAY (HH:MM:SS)",
        }
        .to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum CompositeSpecKind<'db> {
    Struct(Struct<'db>),
    Array(Array<'db>),
    Subrange(SubRange<'db>),
    Enum(Enum<'db>),
}

impl<'db> ToProto<'db> for Spec<'db> {
    fn get_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.span(db)
    }

    fn hover(
        &'db self,
        db: &'db dyn crate::BaseDatabase,
        sema: &'db SemanticIndex<'db>,
    ) -> Option<Hover> {
        match self.kind(db) {
            SpecKind::Simple(simple_kind) => Some(Hover {
                range: Some(self.span(db).into()),
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("```typescript\n{}\n```", simple_kind.with_details()),
                }),
            }),
            SpecKind::Composite(composite_kind) => todo!(),
            SpecKind::Target(target) => {
                match resolve_namespace_access(db, sema.file, self.scope_id(db), *target) {
                    Some(pou) => {
                        pou.hover(db, &sema)
                    }
                    None => None,
                }
            }
            SpecKind::Ref(_) => Some(Hover {
                range: Some(self.span(db).into()),
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!(
                        r#"```typescript
{} 
```"#,
                        self.shorthand(db)
                    ),
                }),
            }),
        }
    }

    fn completion(
        &'db self,
        db: &'db dyn crate::BaseDatabase,
        sema: &'db SemanticIndex<'db>,
        _offset: usize,
    ) -> Option<Vec<auto_lsp::lsp_types::CompletionItem>> {
        let mut primary = elem_type_names();
        let finder = sema.pous_in_scope(db, self.scope_id(db));

        primary.extend(finder.iter().filter_map(|(name, pou)| {
            match pou.pou(db) {
                Pou::DataType(fb) => Some(auto_lsp::lsp_types::CompletionItem {
                    label: pou.name(db).text(db).to_string(),
                    kind: Some(auto_lsp::lsp_types::CompletionItemKind::TYPE_PARAMETER),
                    detail: Some("TYPE".to_string()),
                    documentation: None,
                    ..Default::default()
                }),
                Pou::FunctionBlock(dt) => Some(auto_lsp::lsp_types::CompletionItem {
                    label: pou.name(db).text(db).to_string(),
                    kind: Some(auto_lsp::lsp_types::CompletionItemKind::FUNCTION),
                    detail: Some("FUNCTION_BLOCK".to_string()),
                    documentation: None,
                    ..Default::default()
                }),
                _ => None,
            }
        }));
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
