// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What the runner calls for a `{test}` FUNCTION_BLOCK: one cycle of the
//! block on an instance of its own, which the test keeps from one call to
//! the next. The runner calls it until a cycle leaves the block's `DONE`
//! TRUE, the test's [`MirFunction::test_done`].

use std::{cell::RefCell, rc::Rc};

use db::WorkspaceDataBase;
use hir::hir_def::{
    interned::identifier::Ident,
    pous::{function_block::FunctionBlock, pou::Pou},
};

use crate::{
    expr::{MirConstant, MirExpr, MirPlace, MirUnaryOp},
    function::{MirFunction, MirLinkage, MirLocal, MirVariableStorage},
    lower::{
        lower_expr::StringPool,
        lower_func::{InitTarget, allocate_local_storage, lower_type_default_inits},
        lower_type::LowerTypeError,
    },
    memory::MirMemoryLayout,
    stmt::MirStmt,
    types::{MirElementary, MirType},
};

/// The symbol of `fb`'s test, `Block$__test__`, which no POU can spell, and
/// its export.
pub fn test_fn_name(db: &dyn WorkspaceDataBase, fb: FunctionBlock<'_>) -> Ident {
    let block = super::naming::qualified_pou_ident(db, hir::hir_ty::ty::Type::FunctionBlock(fb));
    Ident::new(
        db,
        compact_str::CompactString::from(format!("{}$__test__", block.text(db))),
    )
}

/// `Block$__test__()`: the instance's starting values at the first call,
/// then the block's body on it. The instance and the flag that says it has
/// started are in static memory, so each call finds them as the last one
/// left them. `block` is `fb`'s instance type.
pub(crate) fn lower_test_block<'db>(
    db: &'db dyn WorkspaceDataBase,
    fb: FunctionBlock<'db>,
    block: &MirType,
    index: u32,
    memory_layout: &mut MirMemoryLayout,
    string_pool: &Rc<RefCell<StringPool>>,
) -> Result<MirFunction, LowerTypeError> {
    let unsupported = |why: &str| {
        LowerTypeError::UnsupportedType(format!("test '{}' {why}", fb.name_with_case(db).text(db)))
    };
    let MirType::Struct(layout) = block else {
        return Err(unsupported("lowered to a non-struct instance type"));
    };
    // E1517 refuses a test block without its DONE.
    let done = hir::hir_ty::async_import::done_output(db, fb)
        .ok_or_else(|| unsupported("declares no DONE"))?;
    let done = layout
        .fields
        .iter()
        .find(|field| field.name(db) == done.name(db))
        .ok_or_else(|| unsupported("has no DONE in its layout"))?;

    // Names no identifier can spell: the function has no others.
    let instance = Ident::new(db, compact_str::CompactString::from("$instance"));
    let started = Ident::new(db, compact_str::CompactString::from("$started"));
    let flag = MirType::Elementary(MirElementary::Bool);
    let mut no_scalars = 0;
    let mut local = |name: Ident, ty: &MirType, memory_layout: &mut MirMemoryLayout| MirLocal {
        name,
        ty: ty.clone(),
        init: None,
        storage: allocate_local_storage(name, ty, true, &mut no_scalars, memory_layout),
        var_storage: MirVariableStorage::Static,
    };
    let locals = vec![
        local(instance, block, memory_layout),
        local(started, &flag, memory_layout),
    ];

    let mut start = Vec::new();
    lower_type_default_inits(
        db,
        InitTarget::Local {
            name: instance,
            base: 0,
            whole: false,
        },
        block,
        hir::hir_ty::ty::Type::FunctionBlock(fb),
        string_pool,
        &mut start,
    )?;
    start.push(MirStmt::Assign {
        target: MirPlace::Local(started),
        value: MirExpr::Constant(MirConstant::Bool(true)),
    });
    let body = vec![
        MirStmt::If {
            condition: MirExpr::UnaryOp {
                op: MirUnaryOp::Not,
                expr: Box::new(MirExpr::Load(MirPlace::Local(started), flag.clone())),
                ty: MirElementary::Bool,
            },
            then_body: start,
            else_ifs: Vec::new(),
            else_body: None,
        },
        MirStmt::FbCall {
            instance: MirPlace::Local(instance),
            body_func: super::naming::body_symbol(
                db,
                Pou::FunctionBlock(fb),
                Pou::FunctionBlock(fb),
            ),
            body_func_index: 0, // resolved during module lowering
            input_writes: Vec::new(),
            output_reads: Vec::new(),
        },
    ];
    let test_done = MirExpr::Load(
        MirPlace::Field {
            base: Box::new(MirPlace::Local(instance)),
            field_name: done.name_with_case,
            field_offset: done.offset,
            field_type: done.ty.clone(),
        },
        done.ty.clone(),
    );

    // Exported under its symbol, which no POU can take: a block named like
    // one of the module's own exports, `memory`, keeps its name.
    let name = test_fn_name(db, fb);
    Ok(MirFunction {
        name,
        origin_name: fb.name(db),
        index,
        params: Vec::new(),
        return_type: None,
        locals,
        body,
        linkage: MirLinkage::Export,
        is_test: true,
        export_name: Some(name.text(db).clone()),
        wit_export: None,
        frame: None,
        host_entry: true,
        test_done: Some(test_done),
    })
}
