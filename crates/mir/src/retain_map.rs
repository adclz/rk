//! The per-field RETAIN persistence map: which byte ranges of the retain
//! band survive a power cycle. Relocation is whole-instance, persistence is
//! per-field. Rules (matching `schedule::is_retain_field`): `RETAIN` retains
//! the subtree, an absent qualifier lets nested `RETAIN` through,
//! `NON_RETAIN` prunes the subtree, `VAR_TEMP` and by-ref pointer fields
//! never persist, `PROGRAM RETAIN` retains every non-pruned field.

use db::WorkspaceDataBase;
use debug_format::{RetainMap, RetainRange};
use hir::{
    Qualifier,
    hir_def::{
        interned::identifier::Ident,
        pous::variable::{VariableDecl, VariableKind},
    },
    hir_ty::{infer::Infer, ty::Type},
};
use rustc_hash::FxHashMap;

use crate::{
    memory::RetainEntry,
    schedule::{MirSchedule, ProgramInfo},
    types::{MirStructField, MirType},
};

/// Every retained range, at final addresses; only instances relocated into
/// the band are walked.
pub fn build_retain_map<'db>(
    db: &'db dyn WorkspaceDataBase,
    schedule: Option<&MirSchedule>,
    program_infos: &FxHashMap<Ident, ProgramInfo<'db>>,
    retain_globals: &[RetainEntry],
    global_types: &FxHashMap<Ident, MirType>,
    retain_base: u32,
    retain_size: u32,
) -> Result<RetainMap, crate::lower::lower_type::LowerTypeError> {
    let mut ranges = Vec::new();

    // Retained VAR_GLOBALs are already per-variable — one range each.
    for g in retain_globals {
        ranges.push(RetainRange {
            path: g.name.text(db).to_string(),
            addr: g.address,
            size: g.size,
            type_key: global_types
                .get(&g.name)
                .map(|ty| type_key(db, ty))
                .unwrap_or(0),
        });
    }

    if let Some(sched) = schedule {
        let band = retain_base..retain_base.saturating_add(retain_size);
        for task in &sched.tasks {
            for inst in &task.programs {
                if retain_size == 0 || !band.contains(&inst.instance_addr) {
                    continue; // not banded — nothing persists
                }
                // `lower_schedule` refused the miss, so this firing means the invariant
                // broke.
                let Some(info) = program_infos.get(&inst.prog_name) else {
                    return Err(crate::lower::lower_type::LowerTypeError::UnsupportedType(
                        format!(
                            "scheduled instance '{}' names program '{}' with no lowered info",
                            inst.inst_name.text(db),
                            inst.prog_name.text(db)
                        ),
                    ));
                };
                walk_members(
                    db,
                    inst.inst_name.text(db),
                    inst.instance_addr,
                    &info.struct_type.fields,
                    info.decl.variables(db),
                    inst.config_retain == Some(true),
                    &mut ranges,
                );
            }
        }
    }

    Ok(RetainMap::new(ranges))
}

/// Walk one FB/class/program struct level: pair each MIR field with its HIR
/// declaration (by name) and apply the retention rules.
fn walk_members<'db>(
    db: &'db dyn WorkspaceDataBase,
    root: &str,
    base: u32,
    fields: &[MirStructField],
    vars: &[VariableDecl<'db>],
    inherited_retain: bool,
    out: &mut Vec<RetainRange>,
) {
    let var_by_name: FxHashMap<Ident, VariableDecl<'db>> =
        vars.iter().map(|v| (v.name(db), *v)).collect();
    for f in fields {
        let Some(v) = var_by_name.get(&f.name) else {
            continue;
        };
        let q = v.qualifier(db);
        if q.contains(Qualifier::NON_RETAIN) {
            continue; // explicit NON_RETAIN prunes the subtree
        }
        if v.kind(db) == VariableKind::Temp {
            continue; // temps are per-scan state, never persisted
        }
        if f.by_ref {
            continue; // by-ref VAR_IN_OUT pointer — transient, re-bound per call
        }
        let retained = inherited_retain || q.contains(Qualifier::RETAIN);
        let path = crate::debug_symbols::join_path(db, root, f.name);
        walk_value(
            db,
            &path,
            base + f.offset,
            f,
            v.spec(db).infer(db),
            retained,
            out,
        );
    }
}

