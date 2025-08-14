use auto_lsp::core::span::Span;
use auto_lsp::default::db::file::File;
use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind};

use crate::hir::expressions::expression::{InitExpr, VariableAccess};
use crate::hir::semantic_index::semantic_index;
use crate::{
    completions::snippets::elem_type_names,
    hir::{
        expressions::expression::{Expr, MultibitsPart},
        interned::{identifier::Ident, namespace::NamespaceAccess},
        pous::pou::Pou,
        scopes::{scope::FileScopeId, solver::resolve_namespace_access},
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

    pub scope_id: FileScopeId,

    pub file: File,
}

impl<'db> Spec<'db> {
    pub fn shorthand(&'db self, db: &'db dyn BaseDatabase) -> String {
        match self.kind(db) {
            SpecKind::Target(target) => {
                let sema = semantic_index(db, self.file(db));
                match resolve_namespace_access(db, sema.file, self.scope_id(db), *target) {
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
            _ => self.to_string(db),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum SpecKind<'db> {
    // Simple types
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

    // Composite types
    Struct(Struct<'db>),
    Array(Array<'db>),
    ArrayConformand(Spec<'db>),
    Subrange(SubRange<'db>),
    Enum(Enum<'db>),

    // Reference to another spec
    Ref(Spec<'db>),

    // Targeting a POU or namespace (has to be resolved)
    Target(NamespaceAccess),
}

impl<'db> Spec<'db> {
    pub fn to_string(&'db self, db: &'db dyn BaseDatabase) -> String {
        match self.kind(db) {
            SpecKind::Bool => "BOOL",
            SpecKind::REDGEBool => "BOOL (Rising Edge)",
            SpecKind::FEDGEBool => "BOOL (Falling Edge)",
            SpecKind::Byte => "BYTE",
            SpecKind::Word => "WORD",
            SpecKind::DWord => "DWORD",
            SpecKind::LWord => "LWORD",
            SpecKind::SInt => "SINT",
            SpecKind::USInt => "USINT",
            SpecKind::UInt => "UINT",
            SpecKind::Int => "INT",
            SpecKind::DInt => "DINT",
            SpecKind::UDInt => "UDINT",
            SpecKind::LInt => "LINT",
            SpecKind::ULInt => "ULINT",
            SpecKind::Real => "REAL",
            SpecKind::LReal => "LREAL",
            SpecKind::String => "STRING",
            SpecKind::WString => "WSTRING",
            SpecKind::Char => "CHAR",
            SpecKind::WChar => "WCHAR",
            SpecKind::Date => "DATE",
            SpecKind::LDate => "LDATE",
            SpecKind::Dt => "DATE_AND_TIME",
            SpecKind::Ldt => "LDATE_AND_TIME",
            SpecKind::Time => "TIME",
            SpecKind::LTime => "LTIME",
            SpecKind::Tod => "TIME_OF_DAY",
            SpecKind::LTod => "LTIME_OF_DAY",
            SpecKind::Struct(_) => "STRUCT",
            SpecKind::Array(_) => "ARRAY",
            SpecKind::Subrange(_) => "SUBRANGE",
            SpecKind::Enum(_) => "ENUM",
            _ => "(unknown spec)",
        }
        .to_string()
    }

    pub fn with_details(&self, db: &'db dyn BaseDatabase) -> String {
        match self.kind(db) {
            SpecKind::Bool => "BOOL",
            SpecKind::REDGEBool => "BOOL (Rising Edge)",
            SpecKind::FEDGEBool => "BOOL (Falling Edge)",
            SpecKind::Byte => "BYTE (8-bit)",
            SpecKind::Word => "WORD (16-bit)",
            SpecKind::DWord => "DWORD (32-bit)",
            SpecKind::LWord => "LWORD (64-bit)",
            SpecKind::SInt => "SINT (-128 to 127)",
            SpecKind::USInt => "USINT (0 to 255)",
            SpecKind::UInt => "UINT (0 to 65535)",
            SpecKind::Int => "INT (-32768 to 32767)",
            SpecKind::DInt => "DINT (-2147483648 to 2147483647)",
            SpecKind::UDInt => "UDINT (0 to 4294967295)",
            SpecKind::LInt => "LINT (-9223372036854775808 to 9223372036854775807)",
            SpecKind::ULInt => "ULINT (0 to 18446744073709551615)",
            SpecKind::Real => "REAL (32-bit floating point)",
            SpecKind::LReal => "LREAL (64-bit floating point)",
            SpecKind::String => "STRING (UTF-8)",
            SpecKind::WString => "WSTRING (UTF-16)",
            SpecKind::Char => "CHAR (8-bit character)",
            SpecKind::WChar => "WCHAR (16-bit character)",
            SpecKind::Date => "DATE (YYYY-MM-DD)",
            SpecKind::LDate => "LDATE (YYYY-MM-DD)",
            SpecKind::Dt => "DATE_AND_TIME (YYYY-MM-DD HH:MM:SS)",
            SpecKind::Ldt => "LDATE_AND_TIME (YYYY-MM-DD HH:MM:SS)",
            SpecKind::Time => "TIME (DD:HH:MM:SS)",
            SpecKind::LTime => "LTIME (DD:HH:MM:SS)",
            SpecKind::Tod => "TIME_OF_DAY (HH:MM:SS)",
            SpecKind::LTod => "LTIME_OF_DAY (HH:MM:SS)",
            _ => "(unknown spec)",
        }
        .to_string()
    }
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
            SpecKind::Target(target) => {
                match resolve_namespace_access(db, sema.file, self.scope_id(db), *target) {
                    Some(pou) => pou.hover(db, sema),
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
            _ => Some(Hover {
                range: Some(self.span(db).into()),
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("```typescript\n{}\n```", self.with_details(db)),
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

        primary.extend(finder.iter().filter_map(|(name, pou)| match pou.pou(db) {
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
    pub located: Option<VariableAccess<'db>>,
    pub multibits: Option<MultibitsPart>,
    pub spec: Spec<'db>,
    pub init: Option<InitExpr<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Enum<'db> {
    pub typ: Option<Spec<'db>>,
    pub variants: Vec<EnumVariant<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct EnumVariant<'db> {
    pub name: Ident,
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
