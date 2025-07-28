use auto_lsp::{
    anyhow,
    core::{ast::AstNode, span::Span},
    default::db::{file::File, BaseDatabase},
};
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
            ident: Ident::new(db, text.to_string()),
        }
    }

    pub fn to_string(&self, db: &dyn BaseDatabase) -> String {
        self.ident.text(db)
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
    #[return_ref]
    pub text: String,
}

#[salsa::tracked]
impl<'db> Ident {
    pub fn from_node(
        db: &dyn BaseDatabase,
        file: File,
        node: &impl AstNode,
    ) -> anyhow::Result<Self> {
        Ok(Ident::new(db, node.get_text(file.document(db).as_bytes())?))
    }

    pub fn join(db: &dyn BaseDatabase, other: &[Ident]) -> Ident {
        Ident::new(db, other.iter().map(|i| i.text(db)).collect::<String>())
    }

    #[salsa::tracked]
    pub fn as_u8(self, db: &'db dyn BaseDatabase) -> Option<u8> {
        self.text(db).as_str().parse::<u8>().ok()
    }

    pub fn is_u8(self, db: &dyn BaseDatabase) -> bool {
        self.as_u8(db).is_some()
    }

    #[salsa::tracked]
    pub fn as_u16(self, db: &'db dyn BaseDatabase) -> Option<u16> {
        self.text(db).as_str().parse::<u16>().ok()
    }

    #[salsa::tracked]
    pub fn as_u32(self, db: &'db dyn BaseDatabase) -> Option<u32> {
        self.text(db).as_str().parse::<u32>().ok()
    }

    #[salsa::tracked]
    pub fn as_u64(self, db: &'db dyn BaseDatabase) -> Option<u64> {
        self.text(db).as_str().parse::<u64>().ok()
    }

    #[salsa::tracked]
    pub fn as_i8(self, db: &'db dyn BaseDatabase) -> Option<i8> {
        self.text(db).as_str().parse::<i8>().ok()
    }

    #[salsa::tracked]
    pub fn as_i16(self, db: &'db dyn BaseDatabase) -> Option<i16> {
        self.text(db).as_str().parse::<i16>().ok()
    }

    #[salsa::tracked]
    pub fn as_i32(self, db: &'db dyn BaseDatabase) -> Option<i32> {
        self.text(db).as_str().parse::<i32>().ok()
    }

    #[salsa::tracked]
    pub fn as_i64(self, db: &'db dyn BaseDatabase) -> Option<i64> {
        self.text(db).as_str().parse::<i64>().ok()
    }
}
