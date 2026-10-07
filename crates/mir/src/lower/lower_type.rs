// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::WorkspaceDataBase;
use hir::{
    hir_def::{
        expressions::spec::{Array, ElementarySpec, Struct, SubRange},
        interned::identifier::Ident,
        pous::{class::Class, function_block::FunctionBlock},
    },
    hir_ty::{infer::Infer, ty::Type},
};

use compact_str::CompactString;
use std::cell::RefCell;

use crate::types::{
    MirArrayType, MirElementary, MirEnumType, MirStructField, MirStructType, MirSubrangeType,
    MirType,
};

#[derive(Debug, thiserror::Error)]
pub enum LowerTypeError {
    #[error("Unsupported type: {0}")]
    UnsupportedType(String),

    #[error("Type could not be resolved (Type::Never encountered)")]
    UnresolvedType,

    /// Two functions lowered to one symbol, so calls to one would reach the
    /// other. `naming` is meant to make this impossible.
    #[error("two functions lower to the symbol `{0}`")]
    DuplicateSymbol(String),

    /// A function the emitted calls reach again, with no frame: HIR's call
    /// graph missed one of the calls, and the function's storage would be
    /// shared between them.
    #[error("`{0}` can call itself again but has no frame")]
    UnframedRecursion(String),

    /// An instance's layout reached back to itself. Only a VAR_IN_OUT
    /// member closes such a cycle, a by-value one being E1302, and the
    /// nearest one catches this: it holds an address whose pointee is typed
    /// where it is dereferenced.
    #[error("the layout of '{0}' contains itself")]
    InstanceCycle(String),

    /// An error carrying the source location it originated at, attached by
    /// [`LowerTypeError::with_location`].
    #[error("{inner}")]
    Located {
        inner: Box<LowerTypeError>,
        file: auto_lsp::default::db::file::File,
        span: auto_lsp::tree_sitter::Range,
    },
}

impl LowerTypeError {
    /// Attach a source location, keeping the innermost one already present.
    pub fn with_location(
        self,
        file: auto_lsp::default::db::file::File,
        span: auto_lsp::tree_sitter::Range,
    ) -> Self {
        match self {
            // already located by a deeper frame — keep the precise one
            located @ LowerTypeError::Located { .. } => located,
            inner => LowerTypeError::Located {
                inner: Box::new(inner),
                file,
                span,
            },
        }
    }

    /// The source location, if one was attached.
    pub fn location(
        &self,
    ) -> Option<(
        auto_lsp::default::db::file::File,
        auto_lsp::tree_sitter::Range,
    )> {
        match self {
            LowerTypeError::Located { file, span, .. } => Some((*file, *span)),
            _ => None,
        }
    }
}

/// Convert an inferred HIR `Type` to a `MirType`. A `STRING` lowers at
/// the default capacity, since an inferred type carries no declared
/// length; use [`lower_spec`] when a spec is available.
pub fn lower_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: Type<'db>,
) -> Result<MirType, LowerTypeError> {
    // Capture a wrapping `TYPE Pt : STRUCT` name before normalize peels it,
    // so the struct carries "Pt" rather than "<anon_struct>". Display-only.
    let type_name = match ty {
        Type::DataType(dt) => Some(dt.name_with_case(db)),
        _ => None,
    };
    // `normalize` resolves a subrange to its base, and the debug type table
    // records the bounds: resolve the subrange first.
    if let Some(sr) = ty.as_subrange(db) {
        return lower_subrange_type(db, sr);
    }
    let ty = ty.normalize(db);

    match ty {
        Type::Elementary(ElementarySpec::String) => Ok(MirType::String {
            capacity: crate::types::DEFAULT_STRING_CAPACITY,
        }),

        Type::Elementary(spec) => {
            let mir_elem = elementary_spec_to_mir(spec)?;
            Ok(MirType::Elementary(mir_elem))
        }

        // An opaque address: what it points at is typed where it is
        // dereferenced, by HIR's deref adjustment (`pointee_of`). Typing it
        // here lowers a type that references itself (`next : REF_TO Node`)
        // without end, and a pointer to a struct or an array would read as a
        // by-reference parameter, which a REF_TO is not.
        Type::RefTo(_) => Ok(MirType::Pointer(Box::new(MirType::Void))),

        Type::Struct(s) => lower_struct_type_named(db, s, type_name),

        Type::Array(arr) => lower_array_type(db, arr),

        Type::Enum(e) => lower_enum_type_named(db, e, type_name),

        Type::SubRange(sr) => lower_subrange_type(db, sr),

        Type::FunctionBlock(fb) => lower_fb_type(db, fb),

        Type::Class(class) => lower_class_type(db, class),

        Type::Void => Ok(MirType::Void),

        Type::Never => Err(LowerTypeError::UnresolvedType),

        _ => Err(LowerTypeError::UnsupportedType(format!("{:?}", ty))),
    }
}

