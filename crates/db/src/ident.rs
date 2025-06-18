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

    #[salsa::tracked]
    pub fn as_f32(self, db: &'db dyn BaseDatabase) -> Option<f32> {
        self.text(db).as_str().parse::<f32>().ok()
    }

    #[salsa::tracked]
    pub fn as_f64(self, db: &'db dyn BaseDatabase) -> Option<f64> {
        self.text(db).as_str().parse::<f64>().ok()
    }
}

#[salsa::tracked]
impl Ident {
    // Bit string data types

    #[salsa::tracked]
    pub fn is_BOOL(self, db: &dyn BaseDatabase) -> bool {
        self.as_bool(db).is_some()
    }

    #[salsa::tracked]
    pub fn is_BYTE(self, db: &dyn BaseDatabase) -> bool {
        self.as_u8(db).is_some()
    }

    #[salsa::tracked]
    pub fn is_WORD(self, db: &dyn BaseDatabase) -> bool {
        self.as_u16(db).is_some()
    }

    #[salsa::tracked]
    pub fn is_DWORD(self, db: &dyn BaseDatabase) -> bool {
        self.as_u32(db).is_some()
    }

    #[salsa::tracked]
    pub fn is_LWORD(self, db: &dyn BaseDatabase) -> bool {
        self.as_u64(db).is_some()
    }

    // Signed integer data types

    #[salsa::tracked]
    pub fn is_SINT(self, db: &dyn BaseDatabase) -> bool {
        self.as_i8(db).is_some()
    }

    #[salsa::tracked]
    pub fn is_INT(self, db: &dyn BaseDatabase) -> bool {
        self.as_i16(db).is_some()
    }

    #[salsa::tracked]
    pub fn is_DINT(self, db: &dyn BaseDatabase) -> bool {
        self.as_i32(db).is_some()
    }

    #[salsa::tracked]
    pub fn is_LINT(self, db: &dyn BaseDatabase) -> bool {
        self.as_i64(db).is_some()
    }

    // Unsigned integer data types

    #[salsa::tracked]
    pub fn is_USINT(self, db: &dyn BaseDatabase) -> bool {
        self.as_u8(db).is_some()
    }

    #[salsa::tracked]
    pub fn is_UINT(self, db: &dyn BaseDatabase) -> bool {
        self.as_u16(db).is_some()
    }

    #[salsa::tracked]
    pub fn is_UDINT(self, db: &dyn BaseDatabase) -> bool {
        self.as_u32(db).is_some()
    }

    #[salsa::tracked]
    pub fn is_ULINT(self, db: &dyn BaseDatabase) -> bool {
        self.as_u64(db).is_some()
    }

    // Real (floating-point) data types

    #[salsa::tracked]
    pub fn is_REAL(self, db: &dyn BaseDatabase) -> bool {
        self.as_f32(db).is_some()
    }

