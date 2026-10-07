// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Where values live in linear memory: the size, the alignment and the
//! offsets of every type, computed here and nowhere else. MIR builds its
//! types from these.
//!
//! Sizes are `u64`: storage past what a module addresses is refused at the
//! declaration (E0322) instead of wrapping in a 32-bit sum, and MIR narrows
//! them once the check has passed.
//!
//! The rules are the wasm module's: every value up to 32 bits takes a
//! 32-bit lane, BOOL and SINT included; a 64-bit one takes 8 bytes; a
//! STRING is a 4-byte length then its capacity in bytes; an address is 4
//! bytes. A STRUCT or an instance lays its parts out in order, each at its
//! own alignment, and rounds its size up to the largest.

use db::WorkspaceDataBase;
use rustc_hash::FxHashSet;

use crate::hir_def::{
    expressions::spec::{Array, ElementarySpec, Spec, SpecKind, Struct},
    interned::identifier::Ident,
    pous::{pou::Pou, variable::VariableDecl},
    program::ProgramDecl,
    scope::ScopeId,
};
use crate::hir_ty::{infer::Infer, ty::Type};

/// The size and the alignment of a value, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub struct Layout {
    pub size: u64,
    pub align: u32,
}

impl Layout {
    /// Whether a module can address it: its size fits the 32 bits every
    /// address and offset is computed in.
    pub fn fits(self) -> bool {
        self.size <= u64::from(u32::MAX)
    }
}

/// An address: a REF_TO, a VAR_IN_OUT, a member declared `AT %I*`.
pub const POINTER: Layout = Layout { size: 4, align: 4 };

/// What [`Layout::fits`] allows, for a message.
pub const ADDRESS_SPACE: u64 = 1 << 32;

/// An elementary value: a 32-bit lane, or 8 bytes for a 64-bit type. A
/// STRING is laid out by [`string`].
pub fn elementary(spec: ElementarySpec) -> Layout {
    let size = match spec {
        ElementarySpec::LInt
        | ElementarySpec::ULInt
        | ElementarySpec::LWord
        | ElementarySpec::LReal
        | ElementarySpec::LTime
        | ElementarySpec::LDate
        | ElementarySpec::LTod
        | ElementarySpec::DateAndTime
        | ElementarySpec::LDateTime => 8,
        ElementarySpec::String => {
            return string(u64::from(
                crate::hir_ty::infer::normalize::DEFAULT_STRING_CAPACITY,
            ));
        }
        _ => 4,
    };
    Layout {
        size,
        align: size as u32,
    }
}

/// A STRING of `capacity` bytes: its 4-byte length, then the bytes.
pub fn string(capacity: u64) -> Layout {
    Layout {
        size: capacity.saturating_add(4),
        align: 4,
    }
}

/// What a declaration's spec takes: a variable, a field, an element, a
/// TYPE. `None` when it has no layout of its own (an `ARRAY[*]`, whose
/// bounds each call gives) or one that does not resolve, which is reported
/// where it is written.
pub fn of_spec<'db>(db: &'db dyn WorkspaceDataBase, spec: Spec<'db>) -> Option<Layout> {
    spec_layout(db, spec, &mut Vec::new())
}

/// What a value of `ty` takes. A STRING the type no longer says the length
/// of is at the default capacity: prefer [`of_spec`] for a declaration.
pub fn of_type<'db>(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Option<Layout> {
    type_layout(db, ty, &mut Vec::new())
}

/// The elements of a STRUCT, in declaration order.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct StructLayout {
    /// Each element's offset from the start of the STRUCT.
    pub offsets: Vec<u64>,
    pub whole: Layout,
}

#[salsa::tracked(returns(ref))]
pub fn struct_layout<'db>(
    db: &'db dyn WorkspaceDataBase,
    strukt: Struct<'db>,
) -> Option<StructLayout> {
    struct_parts(db, strukt, &mut Vec::new())
}

/// An array: its element, repeated over its dimensions, row by row.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ArrayLayout {
    pub element: Layout,
    /// `(lower, upper)` of each dimension.
    pub dimensions: Vec<(i64, i64)>,
    /// How many elements, all dimensions together.
    pub count: u64,
    pub whole: Layout,
}