/// Convert an `ElementarySpec` to `MirElementary`; an error for ANY_*.
pub fn elementary_spec_to_mir(spec: ElementarySpec) -> Result<MirElementary, LowerTypeError> {
    Ok(match spec {
        ElementarySpec::Bool => MirElementary::Bool,
        ElementarySpec::SInt => MirElementary::SInt,
        ElementarySpec::Int => MirElementary::Int,
        ElementarySpec::DInt => MirElementary::DInt,
        ElementarySpec::LInt => MirElementary::LInt,
        ElementarySpec::USInt => MirElementary::USInt,
        ElementarySpec::UInt => MirElementary::UInt,
        ElementarySpec::UDInt => MirElementary::UDInt,
        ElementarySpec::ULInt => MirElementary::ULInt,
        ElementarySpec::Byte => MirElementary::Byte,
        ElementarySpec::Word => MirElementary::Word,
        ElementarySpec::DWord => MirElementary::DWord,
        ElementarySpec::LWord => MirElementary::LWord,
        ElementarySpec::Real => MirElementary::Real,
        ElementarySpec::LReal => MirElementary::LReal,
        ElementarySpec::Char => MirElementary::Char,
        ElementarySpec::Time => MirElementary::Time,
        ElementarySpec::LTime => MirElementary::LTime,
        ElementarySpec::Date => MirElementary::Date,
        ElementarySpec::LDate => MirElementary::LDate,
        ElementarySpec::Tod => MirElementary::Tod,
        ElementarySpec::LTod => MirElementary::LTod,
        ElementarySpec::DateAndTime => MirElementary::DateAndTime,
        ElementarySpec::LDateTime => MirElementary::LDateTime,
        // STRING is memory-resident: a STRING used where a scalar is expected
        // (comparison routes to `str.byte_cmp` before asking).
        ElementarySpec::String => {
            return Err(LowerTypeError::UnsupportedType(
                "STRING has no scalar MIR representation (used where a scalar \
                 elementary is expected)"
                    .to_string(),
            ));
        }
    })
}

/// Lower a DECLARATION to its MIR type: a variable, a struct element, an
/// array's element type, a global. Prefer it over [`lower_type`], which
/// takes an inferred `Type` and cannot know a declared `STRING[n]` length:
/// `Type::normalize` collapses `STRING[n]` and plain `STRING`, since the
/// length is a layout fact, not part of type identity.
pub(crate) fn lower_spec<'db>(
    db: &'db dyn WorkspaceDataBase,
    spec: hir::hir_def::expressions::spec::Spec<'db>,
) -> Result<MirType, LowerTypeError> {
    Ok(apply_sized_string(
        db,
        spec,
        lower_type(db, spec.infer(db))?,
    ))
}

fn apply_sized_string<'db>(
    db: &'db dyn WorkspaceDataBase,
    spec: hir::hir_def::expressions::spec::Spec<'db>,
    mir: MirType,
) -> MirType {
    if !matches!(mir, MirType::String { .. }) {
        return mir;
    }
    match hir::hir_ty::infer::normalize::declared_string_capacity(db, spec) {
        Some(capacity) => MirType::String { capacity },
        None => mir,
    }
}

/// Lower a struct type, optionally with an explicit name
/// (used when the name comes from a wrapping DataType or variable).
pub fn lower_struct_type_named<'db>(
    db: &'db dyn WorkspaceDataBase,
    struct_type: Struct<'db>,
    name: Option<Ident>,
) -> Result<MirType, LowerTypeError> {
    let layout = hir::hir_ty::layout::struct_layout(db, struct_type)
        .as_ref()
        .ok_or_else(|| no_layout("a STRUCT"))?;
    let mut fields = Vec::new();
    for (element, offset) in struct_type.elements(db).iter().zip(&layout.offsets) {
        fields.push(MirStructField {
            name_with_case: element.name_with_case(db),
            ty: lower_spec(db, element.spec(db))?,
            offset: narrow(*offset)?,
            by_ref: false,
        });
    }

    // Use provided name or create an anonymous one
    let struct_name = name.unwrap_or_else(|| Ident::new(db, CompactString::from("<anon_struct>")));

    Ok(MirType::Struct(MirStructType {
        name: struct_name,
        fields,
        size: narrow(layout.whole.size)?,
        align: layout.whole.align,
    }))
}

/// A size or an offset HIR laid out, in the 32 bits a module addresses:
/// one past them is E0322, which stops a build before lowering.
pub(crate) fn narrow(bytes: u64) -> Result<u32, LowerTypeError> {
    u32::try_from(bytes).map_err(|_| {
        LowerTypeError::UnsupportedType(format!(
            "{bytes} bytes, past what a module addresses, reached lowering"
        ))
    })
}

