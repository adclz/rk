//! What a task runs for a program instance with connections: `PROGRAM P1
//! WITH T : F(x1 := %IX1.1, y1 => total)` feeds `x1` before each scan of the
//! instance and copies `y1` out after it.

use std::{cell::RefCell, rc::Rc};

use db::WorkspaceDataBase;
use hir::{
    hir_def::{interned::identifier::Ident, pous::variable::LocatedAddress},
    hir_ty::{
        config::{ConnectionEnd, ResolvedProgram},
        ty::Type,
    },
};

use crate::{
    expr::{MirExpr, MirPlace},
    function::{MirFunction, MirLinkage, MirParam, MirParamKind},
    lower::{
        lower_expr::{ExprLowerCtx, StringPool},
        lower_type::LowerTypeError,
        multibit::View,
    },
    schedule::ProgramInfo,
    stmt::MirStmt,
    types::{MirStructField, MirType},
};

/// The function a task calls for a program instance with connections.
pub fn scan_fn_name(db: &dyn WorkspaceDataBase, instance: Ident) -> Ident {
    Ident::new(
        db,
        compact_str::CompactString::from(format!("{}$__scan__", instance.text(db))),
    )
}

/// `P1$__scan__(this)`: each input written from its source, the program's
/// body run on the instance, each output copied to its sink. The task calls
/// it in place of the body.
pub(crate) fn lower_connections<'db>(
    db: &'db dyn WorkspaceDataBase,
    p: &ResolvedProgram<'db>,
    info: &ProgramInfo<'db>,
    index: u32,
    string_pool: &Rc<RefCell<StringPool>>,
) -> Result<MirFunction, LowerTypeError> {
    let ctx = ExprLowerCtx::new(db, string_pool.clone());
    let field = |var: &hir::hir_def::pous::variable::VariableDecl<'db>| {
        info.struct_type
            .fields
            .iter()
            .find(|f| f.name == var.name(db))
            .ok_or_else(|| {
                LowerTypeError::UnsupportedType(format!(
                    "'{}' has no field in the program's layout",
                    var.name(db).text(db)
                ))
            })
    };

    let mut input_writes = Vec::new();
    for (var, source) in &p.connections.inputs {
        let field = field(var)?;
        let value = match source {
            ConnectionEnd::Constant(expr) => {
                let value = ctx.lower_expr(*expr)?;
                // The input's lane wins, as it does for an initializer.
                match (&field.ty, ctx.expr_to_mir_elementary(*expr)) {
                    (MirType::Elementary(to), Ok(from)) if from != *to => MirExpr::Cast {
                        expr: Box::new(value),
                        from,
                        to: *to,
                    },
                    _ => value,
                }
            }
            end => match end_storage(db, &ctx, end, field)? {
                Storage::Place(place) => read(place, &field.ty),
                Storage::Slice(view) => view.read(),
            },
        };
        input_writes.push((field.offset, value, field.ty.clone()));
    }

    let mut output_reads = Vec::new();
    let mut after = Vec::new();
    for (var, sink) in &p.connections.outputs {
        let field = field(var)?;
        match end_storage(db, &ctx, sink, field)? {
            Storage::Place(place) => {
                output_reads.push((field.offset, place, field.ty.clone(), None))
            }
            // Bits of a wider cell: the whole cell is written back with them
            // replaced.
            Storage::Slice(view) => {
                let (target, value) = view.write(MirExpr::Load(
                    MirPlace::ThisField {
                        field_name: field.name,
                        field_offset: field.offset,
                        field_type: field.ty.clone(),
                    },
                    field.ty.clone(),
                ));
                after.push(MirStmt::Assign { target, value });
            }
        }
    }

    let program = MirType::Struct(info.struct_type.clone());
    let mut body = vec![MirStmt::FbCall {
        // The instance is the wrapper's own `this`.
        instance: MirPlace::ThisField {
            field_name: p.instance_name,
            field_offset: 0,
            field_type: program.clone(),
        },
        body_func: info.body_fn,
        body_func_index: 0, // resolved during module lowering
        input_writes,
        output_reads,
    }];
    body.append(&mut after);

    Ok(MirFunction {
        name: scan_fn_name(db, p.instance_name),
        origin_name: p.instance_name,
        index,
        params: vec![MirParam {
            name: Ident::new(db, compact_str::CompactString::from("this")),
            ty: MirType::Pointer(Box::new(program)),
            kind: MirParamKind::This,
        }],
        return_type: None,
        locals: Vec::new(),
        body,
        // The schedule names it in place of the body.
        linkage: MirLinkage::Export,
        is_test: false,
        export_name: None,
    })
}

/// Where a connection's other end is kept.
enum Storage {
    /// Memory of its own, read and written as the field's type.
    Place(MirPlace),
    /// Bits of a wider cell.
    Slice(View),
}

fn end_storage<'db>(
    db: &'db dyn WorkspaceDataBase,
    ctx: &ExprLowerCtx<'db>,
    end: &ConnectionEnd<'db>,
    field: &MirStructField,
) -> Result<Storage, LowerTypeError> {
    let view = match end {
        ConnectionEnd::Constant(_) => {
            return Err(LowerTypeError::UnsupportedType(
                "a constant connected as a sink".to_string(),
            ));
        }
        ConnectionEnd::Global(global) => ctx.view_of_variable(Type::Variable((*global, None)))?,
        ConnectionEnd::Address(address) => ctx.view_of_address(address.clone())?,
    };
    Ok(match view {
        // Whole bytes of a wider cell, read as the field's type.
        Some(View::Bytes(MirPlace::Field {
            base,
            field_name,
            field_offset,
            ..
        })) => Storage::Place(MirPlace::Field {
            base,
            field_name,
            field_offset,
            field_type: field.ty.clone(),
        }),
        Some(view) => Storage::Slice(view),
        None => Storage::Place(match end {
            ConnectionEnd::Global(global) => MirPlace::Global {
                name: Some(super::lower_module::global_key(db, *global)),
                address: 0,
                ty: field.ty.clone(),
            },
            ConnectionEnd::Address(address) => cell(db, address, &field.ty),
            ConnectionEnd::Constant(_) => unreachable!("refused above"),
        }),
    })
}

/// The cell of an address, read as `ty`: HIR accepted a variable as wide as
/// it, which a REAL in a `%ID` is.
fn cell(db: &dyn WorkspaceDataBase, address: &LocatedAddress, ty: &MirType) -> MirPlace {
    let name = Ident::new(db, address.text.clone());
    MirPlace::Field {
        base: Box::new(MirPlace::Global {
            name: Some(name),
            address: 0,
            ty: MirType::Elementary(crate::located::width_elementary(address.width)),
        }),
        field_name: name,
        field_offset: 0,
        field_type: ty.clone(),
    }
}

/// What a field is written from: the value, or an aggregate's address.
fn read(place: MirPlace, ty: &MirType) -> MirExpr {
    match ty {
        MirType::Struct(_) | MirType::Array(_) => MirExpr::AddrOf(place),
        _ => MirExpr::Load(place, ty.clone()),
    }
}
