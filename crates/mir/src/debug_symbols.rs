// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Builder for the debug-symbol table; the on-wire types live in [`debug_format`].
//! Walks configuration globals and program-instance fields down to their
//! elementary leaves, and lists each pointer with what it points at.

pub use debug_format::{
    DEBUG_SYMBOLS_SECTION, DEBUG_SYMBOLS_VERSION, DebugSymbols, SymType, Symbol, TypeDesc,
};

use crate::types::{MirElementary, MirRefTarget, MirStructType, MirType};
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
    /// A struct as lowered (its `Debug` rendering) → index. Its entry cannot
    /// be the key: a struct that points at itself names its own index.
    structs: FxHashMap<String, u32>,
    /// The structs being interned, innermost last, each with the index it
    /// will have: what a member pointing back at one refers to.
    laying_out: Vec<(Ident, u32)>,
    /// Every named struct interned so far, by name: what a `REF_TO` back to
    /// one refers to when neither the interning nor the walk is inside it.
    named: FxHashMap<Ident, u32>,
}

/// The structs a walk is inside of, outermost first: what a `REF_TO` back to
/// one of them (`a.next`, in a `Node`) points at, when the walk reached it
/// by path and never interned it.
pub type Outer<'t> = [(Ident, &'t MirType)];

impl TypeTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// The finished table, ready for `DebugSymbols::types` / `DebugLocals::types`.
    pub fn into_entries(self) -> Vec<TypeDesc> {
        self.entries
    }

    /// Intern an enumeration's variant table, returning its id.
    pub fn intern_enum(
        &mut self,
        db: &dyn WorkspaceDataBase,
        e: &crate::types::MirEnumType,
    ) -> u32 {
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

    /// Intern `ty`, returning its id.
    pub fn intern(&mut self, db: &dyn WorkspaceDataBase, ty: &MirType) -> u32 {
        self.intern_in(db, ty, false, &[])
    }

    /// Intern `ty` as it stands inside the structs of `outer`. `by_ref` says
    /// a pointer is a VAR_IN_OUT member's.
    pub fn intern_in(
        &mut self,
        db: &dyn WorkspaceDataBase,
        ty: &MirType,
        by_ref: bool,
        outer: &Outer,
    ) -> u32 {
        let desc = match ty {
            MirType::Elementary(_) | MirType::Subrange(_) | MirType::String { .. } => {
                // Exactly the scalar set `scalar_sym_ty` covers, but the
                // enumeration, whose entry names its variants.
                TypeDesc::Scalar(scalar_sym_ty(ty).expect("scalar arm covers scalar types"))
            }
            MirType::Enum(e) => return self.intern_enum(db, e),
            MirType::Struct(s) => return self.intern_struct(db, ty, s, outer),
            MirType::Array(a) => TypeDesc::Array {
                dimensions: a.dimensions.clone(),
                total_elements: a.total_elements,
                elem_size: a.element_size,
                elem: self.intern_in(db, &a.element_type, false, outer),
            },
            // A pointer is locatable, and what it points at is walkable
            // from the address it holds: a live dereference.
            MirType::Pointer(pointee) => TypeDesc::Opaque {
                size: 4,
                target: self.target(db, pointee, outer),
                by_ref,
                conformand: self.conformand(db, pointee, outer),
            },
            MirType::Void | MirType::RefTarget(_) => TypeDesc::Opaque {
                size: 0,
                target: None,
                by_ref: false,
                conformand: None,
            },
        };
        self.push(desc)
    }

    /// A struct's entry. Its index is taken before its members are
    /// interned: one of them may point back at it (`next : REF_TO Node`).
    fn intern_struct(
        &mut self,
        db: &dyn WorkspaceDataBase,
        ty: &MirType,
        s: &MirStructType,
        outer: &Outer,
    ) -> u32 {
        let key = format!("{ty:?}");
        if let Some(&id) = self.structs.get(&key) {
            return id;
        }
        let id = self.entries.len() as u32;
        self.structs.insert(key, id);
        // An anonymous struct's name is one every anonymous struct has.
        if !s.name.text(db).starts_with('<') {
            self.named.entry(s.name).or_insert(id);
        }
        // Held until the members are known.
        self.entries.push(TypeDesc::Opaque {
            size: 0,
            target: None,
            by_ref: false,
            conformand: None,
        });
        self.laying_out.push((s.name, id));
        let fields = s
            .fields
            .iter()
            .map(|f| debug_format::FieldDesc {
                name: f.name_with_case.text(db).to_string(),
                offset: f.offset,
                ty: self.intern_in(db, &f.ty, f.by_ref, outer),
            })
            .collect();
        self.laying_out.pop();
        self.entries[id as usize] = TypeDesc::Struct {
            name: s.name.text(db).to_string(),
            size: s.size,
            fields,
        };
        id
    }

    /// What a pointer to `pointee` points at: its index, or `None` when
    /// lowering has no type for it.
    pub fn target(
        &mut self,
        db: &dyn WorkspaceDataBase,
        pointee: &MirType,
        outer: &Outer,
    ) -> Option<u32> {
        match pointee {
            // An `ARRAY[*]` has no type of its own: see `conformand`.
            MirType::Void
            | MirType::RefTarget(MirRefTarget::Unknown | MirRefTarget::Conformand { .. }) => None,
            MirType::RefTarget(MirRefTarget::Type(ty)) => {
                Some(self.intern_in(db, ty, false, outer))
            }
            // A struct being interned, or else one the walk is inside of,
            // interned now from its own start.
            MirType::RefTarget(MirRefTarget::Enclosing(name)) => {
                if let Some(&(_, id)) = self.laying_out.iter().rev().find(|(n, _)| n == name) {
                    return Some(id);
                }
                if let Some(at) = outer.iter().rposition(|(n, _)| n == name) {
                    return Some(self.intern_in(db, outer[at].1, false, &outer[..at]));
                }
                // Two STRUCTs pointing at each other, reached from the one
                // lowered second: its target is the first, when that one has
                // been interned, or else nothing.
                self.named.get(name).copied()
            }
            // A VAR_IN_OUT's, typed as declared.
            ty => Some(self.intern_in(db, ty, false, outer)),
        }
    }

    /// What a pointer to `pointee` says of the array it is bound to, when
    /// it is an `ARRAY[*]` VAR_IN_OUT member's: the rank, and the element.
    pub fn conformand(
        &mut self,
        db: &dyn WorkspaceDataBase,
        pointee: &MirType,
        outer: &Outer,
    ) -> Option<debug_format::Conformand> {
        let MirType::RefTarget(MirRefTarget::Conformand { rank, element }) = pointee else {
            return None;
        };
        Some(debug_format::Conformand {
            rank: *rank,
            elem: self.intern_in(db, element, false, outer),
        })
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

/// The most aggregates [`collect_containers`] names under one root: an
/// array of structs is entered per element.
const MAX_CONTAINERS: u32 = 1024;

/// Where a walk writes: the leaves, the array descriptors, the pointers, and
/// the types those reference.
pub struct Tables<'a> {
    pub symbols: &'a mut Vec<Symbol>,
    pub arrays: &'a mut Vec<debug_format::ArraySym>,
    pub pointers: &'a mut Vec<debug_format::PointerSym>,
    pub types: &'a mut TypeTable,
}

/// One walk's state: what it is inside of.
struct Walk<'t> {
    global: bool,
    /// The structs the walk is inside of, outermost first.
    outer: Vec<(Ident, &'t MirType)>,
}

/// Emit a [`Symbol`] for every elementary leaf reachable from a root
/// variable at `addr` with type `ty`. Structs add `.field`; an array is one
/// descriptor, its elements resolved on demand; enums and subranges emit
/// their underlying integer; a pointer is listed with what it points at,
/// `by_ref` when it is a VAR_IN_OUT member's.
fn walk_type<'t>(
    db: &dyn WorkspaceDataBase,
    path: &str,
    addr: u32,
    ty: &'t MirType,
    by_ref: bool,
    walk: &mut Walk<'t>,
    out: &mut Tables,
) {
    let global = walk.global;
    match ty {
        MirType::Elementary(e) => {
            out.symbols.push(Symbol {
                path: path.to_string(),
                address: addr,
                size: e.size_bytes(),
                ty: sym_type_of(*e),
                global,
                named_type: None,
                bits: None,
            });
        }
        // An enum leaf references its variant table so a debugger can name
        // the value.
        MirType::Enum(e) => {
            out.symbols.push(Symbol {
                path: path.to_string(),
                address: addr,
                size: e.storage.size_bytes(),
                ty: sym_type_of(e.storage),
                global,
                named_type: Some(out.types.intern_enum(db, e)),
                bits: None,
            });
        }
        // A subrange is its base type; the bounds are display-only.
        MirType::Subrange(s) => {
            out.symbols.push(Symbol {
                path: path.to_string(),
                address: addr,
                size: s.base.size_bytes(),
                ty: sym_type_of(s.base),
                global,
                named_type: None,
                bits: None,
            });
        }
        MirType::Struct(s) => {
            walk.outer.push((s.name, ty));
            for f in s.fields.iter().filter(|f| !f.is_hidden(db)) {
                let child = format!("{path}.{}", f.name_with_case.text(db));
                walk_type(db, &child, addr + f.offset, &f.ty, f.by_ref, walk, out);
            }
            walk.outer.pop();
        }
        // The descriptor is the whole record: shape once, elements resolved
        // on demand, in a frame as in the module.
        MirType::Array(a) => {
            let elem_ty = scalar_sym_ty(&a.element_type);
            out.arrays.push(debug_format::ArraySym {
                path: path.to_string(),
                address: addr,
                dimensions: a.dimensions.clone(),
                total_elements: a.total_elements,
                elem_size: a.element_size,
                elem_ty,
                global,
                // An aggregate element, or a pointer, gets its layout
                // interned so members resolve on demand; an enumeration,
                // its variants.
                elem_type: (elem_ty.is_none() || matches!(&*a.element_type, MirType::Enum(_)))
                    .then(|| out.types.intern_in(db, &a.element_type, false, &walk.outer)),
            });
        }
        // STRING → one leaf carrying capacity; the runtime reads the 4-byte len
        // prefix then that many UTF-8 bytes.
        MirType::String { capacity } => {
            out.symbols.push(Symbol {
                path: path.to_string(),
                address: addr,
                size: 4 + *capacity,
                ty: SymType::String {
                    capacity: *capacity,
                },
                global,
                named_type: None,
                bits: None,
            });
        }
        // An address: what it points at is read through it, live.
        MirType::Pointer(pointee) => {
            out.pointers.push(debug_format::PointerSym {
                path: path.to_string(),
                address: addr,
                global,
                target: out.types.target(db, pointee, &walk.outer),
                by_ref,
                conformand: out.types.conformand(db, pointee, &walk.outer),
            });
        }
        MirType::Void | MirType::RefTarget(_) => {}
    }
}