/// HIR has no layout for it: a type that does not resolve, or contains
/// itself, which the check reported.
fn no_layout(what: &str) -> LowerTypeError {
    LowerTypeError::UnsupportedType(format!("{what} with no layout reached lowering"))
}

fn lower_array_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    array_type: Array<'db>,
) -> Result<MirType, LowerTypeError> {
    let layout = hir::hir_ty::layout::array_layout(db, array_type)
        .as_ref()
        .ok_or_else(|| no_layout("an array"))?;
    Ok(MirType::Array(MirArrayType {
        element_type: Box::new(lower_spec(db, array_type.of_type(db))?),
        dimensions: layout.dimensions.clone(),
        total_elements: narrow(layout.count)?,
        element_size: narrow(layout.element.size)?,
        size: narrow(layout.whole.size)?,
        align: layout.element.align,
    }))
}

/// Each enumerator paired with its declared numeric value: an explicit
/// `(Idle := 10, Run := 20)`, or continuing from the previous value from 0.
/// The single source of enum numbering for both the type and its literals.
pub fn enum_variant_values<'db>(
    db: &'db dyn WorkspaceDataBase,
    enum_type: hir::hir_def::expressions::spec::Enum<'db>,
) -> Result<Vec<(hir::hir_def::expressions::spec::EnumVariant<'db>, i64)>, LowerTypeError> {
    // The ordinals inference evaluated; one that does not fold is E0604.
    hir::hir_ty::infer::const_eval::enum_ordinals(db, enum_type)
        .into_iter()
        .map(|(variant, value)| {
            value.map(|v| (variant, v)).ok_or_else(|| {
                LowerTypeError::UnsupportedType(
                    "enum variant value was not folded to a constant".to_string(),
                )
            })
        })
        .collect()
}

/// The storage lane of an enum: its declared base type, DInt by default.
/// Every consumer derives the lane from this.
pub fn enum_storage<'db>(
    db: &'db dyn WorkspaceDataBase,
    enum_type: hir::hir_def::expressions::spec::Enum<'db>,
) -> Result<MirElementary, LowerTypeError> {
    let Some(spec) = enum_type.typ(db) else {
        return Ok(MirElementary::DInt);
    };
    match spec.infer(db).normalize(db) {
        Type::Elementary(es) => elementary_spec_to_mir(es),
        other => Err(LowerTypeError::UnsupportedType(format!(
            "enum base must be an elementary integer type, got {other:?}"
        ))),
    }
}

pub fn lower_enum_type_named<'db>(
    db: &'db dyn WorkspaceDataBase,
    enum_type: hir::hir_def::expressions::spec::Enum<'db>,
    name: Option<Ident>,
) -> Result<MirType, LowerTypeError> {
    let mut variants = Vec::new();
    for (variant, value) in enum_variant_values(db, enum_type)? {
        variants.push((variant.name.with_case.text(db).clone(), value));
    }

    let enum_name = name.unwrap_or_else(|| Ident::new(db, CompactString::from("<anon_enum>")));

    Ok(MirType::Enum(MirEnumType {
        name: enum_name,
        variants,
        storage: enum_storage(db, enum_type)?,
    }))
}

fn lower_subrange_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    subrange: SubRange<'db>,
) -> Result<MirType, LowerTypeError> {
    let base_type = subrange._type(db).infer(db);
    let base = match base_type.normalize(db) {
        Type::Elementary(spec) => elementary_spec_to_mir(spec)?,
        _ => {
            return Err(LowerTypeError::UnsupportedType(
                "Subrange base must be elementary".to_string(),
            ));
        }
    };

    // The bounds inference folded; a bound that does not fold is E0703 at
    // the declaration.
    let (Some(lower), Some(upper)) = hir::hir_ty::infer::const_eval::subrange_bounds(db, subrange)
    else {
        return Err(LowerTypeError::UnsupportedType(
            "subrange bound was not folded to a constant".to_string(),
        ));
    };

    Ok(MirType::Subrange(MirSubrangeType { base, lower, upper }))
}

/// Lower a FunctionBlock to a `MirType::Struct` named with its
/// namespace-qualified identifier, which nested-FB instance fields
/// resolve against.
/// The hidden member holding the capacity of the buffer an FB's STRING
/// VAR_IN_OUT `name` is bound to, laid out right after its pointer. `$`
/// cannot appear in an identifier, so no member of the source collides.
pub(crate) fn string_capacity_field(db: &dyn WorkspaceDataBase, name: Ident) -> Ident {
    Ident::new(
        db,
        compact_str::CompactString::from(format!("{}$cap", name.text(db))),
    )
}

/// The member holding an edge input's edge, which the block's own code reads
/// under the input's name.
pub(crate) fn edge_field(db: &dyn WorkspaceDataBase, name: Ident) -> Ident {
    Ident::new(db, CompactString::from(format!("{}$edge", name.text(db))))
}

