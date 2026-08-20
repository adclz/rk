use db::WorkspaceDataBase;
use hir::{
    hir_def::{
        expressions::{
            spec::{Array, ElementarySpec, Struct, SubRange},
        },
        interned::identifier::Ident,
        pous::{class::Class, function_block::FunctionBlock},
    },
    hir_ty::{infer::Infer, ty::Type},
};

use compact_str::CompactString;

use crate::{
    memory::align_to,
    types::{
        MirArrayType, MirElementary, MirEnumType, MirStructField, MirStructType, MirSubrangeType,
        MirType,
    },
};

#[derive(Debug, thiserror::Error)]
pub enum LowerTypeError {
    #[error("Unsupported type: {0}")]
    UnsupportedType(String),

    #[error("Type could not be resolved (Type::Never encountered)")]
    UnresolvedType,

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
    pub fn location(&self) -> Option<(auto_lsp::default::db::file::File, auto_lsp::tree_sitter::Range)> {
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
        Type::DataType(dt) => Some(dt.name(db)),
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
        ElementarySpec::Bool | ElementarySpec::REDGEBool | ElementarySpec::FEDGEBool => {
            MirElementary::Bool
        }
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
    Ok(apply_sized_string(db, spec, lower_type(db, spec.infer(db))?))
}

fn apply_sized_string<'db>(
    db: &'db dyn WorkspaceDataBase,
    spec: hir::hir_def::expressions::spec::Spec<'db>,
    mir: MirType,
) -> MirType {
    if !matches!(mir, MirType::String { .. }) {
        return mir;
    }
    match declared_string_capacity(db, spec, 0) {
        Some(capacity) => MirType::String { capacity },
        None => mir,
    }
}

/// The `N` a spec declares for a STRING, seen through whatever names it.
///
/// `Type::normalize` collapses `STRING[N]` and plain `STRING` onto the same
/// type, so the length only survives on the SPEC. It does not always survive on
/// the spec at hand either: `s : Alias10` where `TYPE Alias10 : STRING[10]`
/// carries a `Target`, and the `SizedString` sits on the data type's own spec
/// one hop away. Following that hop is the difference between a 10-character
/// string and an 80-character one.
fn declared_string_capacity<'db>(
    db: &'db dyn WorkspaceDataBase,
    spec: hir::hir_def::expressions::spec::Spec<'db>,
    depth: u32,
) -> Option<u32> {
    use hir::hir_def::expressions::spec::SpecKind;
    // A cyclic alias is rejected by HIR (E09xx); stop regardless so lowering
    // terminates on a body that was compiled anyway.
    if depth > 16 {
        return None;
    }
    match spec.kind(db) {
        SpecKind::SizedString(length_expr) => length_expr.as_range(db).map(|n| n as u32),
        SpecKind::Ref(inner) => declared_string_capacity(db, *inner, depth + 1),
        // Named: ask the data type it resolves to for its own spec. Using the
        // inferred type rather than re-resolving the name keeps the binding
        // HIR's decision.
        SpecKind::Target(_) => match spec.infer(db) {
            Type::DataType(dt) => declared_string_capacity(db, dt.spec(db), depth + 1),
            _ => None,
        },
        _ => None,
    }
}

/// Lower a struct type, optionally with an explicit name
/// (used when the name comes from a wrapping DataType or variable).
pub fn lower_struct_type_named<'db>(
    db: &'db dyn WorkspaceDataBase,
    struct_type: Struct<'db>,
    name: Option<Ident>,
) -> Result<MirType, LowerTypeError> {
    let mut offset = 0u32;
    let mut max_align = 1u32;
    let mut fields = Vec::new();

    for element in struct_type.elements(db) {
        let mir_type = lower_spec(db, element.spec(db))?;
        let field_align = mir_type.alignment();
        let field_size = mir_type.size_bytes();

        max_align = max_align.max(field_align);
        offset = align_to(offset, field_align);

        fields.push(MirStructField {
            name: element.name(db),
            ty: mir_type,
            offset,
            by_ref: false,
        });

        offset += field_size;
    }

    // Pad to alignment at the end
    offset = align_to(offset, max_align);

    // Use provided name or create an anonymous one
    let struct_name = name.unwrap_or_else(|| Ident::new(db, CompactString::from("<anon_struct>")));

    Ok(MirType::Struct(MirStructType {
        name: struct_name,
        fields,
        size: offset,
        align: max_align,
    }))
}

fn lower_array_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    array_type: Array<'db>,
) -> Result<MirType, LowerTypeError> {
    let element_type = lower_spec(db, array_type.of_type(db))?;
    let element_size = element_type.size_bytes();
    let element_align = element_type.alignment();

    let mut dimensions = Vec::new();
    let mut total_elements = 1u32;

    for (start_expr, end_expr) in array_type.subranges(db) {
        let start = extract_integer_literal(db, start_expr)?;
        let end = extract_integer_literal(db, end_expr)?;
        dimensions.push((start, end));

        let dim_size = (end - start + 1).max(0) as u32;
        total_elements = total_elements.checked_mul(dim_size).ok_or_else(|| {
            LowerTypeError::UnsupportedType(format!(
                "Array dimension overflow: {} * {}",
                total_elements, dim_size
            ))
        })?;
    }

    let total_size = element_size.checked_mul(total_elements).ok_or_else(|| {
        LowerTypeError::UnsupportedType(format!(
            "Array size overflow: {} * {}",
            element_size, total_elements
        ))
    })?;

    Ok(MirType::Array(MirArrayType {
        element_type: Box::new(element_type),
        dimensions,
        total_elements,
        element_size,
        size: total_size,
        align: element_align,
    }))
}


