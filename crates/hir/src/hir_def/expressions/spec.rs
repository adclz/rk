use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::interned::namespace::SpanNamespaceAccess;
use db::WorkspaceDataBase;

use crate::hir_def::expressions::expression::{InitExpr, VariableAccess};
use crate::{AstId, HasName};
use crate::{
    HirNodeInfo,
    hir_def::{
        expressions::expression::{Expr, MultibitsPart},
        interned::identifier::Ident,
        scope::ScopeId,
    },
};

#[salsa::tracked(debug)]
pub struct Spec<'db> {
    #[returns(ref)]
    pub kind: SpecKind<'db>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    pub scope_id: ScopeId<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum SpecKind<'db> {
    // Simple types
    Simple(ElementarySpec),

    // Composite types
    Struct(Struct<'db>),
    Array(Array<'db>),
    ArrayConformand(Spec<'db>),
    Subrange(SubRange<'db>),
    Enum(Enum<'db>),

    // Sized string types (STRING[N], WSTRING[N])
    SizedString(Expr<'db>),
    SizedWString(Expr<'db>),

    // Reference to another spec
    Ref(Spec<'db>),

    // Targeting a POU or namespace (has to be resolved)
    Target(SpanNamespaceAccess<'db>),

    // INTO(ref) — type must be implicitly convertible to the referenced variable's type
    Into(SpanIdent<'db>),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ElementarySpec {
    Bool,
    REDGEBool,
    FEDGEBool,
    Byte,
    Word,
    DWord,
    LWord,
    SInt,
    USInt,
    UInt,
    Int,
    DInt,
    UDInt,
    LInt,
    ULInt,
    Real,
    LReal,
    String,
    WString,
    Char,
    WChar,
    Date,
    LDate,
    DateAndTime,
    LDateTime,
    Time,
    LTime,
    Tod,
    LTod,
    // ANY type hierarchy — polymorphic type specs
    Any,
    AnyNum,
    AnyInt,
    AnyReal,
    AnyBit,
    AnyElementary,
    AnyMagnitude,
    AnyChars,
    AnyChar,
    AnyString,
    AnyDate,
    AnyDuration,
    AnySigned,
    AnyUnsigned,
}

impl ElementarySpec {
    pub fn is_simple(&self) -> bool {
        match self {
            ElementarySpec::Bool
            | ElementarySpec::REDGEBool
            | ElementarySpec::FEDGEBool
            | ElementarySpec::Byte
            | ElementarySpec::Word
            | ElementarySpec::DWord
            | ElementarySpec::LWord
            | ElementarySpec::SInt
            | ElementarySpec::USInt
            | ElementarySpec::UInt
            | ElementarySpec::Int
            | ElementarySpec::DInt
            | ElementarySpec::UDInt
            | ElementarySpec::LInt
            | ElementarySpec::ULInt
            | ElementarySpec::Real
            | ElementarySpec::LReal
            | ElementarySpec::String
            | ElementarySpec::WString
            | ElementarySpec::Char
            | ElementarySpec::WChar
            | ElementarySpec::Date
            | ElementarySpec::LDate
            | ElementarySpec::DateAndTime
            | ElementarySpec::LDateTime
            | ElementarySpec::Time
            | ElementarySpec::LTime
            | ElementarySpec::Tod
            | ElementarySpec::LTod => true,
            // ANY types are polymorphic, not simple concrete types
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
            | ElementarySpec::AnyUnsigned => false,
        }
    }

    pub fn is_complex(&self) -> bool {
        !self.is_simple()
    }

    /// Enumerate every concrete elementary type accepted by this `ANY_*`
    /// spec. Returns an empty vec for non-ANY_* receivers. Used by the
    /// preprocess exhaustiveness check (E0325) to compute which variants
    /// a `{#if}` chain must cover.
    pub fn concrete_variants(&self) -> Vec<ElementarySpec> {
        if !self.is_any() {
            return Vec::new();
        }
        const ALL: &[ElementarySpec] = &[
            ElementarySpec::Bool,
            ElementarySpec::Byte,
            ElementarySpec::Word,
            ElementarySpec::DWord,
            ElementarySpec::LWord,
            ElementarySpec::SInt,
            ElementarySpec::USInt,
            ElementarySpec::Int,
            ElementarySpec::UInt,
            ElementarySpec::DInt,
            ElementarySpec::UDInt,
            ElementarySpec::LInt,
            ElementarySpec::ULInt,
            ElementarySpec::Real,
            ElementarySpec::LReal,
            ElementarySpec::String,
            ElementarySpec::WString,
            ElementarySpec::Char,
            ElementarySpec::WChar,
            ElementarySpec::Date,
            ElementarySpec::LDate,
            ElementarySpec::DateAndTime,
            ElementarySpec::LDateTime,
            ElementarySpec::Time,
            ElementarySpec::LTime,
            ElementarySpec::Tod,
            ElementarySpec::LTod,
        ];
        ALL.iter().copied().filter(|c| self.accepts(*c)).collect()
    }

    /// Returns true if this is an ANY_* polymorphic type spec.
    pub fn is_any(&self) -> bool {
        matches!(
            self,
            Self::Any
                | Self::AnyNum
                | Self::AnyInt
                | Self::AnyReal
                | Self::AnyBit
                | Self::AnyElementary
                | Self::AnyMagnitude
                | Self::AnyChars
                | Self::AnyChar
                | Self::AnyString
                | Self::AnyDate
                | Self::AnyDuration
                | Self::AnySigned
                | Self::AnyUnsigned
        )
    }

    /// Checks if a concrete ElementarySpec is accepted by this ANY_* spec.
    /// Returns false if `self` is not an ANY_* variant.
    pub fn accepts(&self, concrete: ElementarySpec) -> bool {
        match self {
            Self::Any => true,
            Self::AnyElementary => concrete.is_simple(),
            Self::AnyMagnitude => {
                Self::AnyNum.accepts(concrete) || Self::AnyDuration.accepts(concrete)
            }
            Self::AnyNum => Self::AnyReal.accepts(concrete) || Self::AnyInt.accepts(concrete),
            Self::AnyInt => matches!(
                concrete,
                Self::SInt
                    | Self::Int
                    | Self::DInt
                    | Self::LInt
                    | Self::USInt
                    | Self::UInt
                    | Self::UDInt
                    | Self::ULInt
            ),
            Self::AnyReal => matches!(concrete, Self::Real | Self::LReal),
            Self::AnyBit => matches!(
                concrete,
                Self::Bool | Self::Byte | Self::Word | Self::DWord | Self::LWord
            ),
            Self::AnyChars => Self::AnyString.accepts(concrete) || Self::AnyChar.accepts(concrete),
            Self::AnyString => matches!(concrete, Self::String | Self::WString),
            Self::AnyChar => matches!(concrete, Self::Char | Self::WChar),
            Self::AnyDate => matches!(
                concrete,
                Self::Date
                    | Self::LDate
                    | Self::DateAndTime
                    | Self::LDateTime
                    | Self::Tod
                    | Self::LTod
            ),
            Self::AnyDuration => matches!(concrete, Self::Time | Self::LTime),
            Self::AnySigned => matches!(concrete, Self::SInt | Self::Int | Self::DInt | Self::LInt),
            Self::AnyUnsigned => matches!(
                concrete,
                Self::USInt | Self::UInt | Self::UDInt | Self::ULInt
            ),
            _ => false,
        }
    }
}

impl<'db> HirNodeInfo<'db> for Spec<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

#[salsa::tracked(debug)]
pub struct Struct<'db> {
    pub overlap: bool,
    pub elements: Vec<StructElement<'db>>,
}

#[salsa::tracked(debug)]
pub struct StructElement<'db> {
    pub name: Ident,

    #[tracked]
    #[no_eq]
    pub name_id: AstId,

    pub located: Option<VariableAccess<'db>>,
    pub multibits: Option<MultibitsPart>,
    pub spec: Spec<'db>,
    pub init: Option<InitExpr<'db>>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for StructElement<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

impl<'db> HasName<'db> for StructElement<'db> {
    fn get_name_ident(&self, db: &'db dyn WorkspaceDataBase) -> Ident {
        self.name(db)
    }

    fn get_name_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.name_id(db)
    }
}

#[salsa::tracked(debug)]
pub struct Enum<'db> {
    pub typ: Option<Spec<'db>>,
    pub variants: Vec<EnumVariant<'db>>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct EnumVariant<'db> {
    pub name: SpanIdent<'db>,
    pub value: Option<Expr<'db>>,
}

#[salsa::tracked(debug)]
pub struct Array<'db> {
    // lower - upper bounds
    pub subranges: Vec<(Expr<'db>, Expr<'db>)>,
    pub of_type: Spec<'db>,
}

#[salsa::tracked(debug)]
pub struct SubRange<'db> {
    // Should be a INT
    pub _type: Spec<'db>,
    pub lower: Expr<'db>,
    pub upper: Expr<'db>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that every `ANY_*` variant returns the exact set of concrete
    /// elementary types it accepts. Locks the exhaustiveness check's expected
    /// set against accidental drift (e.g. someone adds a new concrete type
    /// to the language but forgets to update `concrete_variants`).
    #[test]
    fn concrete_variants_for_each_any() {
        use ElementarySpec::*;
        let cases: &[(ElementarySpec, &[ElementarySpec])] = &[
            (
                AnyInt,
                &[SInt, USInt, Int, UInt, DInt, UDInt, LInt, ULInt],
            ),
            (AnyReal, &[Real, LReal]),
            (AnyBit, &[Bool, Byte, Word, DWord, LWord]),
            (AnySigned, &[SInt, Int, DInt, LInt]),
            (AnyUnsigned, &[USInt, UInt, UDInt, ULInt]),
            (
                AnyNum,
                &[
                    SInt, USInt, Int, UInt, DInt, UDInt, LInt, ULInt, Real, LReal,
                ],
            ),
            (
                AnyMagnitude,
                &[
                    SInt, USInt, Int, UInt, DInt, UDInt, LInt, ULInt, Real, LReal, Time, LTime,
                ],
            ),
            (AnyDuration, &[Time, LTime]),
            (
                AnyDate,
                &[Date, LDate, DateAndTime, LDateTime, Tod, LTod],
            ),
            (AnyChar, &[Char, WChar]),
            (AnyString, &[String, WString]),
            (AnyChars, &[String, WString, Char, WChar]),
        ];

        for (any, expected) in cases {
            let got = any.concrete_variants();
            // Sort both sides on byte-pattern of the discriminants for
            // order-independent equality. Vec equality would care about
            // ordering; here we only care that the *sets* match.
            let mut got_sorted = got.clone();
            got_sorted.sort_by_key(|e| format!("{:?}", e));
            let mut expected_sorted: Vec<ElementarySpec> = expected.to_vec();
            expected_sorted.sort_by_key(|e| format!("{:?}", e));
            assert_eq!(
                got_sorted, expected_sorted,
                "{any:?}: got {got:?}, expected {expected:?}",
            );
        }
    }

    /// `concrete_variants` on a non-ANY_* spec must return empty — concrete
    /// types don't enumerate themselves.
    #[test]
    fn concrete_variants_empty_for_concrete() {
        use ElementarySpec::*;
        for c in [Int, Real, Bool, Byte, Time, String, Char] {
            assert!(
                c.concrete_variants().is_empty(),
                "concrete spec {c:?} should yield empty variant set",
            );
        }
    }

    /// `Any` is the universal supertype; `AnyElementary` covers everything
    /// `is_simple` does. Both should be non-empty and contain a representative
    /// of each major category.
    #[test]
    fn concrete_variants_universal_specs_are_non_empty() {
        use ElementarySpec::*;
        let any = Any.concrete_variants();
        let any_elem = AnyElementary.concrete_variants();
        for representative in [Int, Real, Bool, Time, Date, String] {
            assert!(any.contains(&representative), "Any missing {representative:?}");
            assert!(
                any_elem.contains(&representative),
                "AnyElementary missing {representative:?}"
            );
        }
    }
}