impl ArrayLayout {
    /// The rows after the first `through` dimensions: what `m[i]` names,
    /// laid out where it starts.
    pub fn row(&self, through: usize) -> Option<ArrayLayout> {
        let dimensions = self.dimensions.get(through..)?.to_vec();
        let count = dimensions
            .iter()
            .map(|(lower, upper)| {
                u64::try_from(i128::from(*upper) - i128::from(*lower) + 1).unwrap_or(0)
            })
            .fold(1u64, u64::saturating_mul);
        Some(ArrayLayout {
            element: self.element,
            dimensions,
            count,
            whole: Layout {
                size: self.element.size.saturating_mul(count),
                align: self.element.align,
            },
        })
    }
}

#[salsa::tracked(returns(ref))]
pub fn array_layout<'db>(db: &'db dyn WorkspaceDataBase, array: Array<'db>) -> Option<ArrayLayout> {
    array_parts(db, array, &mut Vec::new())
}

/// The parts of an instance: an FB's, a CLASS's or a PROGRAM's.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct InstanceLayout<'db> {
    pub fields: Vec<Field<'db>>,
    pub whole: Layout,
}

/// One part of an instance, at its offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub struct Field<'db> {
    pub var: VariableDecl<'db>,
    pub part: Part,
    pub offset: u64,
    pub layout: Layout,
}

/// What a [`Field`] holds of its variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum Part {
    /// The variable's own storage.
    Value,
    /// The address of what it is bound to: a VAR_IN_OUT, whose call binds
    /// it, or a member declared `AT %I*`, which VAR_CONFIG places.
    Address,
    /// The capacity of the buffer a STRING VAR_IN_OUT is bound to, right
    /// after its address: the body writes at it.
    Capacity,
    /// An edge input's edge, which the block's own code reads under the
    /// input's name.
    Edge,
    /// What an edge input's edge is computed against at the next call.
    EdgeMemory,
}

/// An FB's or a CLASS's instance: its own members after its bases', so a
/// derived instance is laid out as its base where they overlap.
#[salsa::tracked(returns(ref))]
pub fn instance_layout<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
) -> Option<InstanceLayout<'db>> {
    instance_parts(db, pou, &mut Vec::new())
}

/// A PROGRAM's instance: its variables but the ones that live elsewhere (a
/// VAR_EXTERNAL is the global's, a VAR_TEMP is one call's, a located VAR is
/// its channel's).
#[salsa::tracked(returns(ref))]
pub fn program_layout<'db>(
    db: &'db dyn WorkspaceDataBase,
    program: ProgramDecl<'db>,
) -> Option<InstanceLayout<'db>> {
    let vars = program
        .variables(db)
        .iter()
        .filter(|var| !var.is_external(db) && !var.is_temp(db) && !var.is_program_located(db));
    fields_of(db, vars.copied(), Binding::Scheduled, &mut Vec::new())
}

/// The variables whose address `scope` takes, in its statements and in its
/// locals' initializers, which linear memory holds even when they are
/// scalars: the root of every `REF()`, VAR_IN_OUT argument and output
/// destination, wherever it stands (a subscript, an assignment target, a
/// nested call, an initializer). Read off inference, which visited every
/// expression and bound every call.
#[salsa::tracked(returns(ref))]
pub fn address_taken<'db>(db: &'db dyn WorkspaceDataBase, scope: ScopeId<'db>) -> FxHashSet<Ident> {
    use crate::hir_def::expressions::expression::{
        BeginPathExpr, ExprKind, PrimaryExpr, RefValue, VariableAccessKind,
    };
    use crate::hir_ty::body::ParamBinding;

    fn root<'db>(db: &'db dyn WorkspaceDataBase, path: &BeginPathExpr<'db>) -> Option<Ident> {
        let root = path.expr(db)?.flatten(db).first()?.get_expr(db);
        Some(root.ident(db).ident(db))
    }

    let inference = scope.inference(db);
    let mut result = FxHashSet::default();
    for (expr, _) in inference.typed_exprs() {
        if let ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
            value: RefValue::Address(path),
        }) = expr.expr(db)
        {
            result.extend(root(db, path));
        }
    }
    for (_, call) in inference.resolved_calls() {
        for (var, binding) in &call.params {
            let access = match binding {
                ParamBinding::Values(values) if var.is_in_out(db) => values
                    .iter()
                    .filter_map(|value| value.variable_access(db))
                    .collect(),
                ParamBinding::Output { variable, .. } => vec![*variable],
                _ => continue,
            };
            for access in access {
                if let VariableAccessKind::Symbolic(path) = access.kind(db) {
                    result.extend(root(db, &path));
                }
            }
        }
    }
    result
}

