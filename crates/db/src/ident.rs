#![allow(non_snake_case)]

use auto_lsp::{anyhow, core::ast::AstNode, default::db::{BaseDatabase, File}};

/// Interned identifier
#[salsa::interned(debug, no_lifetime)]
pub struct Ident {
    #[return_ref]
    pub text: String,
}


#[salsa::tracked]
impl<'db> Ident {
    pub fn from_node(db: &dyn BaseDatabase, file: File, node: &impl AstNode) -> anyhow::Result<Self> {
        Ok(Ident::new(db, node.get_text(file.document(db).as_bytes())?))
    }

    pub fn join(db: &dyn BaseDatabase, other: &[Ident]) -> Ident {
        Ident::new(db, other.iter().map(|i| i.text(db)).collect::<String>())
    }

    #[salsa::tracked]
    pub fn as_bool(self, db: &'db dyn BaseDatabase) -> Option<bool> {
        match self.text(db).as_str() {
            "TRUE" | "1" => Some(true),
            "FALSE" | "0" => Some(false),
            _ => None,
        }
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

#[salsa::tracked]
impl Ident {
    #[salsa::tracked]
    pub fn is_WORD(self, db: &dyn BaseDatabase) -> bool {
        matches!(self.as_u8(db).is_some() || self.as_u16(db).is_some() || self.as_u32(db).is_some() || self.as_u64(db).is_some(), true)
    }

    #[salsa::tracked]
    pub fn is_DWORD(self, db: &dyn BaseDatabase) -> bool {
        matches!(self.as_u32(db).is_some() || self.as_u64(db).is_some(), true)
    }

    #[salsa::tracked]
    pub fn is_LWORD(self, db: &dyn BaseDatabase) -> bool {
        self.as_u64(db).is_some()
    }
}
