use db::WorkspaceDataBase;
use hir::{
    hir_def::{
        expressions::{
            expression::{Elementary, ExprKind, PrimaryExpr},
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
        MirArrayType, MirElementary, MirEnumType, MirStringKind, MirStructField, MirStructType,
        MirSubrangeType, MirType,
    },
};

#[derive(Debug, thiserror::Error)]
pub enum LowerTypeError {
    #[error("Unsupported type: {0}")]
    UnsupportedType(String),

    #[error("Type could not be resolved (Type::Never encountered)")]
    UnresolvedType,
}

/// Convert a HIR `Type` to a `MirType`.
/// The type is normalized first to resolve aliases, variables, and callable return types.
pub fn lower_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: Type<'db>,
) -> Result<MirType, LowerTypeError> {
    let ty = ty.normalize(db);

    match ty {
        Type::Elementary(spec) if is_string_type(spec) => {
            Ok(MirType::String(elementary_to_string_kind(spec)))
        }

        Type::Elementary(spec) => {
            let mir_elem = elementary_spec_to_mir(spec)?;
            Ok(MirType::Elementary(mir_elem))
        }

        Type::RefTo(_) => Ok(MirType::Pointer(Box::new(MirType::Void))),

        Type::Struct(s) => lower_struct_type(db, s),

        Type::Array(arr) => lower_array_type(db, arr),

        Type::Enum(e) => lower_enum_type(db, e),

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
        ElementarySpec::WChar => MirElementary::WChar,
        ElementarySpec::Time => MirElementary::Time,
        ElementarySpec::LTime => MirElementary::LTime,
        ElementarySpec::Date => MirElementary::Date,
        ElementarySpec::LDate => MirElementary::LDate,
        ElementarySpec::Tod => MirElementary::Tod,
        ElementarySpec::LTod => MirElementary::LTod,
        ElementarySpec::DateAndTime => MirElementary::DateAndTime,
        ElementarySpec::LDateTime => MirElementary::LDateTime,
        _ => {
            return Err(LowerTypeError::UnsupportedType(format!(
                "ANY type specs cannot be lowered: {:?}",
                spec
            )));
        }
    })
}

/// Resolve an ANY_* type by finding a matching concrete type in the substitution map.
fn resolve_any_from_subs(
    any_spec: ElementarySpec,
    subs: &rustc_hash::FxHashMap<Ident, ElementarySpec>,
) -> Result<MirType, LowerTypeError> {
    for concrete in subs.values() {
        if any_spec.accepts(*concrete) {
            return Ok(MirType::Elementary(elementary_spec_to_mir(*concrete)?));
        }
    }
    Err(LowerTypeError::UnsupportedType(format!(
        "ANY type specs cannot be lowered: {:?}",
        any_spec
    )))
}

