use auto_lsp::default::db::BaseDatabase;

use crate::{ident::Ident};


#[salsa::tracked(debug)]
pub struct Variable<'db> {
    name: Ident,

    kind: VariableKind,
}

impl <'db> Variable<'db> {
    pub fn from(db: &'db dyn BaseDatabase, name: &str, kind: VariableKind) -> Self {
        Self::new(db, Ident::new(db, name.to_string()), kind)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum VariableKind {
    Primitive(PrimitiveKind),
    Edge,
    Array(Array),
    Struct,
}

impl VariableKind {
    pub fn primitive_kind(&self) -> Option<PrimitiveKind> {
        match self {
            Self::Primitive(p) => Some(*p),
            Self::Edge => Some(PrimitiveKind::Bool),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveKind {
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
    LInt,
    ULInt,
    Real,
    LReal,
    String,
    WString
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Array {
    subrange: Vec<[Ident; 2]>
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Bool(Ident),
    BitString(BitString),
    Char(Ident),
    Numeric(Ident),
    Time(Ident),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BitString {
    Binary(Ident),
    Hex(Ident),
    Octal(Ident),
    Unsigned(Ident),
}