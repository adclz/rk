use auto_lsp::default::db::BaseDatabase;

use crate::{ident::Ident};


#[salsa::tracked(debug)]
pub struct Variable<'db> {
    name: Ident,
}

impl <'db> Variable<'db> {
    pub fn from(db: &'db dyn BaseDatabase, name: Ident) -> Self {
        Self::new(db, name)
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
    // Any numeric type (non floating point)
    AnyNumeric(Numeric),

    // Signed
    SInt(Numeric),
    Int(Numeric),
    DInt(Numeric),
    LInt(Numeric),

    // Unsigned
    USInt(Numeric),
    UInt(Numeric),
    UDInt(Numeric),
    ULInt(Numeric),

    // Bit string
    Byte(Numeric),
    Word(Numeric),
    DWord(Numeric),
    LWord(Numeric),
    
    Real(Ident),
    LReal(Ident),

    Bool(Ident),

    Char(Ident),
    DChar(Ident),

    Date(Ident),
    LDate(Ident),
    Tod(Ident),
    LTod(Ident),
    Time(Ident),
    LTime(Ident),
    DateTime(Ident),
    LDateTime(Ident),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Numeric {
    Binary(Ident),
    Hex(Ident),
    Octal(Ident),
    Signed(Ident),
}