fn lower_struct_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    struct_type: Struct<'db>,
) -> Result<MirType, LowerTypeError> {
    lower_struct_type_named(db, struct_type, None)
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
        let field_type = element.spec(db).infer(db);
        let mir_type = lower_type(db, field_type)?;
        let field_align = mir_type.alignment();
        let field_size = mir_type.size_bytes();

        max_align = max_align.max(field_align);
        offset = align_to(offset, field_align);

        fields.push(MirStructField {
            name: element.name(db),
            ty: mir_type,
            offset,
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
    let element_type_hir = array_type.of_type(db).infer(db);
    let element_type = lower_type(db, element_type_hir)?;
    let element_size = element_type.size_bytes();
    let element_align = element_type.alignment();

    let mut dimensions = Vec::new();
    let mut total_elements = 1u32;

    for (start_expr, end_expr) in array_type.subranges(db) {
        let start = extract_integer_literal(db, start_expr)?;
        let end = extract_integer_literal(db, end_expr)?;
        dimensions.push((start as i64, end as i64));

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

fn lower_enum_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    enum_type: hir::hir_def::expressions::spec::Enum<'db>,
) -> Result<MirType, LowerTypeError> {
    lower_enum_type_named(db, enum_type, None)
}

pub fn lower_enum_type_named<'db>(
    db: &'db dyn WorkspaceDataBase,
    enum_type: hir::hir_def::expressions::spec::Enum<'db>,
    name: Option<Ident>,
) -> Result<MirType, LowerTypeError> {
    let mut variants = Vec::new();
    for (i, variant) in enum_type.variants(db).iter().enumerate() {
        let variant_name = variant.name.ident.text(db).clone();
        variants.push((variant_name, i as i64));
    }

    let enum_name = name.unwrap_or_else(|| Ident::new(db, CompactString::from("<anon_enum>")));

    // Enums are stored as DInt by default
    Ok(MirType::Enum(MirEnumType {
        name: enum_name,
        variants,
        storage: MirElementary::DInt,
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

    let lower = extract_integer_literal(db, subrange.lower(db))? as i64;
    let upper = extract_integer_literal(db, subrange.upper(db))? as i64;

    Ok(MirType::Subrange(MirSubrangeType { base, lower, upper }))
}

/// Lower a FunctionBlock type to MirType::Struct (same layout as a struct with FB variables as fields).
pub fn lower_fb_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    fb: FunctionBlock<'db>,
) -> Result<MirType, LowerTypeError> {
    lower_fb_type_with_subs(db, fb, &rustc_hash::FxHashMap::default())
}

/// Lower a FunctionBlock type with ANY_* type substitutions.
/// `any_subs` maps variable names to concrete ElementarySpec types.
pub fn lower_fb_type_with_subs<'db>(
    db: &'db dyn WorkspaceDataBase,
    fb: FunctionBlock<'db>,
    any_subs: &rustc_hash::FxHashMap<hir::hir_def::interned::identifier::Ident, hir::hir_def::expressions::spec::ElementarySpec>,
) -> Result<MirType, LowerTypeError> {
    let mut offset = 0u32;
    let mut max_align = 1u32;
    let mut fields = Vec::new();

    for var in fb.variables(db) {
        let var_type = var.spec(db).infer(db);

        // Check if this variable has an ANY_* type that should be substituted
        let mir_type = if let hir::hir_ty::ty::Type::Elementary(elem) = var_type {
            if elem.is_any() {
                if let Some(concrete) = any_subs.get(&var.name(db)) {
                    MirType::Elementary(elementary_spec_to_mir(*concrete)?)
                } else if let hir::hir_def::expressions::spec::SpecKind::Into(ident) = var.spec(db).kind(db) {
                    // INTO(ref) — resolve from the referenced variable's substitution
                    if let Some(concrete) = any_subs.get(&ident.ident) {
                        MirType::Elementary(elementary_spec_to_mir(*concrete)?)
                    } else {
                        resolve_any_from_subs(elem, any_subs)?
                    }
                } else {
                    // Fallback: find any concrete sub that matches this ANY group
                    resolve_any_from_subs(elem, any_subs)?
                }
            } else {
                lower_type(db, var_type)?
            }
        } else {
            // Check if this is an INTO(ref) variable even if var_type isn't Elementary
            if let hir::hir_def::expressions::spec::SpecKind::Into(ident) = var.spec(db).kind(db) {
                if let Some(concrete) = any_subs.get(&ident.ident) {
                    MirType::Elementary(elementary_spec_to_mir(*concrete)?)
                } else {
                    lower_type(db, var_type)?
                }
            } else {
                lower_type(db, var_type)?
            }
        };
        let field_align = mir_type.alignment();
        let field_size = mir_type.size_bytes();

        max_align = max_align.max(field_align);
        offset = align_to(offset, field_align);

        fields.push(MirStructField {
            name: var.name(db),
            ty: mir_type,
            offset,
        });

        offset += field_size;
    }

    offset = align_to(offset, max_align);

    Ok(MirType::Struct(MirStructType {
        name: fb.name(db),
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
    let mut offset = 0u32;
    let mut max_align = 1u32;
    let mut fields = Vec::new();

    // TODO: Handle inheritance — include parent class fields first
    for var in class.variables(db) {
        let var_type = var.spec(db).infer(db);
        let mir_type = lower_type(db, var_type)?;
        let field_align = mir_type.alignment();
        let field_size = mir_type.size_bytes();

        max_align = max_align.max(field_align);
        offset = align_to(offset, field_align);

        fields.push(MirStructField {
            name: var.name(db),
            ty: mir_type,
            offset,
        });

        offset += field_size;
    }

    offset = align_to(offset, max_align);

    Ok(MirType::Struct(MirStructType {
        name: class.name(db),
        fields,
        size: offset,
        align: max_align,
    }))
}

fn is_string_type(spec: ElementarySpec) -> bool {
    matches!(spec, ElementarySpec::String | ElementarySpec::WString)
}

fn elementary_to_string_kind(spec: ElementarySpec) -> MirStringKind {
    match spec {
        ElementarySpec::WString => MirStringKind::WString,
        _ => MirStringKind::String,
    }
}

fn extract_integer_literal<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: hir::hir_def::expressions::expression::Expr<'db>,
) -> Result<i32, LowerTypeError> {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::InferInteger(int))) => {
            int.as_i32(db).map_err(|e| {
                LowerTypeError::UnsupportedType(format!(
                    "Failed to parse integer literal as i32: {}",
                    e
                ))
            })
        }
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::Int(int))) => {
            int.as_i32(db).map_err(|e| {
                LowerTypeError::UnsupportedType(format!(
                    "Failed to parse Int literal as i32: {}",
                    e
                ))
            })
        }
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::DInt(int))) => {
            int.as_i32(db).map_err(|e| {
                LowerTypeError::UnsupportedType(format!(
                    "Failed to parse DInt literal as i32: {}",
                    e
                ))
            })
        }
        _ => Err(LowerTypeError::UnsupportedType(
            "Array bounds must be integer literals".to_string(),
        )),
    }
}
