use auto_lsp::{
    anyhow,
    core::{ast::AstNode, span::Span},
    default::db::{file::File, BaseDatabase},
};
use compact_str::CompactString;
use std::hash::Hash;

use crate::to_proto::ToProto;

#[derive(Clone, Eq, salsa::Update, Debug)]
pub struct SpannedIdent {
    pub span: Span,
    pub ident: Ident,
}

impl PartialEq for SpannedIdent {
    fn eq(&self, other: &Self) -> bool {
        self.ident == other.ident
    }
}

impl PartialEq<Ident> for SpannedIdent {
    fn eq(&self, other: &Ident) -> bool {
        self.ident == *other
    }
}

impl Hash for SpannedIdent {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ident.hash(state);
    }
}

impl SpannedIdent {
    pub fn new(db: &dyn BaseDatabase, file: File, ident: &impl AstNode) -> anyhow::Result<Self> {
        Ok(SpannedIdent {
            span: ident.get_span(),
            ident: Ident::from_node(db, file, ident)?,
        })
    }

    #[cfg(debug_assertions)]
    pub fn from_blank(db: &dyn BaseDatabase, text: &str) -> Self {
        use auto_lsp::tree_sitter::Point;
        use auto_lsp::tree_sitter::Range;

        let range = Range {
            start_byte: 0,
            end_byte: 0,
            start_point: Point { row: 0, column: 0 },
            end_point: Point { row: 0, column: 0 },
        };
        SpannedIdent {
            span: Span::from(range),
            ident: Ident::from_slice(db, text),
        }
    }

    pub fn to_string<'db>(&'db self, db: &'db dyn BaseDatabase) -> &'db str {
        &self.ident.text(db)
    }
}

impl<'db> ToProto<'db> for SpannedIdent {
    fn get_span(&'db self, db: &'db dyn crate::BaseDatabase) -> &'db Span {
        &self.span
    }
}

/// Interned identifier
#[salsa::interned(debug, no_lifetime)]
pub struct Ident {
    #[returns(ref)]
    pub text: CompactString,
}

#[salsa::tracked]
impl<'db> Ident {
    pub fn from_node(
        db: &dyn BaseDatabase,
        file: File,
        node: &impl AstNode,
    ) -> anyhow::Result<Self> {
        Ok(Ident::new(
            db,
            CompactString::from(node.get_text(file.document(db).as_bytes())?),
        ))
    }

    pub fn from_slice(db: &dyn BaseDatabase, text: &str) -> Self {
        Ident::new(db, CompactString::from(text))
    }

    pub fn join(db: &dyn BaseDatabase, other: &[Ident]) -> Ident {
        Ident::new(
            db,
            other
                .iter()
                .map(|i| i.text(db).to_owned())
                .collect::<CompactString>(),
        )
    }
}