/// Each enumerator paired with its declared numeric value: an explicit
/// `(Idle := 10, Run := 20)`, or continuing from the previous value from 0.
/// The single source of enum numbering for both the type and its literals.
pub fn enum_variant_values<'db>(
    db: &'db dyn WorkspaceDataBase,
    enum_type: hir::hir_def::expressions::spec::Enum<'db>,
) -> Result<Vec<(hir::hir_def::expressions::spec::EnumVariant<'db>, i64)>, LowerTypeError> {
    let mut out = Vec::new();
    let mut next: i64 = 0;
    for variant in enum_type.variants(db).iter() {
        let value = match variant.value {
            Some(expr) => extract_integer_literal(db, expr)?,
            None => next,
        };
        out.push((*variant, value));
        next = value + 1;
    }
    Ok(out)
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
        variants.push((variant.name.ident.text(db).clone(), value));
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

    let lower = extract_integer_literal(db, subrange.lower(db))?;
    let upper = extract_integer_literal(db, subrange.upper(db))?;

    Ok(MirType::Subrange(MirSubrangeType { base, lower, upper }))
}

/// Lower a FunctionBlock to a `MirType::Struct` named with its
/// namespace-qualified identifier, which nested-FB instance fields
/// resolve against.
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

/// Lay out an FB/CLASS instance from HIR's [`instance_members`] (base-most
/// first, so a derived instance is layout-compatible with its base); MIR
/// only turns the list into offsets.
fn lower_instance_struct<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: hir::hir_def::pous::pou::Pou<'db>,
    name: Ident,
) -> Result<MirType, LowerTypeError> {
    let mut offset = 0u32;
    let mut max_align = 1u32;
    let mut fields = Vec::new();

    for member in hir::hir_ty::head::inheritance::instance_members(db, pou) {
        let var = member.var;
        let mir_type = lower_spec(db, var.spec(db))?;
        // A VAR_IN_OUT field holds the address of the caller's l-value: a
        // pointer the body auto-derefs and the call site writes once.
        let is_inout = var.kind(db) == hir::hir_def::pous::variable::VariableKind::InOut;
        let mir_type = if is_inout {
            MirType::Pointer(Box::new(mir_type))
        } else {
            mir_type
        };
        let field_align = mir_type.alignment();
        let field_size = mir_type.size_bytes();

        max_align = max_align.max(field_align);
        offset = align_to(offset, field_align);

        fields.push(MirStructField {
            name: var.name(db),
            ty: mir_type,
            offset,
            by_ref: is_inout,
        });

        offset += field_size;
    }

    offset = align_to(offset, max_align);

    Ok(MirType::Struct(MirStructType {
        name,
        fields,
        size: offset,
        align: max_align,
    }))
}

pub fn lower_program_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    program: hir::hir_def::program::ProgramDecl<'db>,
) -> Result<MirType, LowerTypeError> {
    let mut offset = 0u32;
    let mut max_align = 1u32;
    let mut fields = Vec::new();

    for var in program.variables(db) {
        // VAR_EXTERNAL resolves to a global's address; not instance state.
        if var.kind(db) == hir::hir_def::pous::variable::VariableKind::External {
            continue;
        }
        // VAR_TEMP is a body local, fresh at every invocation, not instance
        // state.
        if var.kind(db) == hir::hir_def::pous::variable::VariableKind::Temp {
            continue;
        }
        let mir_type = lower_spec(db, var.spec(db))?;
        let field_align = mir_type.alignment();
        let field_size = mir_type.size_bytes();

        max_align = max_align.max(field_align);
        offset = align_to(offset, field_align);
        fields.push(MirStructField {
            name: var.name(db),
            ty: mir_type,
            offset,
            // PROGRAM instances are driven by the scheduler, never through an
            // `FbCall`, so PROGRAM VAR_IN_OUT stays value-based.
            by_ref: false,
        });
        offset += field_size;
    }

    offset = align_to(offset, max_align);

    Ok(MirType::Struct(MirStructType {
        name: program.name(db),
        fields,
        size: offset,
        align: max_align,
    }))
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

/// Fold a compile-time integer (array/subrange bound, enum value).
///
/// Delegates to HIR's [`Expr::as_const_int`] — the one const-integer evaluator,
/// also used by the checks that validate these same expressions. MIR previously
/// had its own literal matcher, which accepted a different set than the checker
/// did, so validation and lowering could disagree about what counts as a
/// constant.
/// The i64 carries every base an enum or subrange may declare — the old i32
/// clamp refused legal `LInt`/`LWORD` variant values and subrange bounds
/// (both `MirSubrangeType` bounds and enum values are stored as i64).
fn extract_integer_literal<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: hir::hir_def::expressions::expression::Expr<'db>,
) -> Result<i64, LowerTypeError> {
    expr.as_const_int(db).ok_or_else(|| {
        LowerTypeError::UnsupportedType("expected a constant integer".to_string())
    })
}
