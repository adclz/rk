use crate::{hir_def::expressions::spec::ElementarySpec, hir_ty::ty::Type};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ExplicitCast<'db> {
    from: Type<'db>,
    to: Type<'db>,
}

impl ElementarySpec {
    /// Implicit casts according to IEC 61131-3 standard
    ///
    /// See 6.6.1.6 Data type conversion
    pub fn implicit_cast(&self, typ: ElementarySpec) -> Option<ElementarySpec> {
        use ElementarySpec::*;

        Some(match typ {
            // BOOL BYTE WORD DWORD LWORD
            Bool | REDGEBool | FEDGEBool => match self {
                Byte => Byte,
                Word => Word,
                DWord => DWord,
                LWord => LWord,
                _ => None?,
            },
            Byte => match self {
                Word => Word,
                DWord => DWord,
                LWord => LWord,
                _ => None?,
            },
            Word => match self {
                DWord => DWord,
                LWord => LWord,
                _ => None?,
            },
            DWord => match self {
                LWord => LWord,
                _ => None?,
            },
            // SINT INT DINT LINT
            SInt => match self {
                Int => Int,
                DInt => DInt,
                LInt => LInt,
                Real => Real,
                LReal => LReal,
                _ => None?,
            },
            Int => match self {
                DInt => DInt,
                LInt => LInt,
                Real => Real,
                LReal => LReal,
                _ => None?,
            },
            // DINT LREAL
            DInt => match self {
                LInt => LInt,
                // no real (see table in standard)
                LReal => LReal,
                _ => None?,
            },
            // REAL LREAL
            Real => match self {
                LReal => Real,
                _ => None?,
            },
            // USINT UINT UDINT ULINT
            USInt => match self {
                UInt => UInt,
                UDInt => UDInt,
                ULInt => ULInt,
                // sint is explicit only
                Int => Int,
                DInt => DInt,
                LInt => LInt,
                Real => Real,
                LReal => LReal,
                _ => None?,
            },
            UInt => match self {
                UDInt => UDInt,
                ULInt => ULInt,
                DInt => DInt,
                LInt => LInt,
                Real => Real,
                LReal => LReal,
                _ => None?,
            },
            UDInt => match self {
                LReal => LReal,
                LInt => LInt,
                ULInt => ULInt,
                _ => None?,
            },
            // TIME LTIME
            Time => match self {
                LTime => LTime,
                _ => None?,
            },
            // DT LDT
            DateAndTime => match self {
                LDateTime => LDateTime,
                _ => None?,
            },
            // DATE LDATE
            Date => match self {
                LDate => LDate,
                _ => None?,
            },
            // TOD LTOD
            Tod => match self {
                LTod => LTod,
                _ => None?,
            },
            // CHAR STRING
            Char => match self {
                String => String,
                _ => None?,
            },
            _ => None?,
        })
    }

    /// Explicit casts according to IEC 61131-3 standard
    ///
    /// See 6.6.1.6 Data type conversion
    pub fn explicit_cast(&self, typ: ElementarySpec) -> bool {
        use ElementarySpec::*;
        match typ {
            LReal => matches!(
                self,
                Real | LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | LWord
            ),
            Real => matches!(
                self,
                LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | DWord
            ),
            LInt => matches!(
                self,
                LReal
                    | Real
                    | DInt
                    | Int
                    | SInt
                    | ULInt
                    | UDInt
                    | UInt
                    | USInt
                    | LWord
                    | DWord
                    | Word
                    | Byte
            ),
            DInt => matches!(
                self,
                Real | Int | SInt | ULInt | UDInt | UInt | USInt | LWord | DWord | Word | Byte
            ),
            Int => matches!(
                self,
                SInt | ULInt | UDInt | UInt | USInt | LWord | DWord | Word | Byte
            ),
            SInt => matches!(
                self,
                ULInt | UDInt | UInt | USInt | LWord | DWord | Word | Byte
            ),
            ULInt => matches!(
                self,
                LReal
                    | Real
                    | LInt
                    | DInt
                    | Int
                    | SInt
                    | UDInt
                    | UInt
                    | USInt
                    | LWord
                    | DWord
                    | Word
                    | Byte
            ),
            UDInt => matches!(
                self,
                Real | DInt | Int | SInt | UInt | USInt | LWord | DWord | Word | Byte
            ),
            UInt => matches!(self, Int | SInt | USInt | LWord | DWord | Word | Byte),
            USInt => matches!(self, SInt | LWord | DWord | Word | Byte),
            LWord => matches!(
                self,
                LReal
                    | LInt
                    | DInt
                    | Int
                    | SInt
                    | ULInt
                    | UDInt
                    | UInt
                    | USInt
                    | DWord
                    | Word
                    | Byte
            ),
            DWord => matches!(
                self,
                Real | LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | Word | Byte
            ),
            Word => matches!(
                self,
                LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | Byte
            ),
            Byte => match self {
                LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt => true,
                Char => true, // char is the size of a byte
                _ => false,
            },
            Bool | REDGEBool | FEDGEBool => matches!(
                self,
                LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt
            ),
            LTime => matches!(self, Time),
            LDateTime => matches!(self, DateAndTime | LDate | Date | LTod | Tod),
            DateAndTime => matches!(self, LDate | Date | LTod | Tod),
            LDate => matches!(self, Date),
            LTod => matches!(self, Tod),
            _ => false,
        }
    }
}