    #[salsa::tracked]
    pub fn is_LREAL(self, db: &dyn BaseDatabase) -> bool {
        self.as_f64(db).is_some()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    use crate::RootDatabase;

    #[test]
    fn is_BOOL() {
        let db = RootDatabase::default();
        assert_eq!(Ident::new(&db, "TRUE".to_string()).is_BOOL(&db), true);
        assert_eq!(Ident::new(&db, "FALSE".to_string()).is_BOOL(&db), true);
        assert_eq!(Ident::new(&db, "1".to_string()).is_BOOL(&db), true);
        assert_eq!(Ident::new(&db, "0".to_string()).is_BOOL(&db), true);
        assert_eq!(Ident::new(&db, "2".to_string()).is_BOOL(&db), false);
    }

    #[test]
    fn is_BYTE() {
        let db = RootDatabase::default();
        assert_eq!(Ident::new(&db, "0".to_string()).is_BYTE(&db), true);
        assert_eq!(Ident::new(&db, "255".to_string()).is_BYTE(&db), true);
        assert_eq!(Ident::new(&db, "256".to_string()).is_BYTE(&db), false);
    }

    #[test]
    fn is_WORD() {
        let db = RootDatabase::default();
        assert_eq!(Ident::new(&db, "0".to_string()).is_WORD(&db), true);
        assert_eq!(Ident::new(&db, "65535".to_string()).is_WORD(&db), true);
        assert_eq!(Ident::new(&db, "65536".to_string()).is_WORD(&db), false);
        assert_eq!(Ident::new(&db, "-1".to_string()).is_WORD(&db), false);
    }

    #[test]
    fn is_DWORD() {
        let db = RootDatabase::default();
        assert_eq!(Ident::new(&db, "0".to_string()).is_DWORD(&db), true);
        assert_eq!(Ident::new(&db, "4294967295".to_string()).is_DWORD(&db), true);
        assert_eq!(Ident::new(&db, "4294967296".to_string()).is_DWORD(&db), false);
        assert_eq!(Ident::new(&db, "-1".to_string()).is_DWORD(&db), false);
    }

    #[test]
    fn is_LWORD() {
        let db = RootDatabase::default();
        assert_eq!(Ident::new(&db, "0".to_string()).is_LWORD(&db), true);
        assert_eq!(Ident::new(&db, "18446744073709551615".to_string()).is_LWORD(&db), true);
        assert_eq!(Ident::new(&db, "18446744073709551616".to_string()).is_LWORD(&db), false);
        assert_eq!(Ident::new(&db, "-1".to_string()).is_LWORD(&db), false);
    }

    #[test]
    fn is_SINT() {
        let db = RootDatabase::default();
        assert_eq!(Ident::new(&db, "0".to_string()).is_SINT(&db), true);
        assert_eq!(Ident::new(&db, "127".to_string()).is_SINT(&db), true);
        assert_eq!(Ident::new(&db, "128".to_string()).is_SINT(&db), false);
        assert_eq!(Ident::new(&db, "-128".to_string()).is_SINT(&db), true);
        assert_eq!(Ident::new(&db, "-129".to_string()).is_SINT(&db), false);
    }

    #[test]
    fn is_INT() {
        let db = RootDatabase::default();
        assert_eq!(Ident::new(&db, "0".to_string()).is_INT(&db), true);
        assert_eq!(Ident::new(&db, "32767".to_string()).is_INT(&db), true);
        assert_eq!(Ident::new(&db, "32768".to_string()).is_INT(&db), false);
        assert_eq!(Ident::new(&db, "-32768".to_string()).is_INT(&db), true);
        assert_eq!(Ident::new(&db, "-32769".to_string()).is_INT(&db), false);
    }

    #[test]
    fn is_DINT() {
        let db = RootDatabase::default();
        assert_eq!(Ident::new(&db, "0".to_string()).is_DINT(&db), true);
        assert_eq!(Ident::new(&db, "2147483647".to_string()).is_DINT(&db), true);
        assert_eq!(Ident::new(&db, "2147483648".to_string()).is_DINT(&db), false);
        assert_eq!(Ident::new(&db, "-2147483648".to_string()).is_DINT(&db), true);
        assert_eq!(Ident::new(&db, "-2147483649".to_string()).is_DINT(&db), false);
    }

    #[test]
    fn is_LINT() {
        let db = RootDatabase::default();
        assert_eq!(Ident::new(&db, "0".to_string()).is_LINT(&db), true);
        assert_eq!(Ident::new(&db, "9223372036854775807".to_string()).is_LINT(&db), true);
        assert_eq!(Ident::new(&db, "9223372036854775808".to_string()).is_LINT(&db), false);
        assert_eq!(Ident::new(&db, "-9223372036854775808".to_string()).is_LINT(&db), true);
        assert_eq!(Ident::new(&db, "-9223372036854775809".to_string()).is_LINT(&db), false);
    }

    #[test]
    fn is_USINT() {
        let db = RootDatabase::default();
        assert_eq!(Ident::new(&db, "0".to_string()).is_USINT(&db), true);
        assert_eq!(Ident::new(&db, "255".to_string()).is_USINT(&db), true);
        assert_eq!(Ident::new(&db, "256".to_string()).is_USINT(&db), false);
        assert_eq!(Ident::new(&db, "-1".to_string()).is_USINT(&db), false);
    }

    #[test]
    fn is_UINT() {
        let db = RootDatabase::default();
        assert_eq!(Ident::new(&db, "0".to_string()).is_UINT(&db), true);
        assert_eq!(Ident::new(&db, "65535".to_string()).is_UINT(&db), true);
        assert_eq!(Ident::new(&db, "65536".to_string()).is_UINT(&db), false);
        assert_eq!(Ident::new(&db, "-1".to_string()).is_UINT(&db), false);
    }

    #[test]
    fn is_UDINT() {
        let db = RootDatabase::default();
        assert_eq!(Ident::new(&db, "0".to_string()).is_UDINT(&db), true);
        assert_eq!(Ident::new(&db, "4294967295".to_string()).is_UDINT(&db), true);
        assert_eq!(Ident::new(&db, "4294967296".to_string()).is_UDINT(&db), false);
        assert_eq!(Ident::new(&db, "-1".to_string()).is_UDINT(&db), false);
    }

    #[test]
    fn is_ULINT() {
        let db = RootDatabase::default();
        assert_eq!(Ident::new(&db, "0".to_string()).is_ULINT(&db), true);
        assert_eq!(Ident::new(&db, "18446744073709551615".to_string()).is_ULINT(&db), true);
        assert_eq!(Ident::new(&db, "18446744073709551616".to_string()).is_ULINT(&db), false);
        assert_eq!(Ident::new(&db, "-1".to_string()).is_ULINT(&db), false);
    }       

    #[test]
    fn is_REAL() {
        let db = RootDatabase::default();
        assert_eq!(Ident::new(&db, "0.0".to_string()).is_REAL(&db), true);
        assert_eq!(Ident::new(&db, "1.0".to_string()).is_REAL(&db), true);
        assert_eq!(Ident::new(&db, "1.1".to_string()).is_REAL(&db), true);
        assert_eq!(Ident::new(&db, "1.1e1".to_string()).is_REAL(&db), true);
        assert_eq!(Ident::new(&db, "1.1e-1".to_string()).is_REAL(&db), true);
        assert_eq!(Ident::new(&db, "1.1e+1".to_string()).is_REAL(&db), true);
        assert_eq!(Ident::new(&db, "1.1e1.1".to_string()).is_REAL(&db), false);
    }

    #[test]
    fn is_LREAL() {
        let db = RootDatabase::default();
        assert_eq!(Ident::new(&db, "0.0".to_string()).is_LREAL(&db), true);
        assert_eq!(Ident::new(&db, "1.0".to_string()).is_LREAL(&db), true);
        assert_eq!(Ident::new(&db, "1.1".to_string()).is_LREAL(&db), true);
        assert_eq!(Ident::new(&db, "1.1e1".to_string()).is_LREAL(&db), true);
        assert_eq!(Ident::new(&db, "1.1e-1".to_string()).is_LREAL(&db), true);
        assert_eq!(Ident::new(&db, "1.1e+1".to_string()).is_LREAL(&db), true);
        assert_eq!(Ident::new(&db, "1.1e1.1".to_string()).is_LREAL(&db), false);
    }
} 