/// An aggregate being laid out, innermost last: one met again contains
/// itself, which E1302 refuses, and has no layout.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Open<'db> {
    Struct(Struct<'db>),
    Array(Array<'db>),
    Instance(Pou<'db>),
}

fn spec_layout<'db>(
    db: &'db dyn WorkspaceDataBase,
    spec: Spec<'db>,
    open: &mut Vec<Open<'db>>,
) -> Option<Layout> {
    // A STRING's length is its declaration's, which its type forgets.
    if matches!(
        spec.infer(db).normalize(db),
        Type::Elementary(ElementarySpec::String)
    ) {
        let capacity = crate::hir_ty::infer::normalize::declared_string_length(db, spec).unwrap_or(
            u64::from(crate::hir_ty::infer::normalize::DEFAULT_STRING_CAPACITY),
        );
        return Some(string(capacity));
    }
    match spec.kind(db) {
        SpecKind::Struct(strukt) => struct_parts(db, *strukt, open).map(|s| s.whole),
        SpecKind::Array(array) => array_parts(db, *array, open).map(|a| a.whole),
        SpecKind::ArrayConformand(_) => None,
        _ => type_layout(db, spec.infer(db), open),
    }
}

fn type_layout<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: Type<'db>,
    open: &mut Vec<Open<'db>>,
) -> Option<Layout> {
    // A named type keeps its declaration's STRING length.
    if let Type::DataType(dt) = ty {
        return spec_layout(db, dt.spec(db), open);
    }
    match ty.normalize(db) {
        Type::Elementary(spec) => Some(elementary(spec)),
        Type::RefTo(_) | Type::Null => Some(POINTER),
        Type::Struct(strukt) => struct_parts(db, strukt, open).map(|s| s.whole),
        Type::Array(array) => array_parts(db, array, open).map(|a| a.whole),
        Type::Enum(e) => match e.typ(db) {
            Some(base) => type_layout(db, base.infer(db), open),
            None => Some(elementary(ElementarySpec::DInt)),
        },
        Type::SubRange(range) => type_layout(db, range._type(db).infer(db), open),
        Type::FunctionBlock(fb) => {
            instance_parts(db, Pou::FunctionBlock(fb), open).map(|i| i.whole)
        }
        Type::Class(class) => instance_parts(db, Pou::Class(class), open).map(|i| i.whole),
        _ => None,
    }
}

/// Run `lay_out` on `aggregate`, unless it is already being laid out.
fn enter<'db, T>(
    open: &mut Vec<Open<'db>>,
    aggregate: Open<'db>,
    lay_out: impl FnOnce(&mut Vec<Open<'db>>) -> Option<T>,
) -> Option<T> {
    if open.contains(&aggregate) {
        return None;
    }
    open.push(aggregate);
    let laid_out = lay_out(open);
    open.pop();
    laid_out
}

fn struct_parts<'db>(
    db: &'db dyn WorkspaceDataBase,
    strukt: Struct<'db>,
    open: &mut Vec<Open<'db>>,
) -> Option<StructLayout> {
    enter(open, Open::Struct(strukt), |open| {
        let mut cursor = Cursor::default();
        let mut offsets = Vec::new();
        for element in strukt.elements(db) {
            offsets.push(cursor.place(spec_layout(db, element.spec(db), open)?));
        }
        Some(StructLayout {
            offsets,
            whole: cursor.close(),
        })
    })
}