/// The member holding what an edge input's edge is computed against at the
/// next call, as `R_TRIG` and `F_TRIG` keep their `M`.
pub(crate) fn edge_memory_field(db: &dyn WorkspaceDataBase, name: Ident) -> Ident {
    Ident::new(db, CompactString::from(format!("{}$m", name.text(db))))
}

pub fn lower_fb_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    fb: FunctionBlock<'db>,
) -> Result<MirType, LowerTypeError> {
    lower_instance_struct(
        db,
        hir::hir_def::pous::pou::Pou::FunctionBlock(fb),
        super::naming::qualified_pou_ident(db, Type::FunctionBlock(fb)),
    )
}

thread_local! {
    /// The instances whose layout is being built, innermost last.
    static LAYING_OUT: RefCell<Vec<Ident>> = const { RefCell::new(Vec::new()) };
}

/// An FB/CLASS instance as HIR laid it out ([`instance_layout`]): its
/// bases' members first, so a derived instance is laid out as its base where
/// they overlap.
///
/// [`instance_layout`]: hir::hir_ty::layout::instance_layout
fn lower_instance_struct<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: hir::hir_def::pous::pou::Pou<'db>,
    name: Ident,
) -> Result<MirType, LowerTypeError> {
    // Two FBs holding each other through VAR_IN_OUT members come back here
    // while the first is still being laid out.
    struct Leave;
    impl Drop for Leave {
        fn drop(&mut self) {
            LAYING_OUT.with(|stack| stack.borrow_mut().pop());
        }
    }
    if LAYING_OUT.with(|stack| stack.borrow().contains(&name)) {
        return Err(LowerTypeError::InstanceCycle(name.text(db).to_string()));
    }
    LAYING_OUT.with(|stack| stack.borrow_mut().push(name));
    let _leave = Leave;

    let layout = hir::hir_ty::layout::instance_layout(db, pou)
        .as_ref()
        .ok_or_else(|| no_layout("an instance"))?;
    instance_struct(db, name, layout)
}

/// The struct of an instance HIR laid out: each part named and typed as the
/// code reads it.
fn instance_struct<'db>(
    db: &'db dyn WorkspaceDataBase,
    name: Ident,
    layout: &hir::hir_ty::layout::InstanceLayout<'db>,
) -> Result<MirType, LowerTypeError> {
    use hir::hir_ty::layout::Part;
    let mut fields = Vec::new();
    for field in &layout.fields {
        let var = field.var;
        let var_name = var.name_with_case(db);
        let (name_with_case, ty, by_ref) = match field.part {
            Part::Value => (var_name, lower_spec(db, var.spec(db))?, false),
            // The address of the caller's l-value, which the body auto-derefs
            // and the call site writes once, or of a channel `__init` writes
            // from VAR_CONFIG. An `ARRAY[*]` points at the array each copy of
            // the body is specialized for, and a block two VAR_IN_OUTs hold
            // through each other at what is typed where it is dereferenced.
            Part::Address => {
                let pointee = match var.conformand(db) {
                    Some(_) => MirType::Void,
                    None => match lower_spec(db, var.spec(db)) {
                        Err(LowerTypeError::InstanceCycle(_)) => MirType::Void,
                        other => other?,
                    },
                };
                (var_name, MirType::Pointer(Box::new(pointee)), true)
            }
            Part::Capacity => (
                string_capacity_field(db, var_name),
                MirType::Elementary(MirElementary::UDInt),
                false,
            ),
            Part::Edge => (
                edge_field(db, var_name),
                MirType::Elementary(MirElementary::Bool),
                false,
            ),
            Part::EdgeMemory => (
                edge_memory_field(db, var_name),
                MirType::Elementary(MirElementary::Bool),
                false,
            ),
        };
        fields.push(MirStructField {
            name_with_case,
            ty,
            offset: narrow(field.offset)?,
            by_ref,
        });
    }
    Ok(MirType::Struct(MirStructType {
        name,
        fields,
        size: narrow(layout.whole.size)?,
        align: layout.whole.align,
    }))
}

pub fn lower_program_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    program: hir::hir_def::program::ProgramDecl<'db>,
) -> Result<MirType, LowerTypeError> {
    let layout = hir::hir_ty::layout::program_layout(db, program)
        .as_ref()
        .ok_or_else(|| no_layout("a PROGRAM"))?;
    instance_struct(db, program.name_with_case(db), layout)
}

/// Lower a Class type to MirType::Struct.
pub fn lower_class_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    class: Class<'db>,
) -> Result<MirType, LowerTypeError> {
    lower_instance_struct(
        db,
        hir::hir_def::pous::pou::Pou::Class(class),
        super::naming::qualified_pou_ident(db, Type::Class(class)),
    )
}
