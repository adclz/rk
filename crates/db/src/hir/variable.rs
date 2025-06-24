use auto_lsp::default::db::BaseDatabase;

use crate::{ident::Ident, to_proto::{SymbolInfo, ToProto}};


#[salsa::tracked(debug)]
pub struct Variable<'db> {
    #[returns(ref)]
    name: Ident,

    #[returns(ref)]
    pub range: auto_lsp::tree_sitter::Range,

    #[returns(ref)]
    pub name_span: auto_lsp::tree_sitter::Range,
}


impl<'db> ToProto<'db> for Variable<'db> {
    fn symbol_info(&'db self, db: &'db dyn BaseDatabase) -> SymbolInfo<'db> {
        SymbolInfo::builder()
        .kind(auto_lsp::lsp_types::SymbolKind::VARIABLE)
        .name(self.name(db).text(db))
        .range(self.range(db).into())
        .name_range(self.name_span(db).into())
        .build()
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