use crate::{
    hir_def::{expressions::spec::ElementarySpec, pous::generics::AnyGeneric},
    hir_ty::ty::Type,
};

impl<'db> AnyGeneric {
    pub fn implicit_cast_with_spec(&self, spec: ElementarySpec) -> Option<ElementarySpec> {
        use AnyGeneric::*;

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
            _ => None?,
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
            _ => None?,
        })
    }

    /// Explicit casts according to IEC 61131-3 standard
    ///
    /// See 6.6.1.6 Data type conversion
    pub fn explicit_cast(&self, typ: ElementarySpec) -> Option<ExplicitCast<'db>> {
        use ElementarySpec::*;
        match typ {
            LReal => match self {
                Real | LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | LWord => {
                    Some(ExplicitCast {
                        from: Type::Elementary(*self),
                        to: Type::Elementary(LReal),
                    })
                }
                _ => None,
            },
            Real => match self {
                LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | DWord => {
                    Some(ExplicitCast {
                        from: Type::Elementary(*self),
                        to: Type::Elementary(Real),
                    })
                }
                _ => None,
            },
            LInt => match self {
                LReal | Real | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | LWord | DWord
                | Word | Byte => Some(ExplicitCast {
                    from: Type::Elementary(*self),
                    to: Type::Elementary(LInt),
                }),
                _ => None,
            },
            DInt => match self {
                Real | Int | SInt | ULInt | UDInt | UInt | USInt | LWord | DWord | Word | Byte => {
                    Some(ExplicitCast {
                        from: Type::Elementary(*self),
                        to: Type::Elementary(DInt),
                    })
                }
                _ => None,
            },
            Int => match self {
                SInt | ULInt | UDInt | UInt | USInt | LWord | DWord | Word | Byte => {
                    Some(ExplicitCast {
                        from: Type::Elementary(*self),
                        to: Type::Elementary(Int),
                    })
                }
                _ => None,
            },
            SInt => match self {
                ULInt | UDInt | UInt | USInt | LWord | DWord | Word | Byte => Some(ExplicitCast {
                    from: Type::Elementary(*self),
                    to: Type::Elementary(SInt),
                }),
                _ => None,
            },
            ULInt => match self {
                LReal | Real | LInt | DInt | Int | SInt | UDInt | UInt | USInt | LWord | DWord
                | Word | Byte => Some(ExplicitCast {
                    from: Type::Elementary(*self),
                    to: Type::Elementary(ULInt),
                }),
                _ => None,
            },
            UDInt => match self {
                Real | DInt | Int | SInt | UInt | USInt | LWord | DWord | Word | Byte => {
                    Some(ExplicitCast {
                        from: Type::Elementary(*self),
                        to: Type::Elementary(UDInt),
                    })
                }
                _ => None,
            },
            UInt => match self {
                Int | SInt | USInt | LWord | DWord | Word | Byte => Some(ExplicitCast {
                    from: Type::Elementary(*self),
                    to: Type::Elementary(UInt),
                }),
                _ => None,
            },
            USInt => match self {
                SInt | LWord | DWord | Word | Byte => Some(ExplicitCast {
                    from: Type::Elementary(*self),
                    to: Type::Elementary(USInt),
                }),
                _ => None,
            },
            LWord => match self {
                LReal | LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | DWord | Word
                | Byte => Some(ExplicitCast {
                    from: Type::Elementary(*self),
                    to: Type::Elementary(LWord),
                }),
                _ => None,
            },
            DWord => match self {
                Real | LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | Word | Byte => {
                    Some(ExplicitCast {
                        from: Type::Elementary(*self),
                        to: Type::Elementary(DWord),
                    })
                }
                _ => None,
            },
            Word => match self {
                LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt | Byte => {
                    Some(ExplicitCast {
                        from: Type::Elementary(*self),
                        to: Type::Elementary(Word),
                    })
                }
                _ => None,
            },
            Byte => match self {
                LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt => Some(ExplicitCast {
                    from: Type::Elementary(*self),
                    to: Type::Elementary(Byte),
                }),
                _ => None,
            },
            Bool | REDGEBool | FEDGEBool => match self {
                LInt | DInt | Int | SInt | ULInt | UDInt | UInt | USInt => Some(ExplicitCast {
                    from: Type::Elementary(*self),
                    to: Type::Elementary(*self),
                }),
                _ => None,
            },
            _ => None,
        }
    }
}
