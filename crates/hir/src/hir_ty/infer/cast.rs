use crate::{
    hir_def::{expressions::spec::ElementarySpec, pous::generics::AnyGeneric},
    hir_ty::ty::Type,
};

impl<'db> AnyGeneric {
    /// Check if an elementary type can be used where a generic of this ANY_* group is expected.
    /// Returns the spec unchanged if valid, None if not.
    pub fn implicit_cast_with_spec(&self, spec: ElementarySpec) -> Option<ElementarySpec> {
        use AnyGeneric::*;

        // Parent groups delegate to their children
        match self {
            ANY => return Some(spec),
            ANY_MAGNITUDE => {
                return ANY_NUM
                    .implicit_cast_with_spec(spec)
                    .or_else(|| ANY_DURATION.implicit_cast_with_spec(spec));
            }
            ANY_NUM => {
                return ANY_REAL
                    .implicit_cast_with_spec(spec)
                    .or_else(|| ANY_INT.implicit_cast_with_spec(spec));
            }
            ANY_CHARS => {
                return ANY_STRING
                    .implicit_cast_with_spec(spec)
                    .or_else(|| ANY_CHAR.implicit_cast_with_spec(spec));
            }
            _ => {}
        }

        Some(match spec {
            ElementarySpec::SInt => match self {
                ANY_INT | ANY_SIGNED | ANY_REAL => ElementarySpec::SInt,
                _ => None?,
            },
            ElementarySpec::Int => match self {
                ANY_INT | ANY_SIGNED | ANY_REAL => ElementarySpec::Int,
                _ => None?,
            },
            ElementarySpec::DInt => match self {
                ANY_INT | ANY_SIGNED | ANY_REAL => ElementarySpec::DInt,
                _ => None?,
            },
            ElementarySpec::LInt => match self {
                ANY_INT | ANY_SIGNED | ANY_REAL => ElementarySpec::LInt,
                _ => None?,
            },
            ElementarySpec::USInt => match self {
                ANY_UNSIGNED | ANY_REAL => ElementarySpec::USInt,
                _ => None?,
            },
            ElementarySpec::UInt => match self {
                ANY_UNSIGNED | ANY_REAL => ElementarySpec::UInt,
                _ => None?,
            },
            ElementarySpec::UDInt => match self {
                ANY_UNSIGNED | ANY_REAL => ElementarySpec::UDInt,
                _ => None?,
            },
            ElementarySpec::ULInt => match self {
                ANY_UNSIGNED | ANY_REAL => ElementarySpec::ULInt,
                _ => None?,
            },
            ElementarySpec::Real => match self {
                ANY_REAL => ElementarySpec::Real,
                _ => None?,
            },
            ElementarySpec::LReal => match self {
                ANY_REAL => ElementarySpec::LReal,
                _ => None?,
            },
            ElementarySpec::Bool | ElementarySpec::REDGEBool | ElementarySpec::FEDGEBool => {
                match self {
                    ANY_BIT => spec,
                    _ => None?,
                }
            }
            ElementarySpec::Byte => match self {
                ANY_BIT => ElementarySpec::Byte,
                _ => None?,
            },
            ElementarySpec::Word => match self {
                ANY_BIT => ElementarySpec::Word,
                _ => None?,
            },
            ElementarySpec::DWord => match self {
                ANY_BIT => ElementarySpec::DWord,
                _ => None?,
            },
            ElementarySpec::LWord => match self {
                ANY_BIT => ElementarySpec::LWord,
                _ => None?,
            },
            ElementarySpec::String => match self {
                ANY_STRING => ElementarySpec::String,
                _ => None?,
            },
            ElementarySpec::WString => match self {
                ANY_STRING => ElementarySpec::WString,
                _ => None?,
            },
            ElementarySpec::Char => match self {
                ANY_CHAR => ElementarySpec::Char,
                _ => None?,
            },
            ElementarySpec::WChar => match self {
                ANY_CHAR => ElementarySpec::WChar,
                _ => None?,
            },
            ElementarySpec::Date | ElementarySpec::LDate => match self {
                ANY_DATE => spec,
                _ => None?,
            },
            ElementarySpec::DateAndTime | ElementarySpec::LDateTime => match self {
                ANY_DATE => spec,
                _ => None?,
            },
            ElementarySpec::Tod | ElementarySpec::LTod => match self {
                ANY_DATE => spec,
                _ => None?,
            },
            ElementarySpec::Time | ElementarySpec::LTime => match self {
                ANY_DURATION => spec,
                _ => None?,
            },
            // ANY type specs accept any value — no casting needed
            ElementarySpec::Any
            | ElementarySpec::AnyNum
            | ElementarySpec::AnyInt
            | ElementarySpec::AnyReal
            | ElementarySpec::AnyBit
            | ElementarySpec::AnyElementary
            | ElementarySpec::AnyMagnitude
            | ElementarySpec::AnyChars
            | ElementarySpec::AnyChar
            | ElementarySpec::AnyString
            | ElementarySpec::AnyDate
            | ElementarySpec::AnyDuration
            | ElementarySpec::AnySigned
            | ElementarySpec::AnyUnsigned => spec,
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ExplicitCast<'db> {
    from: Type<'db>,
    to: Type<'db>,
}

impl<'db> ElementarySpec {
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
            // WCHAR WSTRING
            WChar => match self {
                WString => WString,
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
            LReal => match self {
                Real | LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | LWord => true,
                _ => false,
            },
            Real => match self {
                LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | DWord => true,
                _ => false,
            },
            LInt => match self {
                LReal | Real | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | LWord | DWord
                | Word | Byte => true,
                _ => false,
            },
            DInt => match self {
                Real | Int | SInt | ULInt | UDInt | UInt | USInt | LWord | DWord | Word | Byte => {
                    true
                }
                _ => false,
            },
            Int => match self {
                SInt | ULInt | UDInt | UInt | USInt | LWord | DWord | Word | Byte => true,
                _ => false,
            },
            SInt => match self {
                ULInt | UDInt | UInt | USInt | LWord | DWord | Word | Byte => true,
                _ => false,
            },
            ULInt => match self {
                LReal | Real | LInt | DInt | Int | SInt | UDInt | UInt | USInt | LWord | DWord
                | Word | Byte => true,
                _ => false,
            },
            UDInt => match self {
                Real | DInt | Int | SInt | UInt | USInt | LWord | DWord | Word | Byte => true,
                _ => false,
            },
            UInt => match self {
                Int | SInt | USInt | LWord | DWord | Word | Byte => true,
                _ => false,
            },
            USInt => match self {
                SInt | LWord | DWord | Word | Byte => true,
                _ => false,
            },
            LWord => match self {
                LReal | LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | DWord | Word
                | Byte => true,
                _ => false,
            },
            DWord => match self {
                Real | LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | Word | Byte => {
                    true
                }
                _ => false,
            },
            Word => match self {
                LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | Byte => true,
                _ => false,
            },
            Byte => match self {
                LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt => true,
                _ => false,
            },
            Bool | REDGEBool | FEDGEBool => match self {
                LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt => true,
                _ => false,
            },
            _ => false,
        }
    }
}
