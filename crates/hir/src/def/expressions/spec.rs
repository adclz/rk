use crate::completions::snippets::elem_type_names;
use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind};

use crate::def::expressions::expression::{InitExpr, VariableAccess};
use crate::def::semantic_index::semantic_index;
use crate::to_proto::AstId;
use crate::ty::name_res::resolve_namespace_access;
use crate::{
    def::{
        expressions::expression::{Expr, MultibitsPart},
        interned::{identifier::Ident, namespace::NamespaceAccess},
        pous::pou::Pou,
        scope::FileScopeId,
    },
    to_proto::ToProto,
};

#[salsa::tracked(debug)]
pub struct Spec<'db> {
    #[tracked]
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
                match resolve_namespace_access(db, self.scope_id(db), *target) {
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
    Target(NamespaceAccess),
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

impl<'db> ToProto<'db> for Spec<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id(db)
    }

    fn hover(&'db self, db: &'db dyn BaseDatabase) -> Option<Hover> {
        let sema = semantic_index(db, self.scope_id(db).file(db));
        match self.kind(db) {
            SpecKind::Target(target) => {
                match resolve_namespace_access(db, self.scope_id(db), *target) {
                    Some(pou) => pou.hover(db),
                    None => None,
                }
            }
            SpecKind::Ref(_) => Some(Hover {
                range: Some(self.get_span(db).into()),
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
                range: Some(self.get_span(db).into()),
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: "".into(),
                }),
            }),
        }
    }

    fn completion(
        &'db self,
        db: &'db dyn BaseDatabase,
        _offset: usize,
    ) -> Option<Vec<auto_lsp::lsp_types::CompletionItem>> {
        let mut primary = elem_type_names();
        let sema = semantic_index(db, self.scope_id(db).file(db));
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