fn array_parts<'db>(
    db: &'db dyn WorkspaceDataBase,
    array: Array<'db>,
    open: &mut Vec<Open<'db>>,
) -> Option<ArrayLayout> {
    enter(open, Open::Array(array), |open| {
        let element = spec_layout(db, array.of_type(db), open)?;
        let mut dimensions = Vec::new();
        let mut count = 1u64;
        // The dimensions inference folded; one that does not fold is E0501
        // or E0502.
        for (lower, upper) in crate::hir_ty::infer::const_eval::array_dimensions(db, array) {
            let (lower, upper) = (lower?, upper?);
            dimensions.push((lower, upper));
            let length = u64::try_from(i128::from(upper) - i128::from(lower) + 1).unwrap_or(0);
            count = count.saturating_mul(length);
        }
        Some(ArrayLayout {
            element,
            dimensions,
            count,
            whole: Layout {
                size: element.size.saturating_mul(count),
                align: element.align,
            },
        })
    })
}

fn instance_parts<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
    open: &mut Vec<Open<'db>>,
) -> Option<InstanceLayout<'db>> {
    enter(open, Open::Instance(pou), |open| {
        let members = crate::hir_ty::oop::instance_members(db, pou);
        fields_of(
            db,
            members.iter().map(|member| member.var),
            Binding::Called,
            open,
        )
    })
}

/// How an instance's VAR_IN_OUT is bound.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Binding {
    /// By the call of a block: the member holds the address.
    Called,
    /// A PROGRAM's, which the scheduler runs and no call binds: the member
    /// holds the value.
    Scheduled,
}

/// Lay out `vars` one after the other, each with the parts it takes.
fn fields_of<'db>(
    db: &'db dyn WorkspaceDataBase,
    vars: impl Iterator<Item = VariableDecl<'db>>,
    binding: Binding,
    open: &mut Vec<Open<'db>>,
) -> Option<InstanceLayout<'db>> {
    let mut cursor = Cursor::default();
    let mut fields = Vec::new();
    let mut push = |cursor: &mut Cursor, var, part, layout| {
        fields.push(Field {
            var,
            part,
            offset: cursor.place(layout),
            layout,
        });
    };
    for var in vars {
        // A VAR_IN_OUT holds the address of what the call binds, and a
        // member declared `AT %I*` the address of its channel. Neither is
        // laid out by what it points at, so a block that two VAR_IN_OUTs
        // hold through each other still has a layout.
        let in_out = var.is_in_out(db) && binding == Binding::Called;
        if in_out || var.is_partly_located(db) {
            push(&mut cursor, var, Part::Address, POINTER);
            // A STRING VAR_IN_OUT also keeps the bound buffer's capacity:
            // a block declaring `s : STRING` wrote 80 bytes into a caller's
            // `STRING[4]`.
            let string = matches!(
                var.spec(db).infer(db).normalize(db),
                Type::Elementary(ElementarySpec::String)
            );
            if in_out && string {
                push(
                    &mut cursor,
                    var,
                    Part::Capacity,
                    elementary(ElementarySpec::UDInt),
                );
            }
        } else {
            push(
                &mut cursor,
                var,
                Part::Value,
                spec_layout(db, var.spec(db), open)?,
            );
        }
        if var.is_edge_input(db) {
            let bool = elementary(ElementarySpec::Bool);
            push(&mut cursor, var, Part::Edge, bool);
            push(&mut cursor, var, Part::EdgeMemory, bool);
        }
    }
    Some(InstanceLayout {
        fields,
        whole: cursor.close(),
    })
}

/// Places parts one after the other.
#[derive(Default)]
struct Cursor {
    offset: u64,
    align: u32,
}

impl Cursor {
    /// Where a part of `layout` goes: past the last one, at its alignment.
    fn place(&mut self, layout: Layout) -> u64 {
        let offset = align_to(self.offset, layout.align);
        self.offset = offset.saturating_add(layout.size);
        self.align = self.align.max(layout.align);
        offset
    }

    /// The whole, its size rounded up to its largest alignment.
    fn close(self) -> Layout {
        let align = self.align.max(1);
        Layout {
            size: align_to(self.offset, align),
            align,
        }
    }
}

/// `offset` rounded up to a multiple of `align`.
pub fn align_to(offset: u64, align: u32) -> u64 {
    let align = u64::from(align.max(1));
    offset.div_ceil(align).saturating_mul(align)
}
