//! Builder for the debug-symbol table; the on-wire types live in [`debug_format`].
//! Walks configuration globals and program-instance fields down to their
//! elementary leaves. Pointers are not emitted.

pub use debug_format::{
    DEBUG_SYMBOLS_SECTION, DEBUG_SYMBOLS_VERSION, DebugSymbols, SymType, Symbol, TypeDesc,
};

use crate::types::{MirElementary, MirType};
use db::WorkspaceDataBase;
use hir::hir_def::interned::identifier::Ident;
use rustc_hash::FxHashMap;

/// Interner for the on-wire type table: one [`TypeDesc`] per distinct
/// layout, referenced by index.
#[derive(Default)]
pub struct TypeTable {
    entries: Vec<TypeDesc>,
    /// Structural key (the entry's `Debug` rendering) → index.
    index: FxHashMap<String, u32>,
}

impl TypeTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// The finished table, ready for `DebugSymbols::types` / `DebugLocals::types`.
    pub fn into_entries(self) -> Vec<TypeDesc> {
        self.entries
    }

    /// Intern an enumeration's variant table, returning its id.
    pub fn intern_enum(&mut self, db: &dyn WorkspaceDataBase, e: &crate::types::MirEnumType) -> u32 {
        let desc = TypeDesc::Enum {
            name: e.name.text(db).to_string(),
            storage: sym_type_of(e.storage),
            variants: e
                .variants
                .iter()
                .map(|(name, value)| (name.to_string(), *value))
                .collect(),
        };
        self.push(desc)
    }

    /// Intern `ty`, returning its id. Children are interned first, so a
    /// descriptor only ever references earlier entries.
    pub fn intern(&mut self, db: &dyn WorkspaceDataBase, ty: &MirType) -> u32 {
        let desc = match ty {
            MirType::Elementary(_) | MirType::Enum(_) | MirType::Subrange(_) | MirType::String { .. } => {
                // Exactly the scalar set `scalar_sym_ty` covers.
                TypeDesc::Scalar(scalar_sym_ty(ty).expect("scalar arm covers scalar types"))
            }
            MirType::Struct(s) => {
                let fields = s
                    .fields
                    .iter()
                    .map(|f| debug_format::FieldDesc {
                        name: f.name.text(db).to_string(),
                        offset: f.offset,
                        ty: self.intern(db, &f.ty),
                    })
                    .collect();
                TypeDesc::Struct {
                    name: s.name.text(db).to_string(),
                    size: s.size,
                    fields,
                }
            }
            MirType::Array(a) => TypeDesc::Array {
                dimensions: a.dimensions.clone(),
                total_elements: a.total_elements,
                elem_size: a.element_size,
                elem: self.intern(db, &a.element_type),
            },
            // A pointer (REF_TO / by-ref VAR_IN_OUT slot) is locatable but not
            // walkable — chasing it needs a live dereference.
            MirType::Pointer(_) => TypeDesc::Opaque { size: 4 },
            MirType::Void => TypeDesc::Opaque { size: 0 },
        };
        self.push(desc)
    }

    /// Deduplicate by structural identity.
    fn push(&mut self, desc: TypeDesc) -> u32 {
        let key = format!("{desc:?}");
        if let Some(&id) = self.index.get(&key) {
            return id;
        }
        let id = self.entries.len() as u32;
        self.entries.push(desc);
        self.index.insert(key, id);
        id
    }
}

/// Map a resolved elementary type to its on-wire [`SymType`] tag.
pub fn sym_type_of(e: MirElementary) -> SymType {
    match e {
        MirElementary::Bool => SymType::Bool,
        MirElementary::SInt => SymType::SInt,
        MirElementary::Int => SymType::Int,
        MirElementary::DInt => SymType::DInt,
        MirElementary::LInt => SymType::LInt,
        MirElementary::USInt => SymType::USInt,
        MirElementary::UInt => SymType::UInt,
        MirElementary::UDInt => SymType::UDInt,
        MirElementary::ULInt => SymType::ULInt,
        MirElementary::Byte => SymType::Byte,
        MirElementary::Word => SymType::Word,
        MirElementary::DWord => SymType::DWord,
        MirElementary::LWord => SymType::LWord,
        MirElementary::Real => SymType::Real,
        MirElementary::LReal => SymType::LReal,
        MirElementary::Char => SymType::Char,
        MirElementary::Time => SymType::Time,
        MirElementary::LTime => SymType::LTime,
        MirElementary::Date => SymType::Date,
        MirElementary::LDate => SymType::LDate,
        MirElementary::Tod => SymType::Tod,
        MirElementary::LTod => SymType::LTod,
        MirElementary::DateAndTime => SymType::DateAndTime,
        MirElementary::LDateTime => SymType::LDateTime,
    }
}

/// Leaf budget per root variable: the most scalar leaves one root may
/// expand into the table. Arrays past it stay described by their
/// [`ArraySym`] and are resolved on demand.
const MAX_ROOT_LEAVES: u32 = 1024;