/// The decode type of one scalar value; `None` for aggregates.
pub(crate) fn scalar_sym_ty(ty: &MirType) -> Option<SymType> {
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
/// `by_ref` says a root that is a pointer is a VAR_IN_OUT member's.
pub fn collect_root(
    db: &dyn WorkspaceDataBase,
    root: &str,
    base: u32,
    ty: &MirType,
    global: bool,
    by_ref: bool,
    out: &mut Tables,
) {
    let mut walk = Walk {
        global,
        outer: Vec::new(),
    };
    walk_type(db, root, base, ty, by_ref, &mut walk, out);
}

/// Name every aggregate under `root` by its base address: what a frame's
/// `this` may point at (structs and struct array elements).
pub fn collect_containers(
    db: &dyn WorkspaceDataBase,
    path: &str,
    addr: u32,
    ty: &MirType,
    global: bool,
    out: &mut Vec<debug_format::ContainerSym>,
) {
    if out.len() as u32 >= MAX_CONTAINERS {
        return;
    }
    match ty {
        MirType::Struct(s) => {
            out.push(debug_format::ContainerSym {
                path: path.to_string(),
                address: addr,
                global,
                type_name: s.name.text(db).to_string(),
            });
            for f in s.fields.iter().filter(|f| !f.is_hidden(db)) {
                collect_containers(
                    db,
                    &format!("{path}.{}", f.name_with_case.text(db)),
                    addr + f.offset,
                    &f.ty,
                    global,
                    out,
                );
            }
        }
        // Only an array whose element can be a `this` is descended.
        MirType::Array(a) if matches!(&*a.element_type, MirType::Struct(_)) => {
            for k in 0..a.total_elements {
                collect_containers(
                    db,
                    &debug_format::element_path(path, k, &a.dimensions),
                    addr + k * a.element_size,
                    &a.element_type,
                    global,
                    out,
                );
            }
        }
        _ => {}
    }
}

/// A leaf symbol's dotted path from a root segment and a field name.
pub fn join_path(db: &dyn WorkspaceDataBase, root: &str, field: Ident) -> String {
    format!("{root}.{}", field.text(db))
}