/// One value at `addr`: FB/class instances recurse, plain data is a single
/// range when retained.
fn walk_value<'db>(
    db: &'db dyn WorkspaceDataBase,
    path: &str,
    addr: u32,
    field: &MirStructField,
    hir_ty: Type<'db>,
    retained: bool,
    out: &mut Vec<RetainRange>,
) {
    match (&field.ty, hir_ty.normalize(db)) {
        // FB/class instance: members carry their own qualifiers — recurse.
        (MirType::Struct(s), Type::FunctionBlock(fb)) => {
            walk_members(db, path, addr, &s.fields, fb.variables(db), retained, out);
        }
        (MirType::Struct(s), Type::Class(class)) => {
            walk_members(
                db,
                path,
                addr,
                &s.fields,
                class.variables(db),
                retained,
                out,
            );
        }
        // Arrays: one range when possible; per-element when the element type
        // needs recursion.
        (MirType::Array(a), hir_norm) => {
            let elem_hir = match hir_norm {
                Type::Array(arr) => arr.of_type(db).infer(db),
                _ => hir_ty,
            };
            let elem_is_pou = matches!(
                elem_hir.normalize(db),
                Type::FunctionBlock(_) | Type::Class(_)
            );
            if retained && !elem_is_pou && !type_has_pointer_holes(&a.element_type) {
                out.push(RetainRange {
                    path: path.to_string(),
                    addr,
                    size: field.ty.size_bytes(),
                    type_key: type_key(db, &field.ty),
                });
                return;
            }
            if !elem_is_pou {
                // Plain data, not retained: nothing can shine through.
                if retained {
                    // Pointer holes outside a POU context are not expressible: a whole
                    // range.
                    out.push(RetainRange {
                        path: path.to_string(),
                        addr,
                        size: field.ty.size_bytes(),
                        type_key: type_key(db, &field.ty),
                    });
                }
                return;
            }
            // POU elements: recurse each element so member qualifiers and
            // pointer holes apply per element.
            let elem_field = MirStructField {
                name: field.name,
                ty: (*a.element_type).clone(),
                offset: 0,
                by_ref: false,
            };
            for i in 0..a.total_elements {
                let p = format!("{path}[{i}]");
                walk_value(
                    db,
                    &p,
                    addr + i * a.element_size,
                    &elem_field,
                    elem_hir,
                    retained,
                    out,
                );
            }
        }
        // Leaf data: one range iff retained.
        _ => {
            if retained {
                out.push(RetainRange {
                    path: path.to_string(),
                    addr,
                    size: field.ty.size_bytes(),
                    type_key: type_key(db, &field.ty),
                });
            }
        }
    }
}

/// Does this type contain by-ref pointer fields anywhere — holes that must be
/// excluded from persistence?
fn type_has_pointer_holes(ty: &MirType) -> bool {
    match ty {
        MirType::Struct(s) => s
            .fields
            .iter()
            .any(|f| f.by_ref || type_has_pointer_holes(&f.ty)),
        MirType::Array(a) => type_has_pointer_holes(&a.element_type),
        _ => false,
    }
}

/// A structural fingerprint of a type for the retain map's layout hash:
/// `x : REAL` becoming `x : DINT` keeps the size but changes the key.
/// Renaming a STRUCT does not invalidate a snapshot; reordering or
/// retyping its fields does.
pub(crate) fn type_key(db: &dyn WorkspaceDataBase, ty: &MirType) -> u32 {
    const FNV_OFFSET: u32 = 0x811c9dc5;
    const FNV_PRIME: u32 = 0x01000193;

    fn eat(h: &mut u32, bytes: &[u8]) {
        for &b in bytes {
            *h ^= b as u32;
            *h = h.wrapping_mul(FNV_PRIME);
        }
    }

    fn walk(db: &dyn WorkspaceDataBase, h: &mut u32, ty: &MirType, depth: u32) {
        // HIR rejects self-referential types; stop regardless.
        if depth > 32 {
            return;
        }
        match ty {
            MirType::Elementary(e) => {
                eat(h, b"E");
                eat(h, &[*e as u8]);
            }
            MirType::String { capacity } => {
                eat(h, b"S");
                eat(h, &capacity.to_le_bytes());
            }
            MirType::Array(a) => {
                eat(h, b"A");
                eat(h, &a.total_elements.to_le_bytes());
                for (lo, hi) in &a.dimensions {
                    eat(h, &lo.to_le_bytes());
                    eat(h, &hi.to_le_bytes());
                }
                walk(db, h, &a.element_type, depth + 1);
            }
            MirType::Struct(s) => {
                eat(h, b"T");
                for f in &s.fields {
                    // Field names are part of the shape: the file is scattered back by path.
                    eat(h, f.name.text(db).as_bytes());
                    eat(h, &f.offset.to_le_bytes());
                    walk(db, h, &f.ty, depth + 1);
                }
            }
            MirType::Enum(e) => {
                eat(h, b"N");
                eat(h, &[e.storage as u8]);
                // Variant VALUES, not names: renaming a variant does not change
                // the bytes, but renumbering one does.
                for (_, value) in &e.variants {
                    eat(h, &value.to_le_bytes());
                }
            }
            MirType::Subrange(sr) => {
                eat(h, b"R");
                eat(h, &[sr.base as u8]);
                eat(h, &sr.lower.to_le_bytes());
                eat(h, &sr.upper.to_le_bytes());
            }
            MirType::Pointer(inner) => {
                eat(h, b"P");
                walk(db, h, inner, depth + 1);
            }
            MirType::Void => eat(h, b"V"),
        }
    }

    let mut h = FNV_OFFSET;
    walk(db, &mut h, ty, 0);
    h
}