/// Emit a [`Symbol`] for every elementary leaf reachable from a root
/// variable at `addr` with type `ty`. Structs add `.field`, arrays `[i]`;
/// enums and subranges emit their underlying integer; pointers are skipped.
#[allow(clippy::too_many_arguments)]
pub fn walk_type(
    db: &dyn WorkspaceDataBase,
    path: &str,
    addr: u32,
    ty: &MirType,
    global: bool,
    out: &mut Vec<Symbol>,
    arrays: &mut Vec<debug_format::ArraySym>,
    types: &mut TypeTable,
    budget: &mut u32,
    arrays_eager: bool,
) {
    // The budget gates leaves; descriptors are always recorded.
    if *budget == 0 && !matches!(ty, MirType::Array(_) | MirType::Struct(_)) {
        return;
    }
    match ty {
        MirType::Elementary(e) => {
            out.push(Symbol {
                path: path.to_string(),
                address: addr,
                size: e.size_bytes(),
                ty: sym_type_of(*e),
                global,
                named_type: None,
            });
            *budget = budget.saturating_sub(1);
        }
        // An enum leaf references its variant table so a debugger can name
        // the value.
        MirType::Enum(e) => {
            out.push(Symbol {
                path: path.to_string(),
                address: addr,
                size: e.storage.size_bytes(),
                ty: sym_type_of(e.storage),
                global,
                named_type: Some(types.intern_enum(db, e)),
            });
            *budget = budget.saturating_sub(1);
        }
        // A subrange is its base type; the bounds are display-only.
        MirType::Subrange(s) => {
            out.push(Symbol {
                path: path.to_string(),
                address: addr,
                size: s.base.size_bytes(),
                ty: sym_type_of(s.base),
                global,
                named_type: None,
            });
            *budget = budget.saturating_sub(1);
        }
        MirType::Struct(s) => {
            for f in &s.fields {
                let child = format!("{path}.{}", f.name.text(db));
                walk_type(
                    db,
                    &child,
                    addr + f.offset,
                    &f.ty,
                    global,
                    out,
                    arrays,
                    types,
                    budget,
                    arrays_eager,
                );
            }
        }
        // The descriptor is the durable record: shape once, elements resolved on
        // demand by `locate`. Frame locals stay eager (`arrays_eager`): their
        // paths are frame-relative and only alive while the stop is.
        MirType::Array(a) => {
            let elem_ty = scalar_sym_ty(&a.element_type);
            arrays.push(debug_format::ArraySym {
                path: path.to_string(),
                address: addr,
                dimensions: a.dimensions.clone(),
                total_elements: a.total_elements,
                elem_size: a.element_size,
                elem_ty,
                global,
                // An aggregate element gets its layout interned so members resolve
                // on demand.
                elem_type: elem_ty
                    .is_none()
                    .then(|| types.intern(db, &a.element_type)),
            });
            if arrays_eager {
                for k in 0..a.total_elements {
                    if *budget == 0 {
                        break;
                    }
                    let child = debug_format::element_path(path, k, &a.dimensions);
                    walk_type(
                        db,
                        &child,
                        addr + k * a.element_size,
                        &a.element_type,
                        global,
                        out,
                        arrays,
                        types,
                        budget,
                        arrays_eager,
                    );
                }
            }
        }
        // STRING → one leaf carrying capacity; the runtime reads the 4-byte len
        // prefix then that many UTF-8 bytes.
        MirType::String { capacity } => {
            out.push(Symbol {
                path: path.to_string(),
                address: addr,
                size: 4 + *capacity,
                ty: SymType::String {
                    capacity: *capacity,
                },
                global,
                named_type: None,
            });
            *budget = budget.saturating_sub(1);
        }
        // Pointers are raw addresses; not emitted yet.
        MirType::Pointer(_) | MirType::Void => {}
    }
}

/// The decode type of one scalar array element; `None` for aggregates.
fn scalar_sym_ty(ty: &MirType) -> Option<SymType> {
    match ty {
        MirType::Elementary(e) => Some(sym_type_of(*e)),
        MirType::Enum(e) => Some(sym_type_of(e.storage)),
        MirType::Subrange(s) => Some(sym_type_of(s.base)),
        MirType::String { capacity } => Some(SymType::String {
            capacity: *capacity,
        }),
        _ => None,
    }
}

/// Emit symbols for one root variable `root` at `base` with type `ty`.
#[allow(clippy::too_many_arguments)]
pub fn collect_root(
    db: &dyn WorkspaceDataBase,
    root: &str,
    base: u32,
    ty: &MirType,
    global: bool,
    out: &mut Vec<Symbol>,
    arrays: &mut Vec<debug_format::ArraySym>,
    types: &mut TypeTable,
) {
    let mut budget = MAX_ROOT_LEAVES;
    walk_type(db, root, base, ty, global, out, arrays, types, &mut budget, false);
}

/// As [`collect_root`], but with eager array elements — for FRAME locals,
/// whose relative paths the on-demand read cannot address.
pub fn collect_frame_root(
    db: &dyn WorkspaceDataBase,
    root: &str,
    base: u32,
    ty: &MirType,
    out: &mut Vec<Symbol>,
    arrays: &mut Vec<debug_format::ArraySym>,
    types: &mut TypeTable,
) {
    let mut budget = MAX_ROOT_LEAVES;
    walk_type(db, root, base, ty, false, out, arrays, types, &mut budget, true);
}

/// A leaf symbol's dotted path from a root segment and a field name.
pub fn join_path(db: &dyn WorkspaceDataBase, root: &str, field: Ident) -> String {
    format!("{root}.{}", field.text(db))
}
