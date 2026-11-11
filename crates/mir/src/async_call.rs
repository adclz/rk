// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! An async import's call, as its function block makes it
//! ([`hir::hir_ty::async_import`]): the function, imported async-lowered,
//! so that a call returns at once; the component model's built-ins the
//! block checks the call with; and where each part of the call is in the
//! block's instance, which codegen reads to emit the block's body.

use compact_str::CompactString;
use db::WorkspaceDataBase;
use hir::hir_def::interned::identifier::Ident;
use hir::hir_def::pous::function_block::FunctionBlock;
use hir::hir_def::pous::pou::Pou;
use hir::hir_def::pous::pragma::ExternPragma;
use hir::hir_ty::async_import::AsyncImport;
use hir::hir_ty::layout::Part;

use crate::MirModule;
use crate::function::{MirExternFunction, MirLinkage, MirParam, MirParamKind};
use crate::lower::lower_type::LowerTypeError;
use crate::types::{MirElementary, MirStructType, MirType};

/// What a block's body does with its call: where each part of it is in
/// the block's instance.
#[derive(Debug, Clone)]
pub struct MirAsyncCall {
    /// The import, async-lowered, by its symbol.
    pub import: Ident,
    /// Where DONE is.
    pub done: u32,
    /// Each parameter: where it is, and its type.
    pub params: Vec<(u32, MirType)>,
    /// Each field of the result: where its VAR_OUTPUT is, and its type.
    pub results: Vec<(u32, MirType)>,
    /// Where the instance keeps what it knows of its call
    /// ([`hir::hir_ty::layout::ASYNC_STATE`]).
    pub state: u32,
    /// Where the host writes the result, when the call has one.
    pub result_area: Option<u32>,
    /// Where the parameters are laid out in memory, when they take more than
    /// [`hir::hir_ty::async_import::MAX_FLAT_ASYNC_PARAMS`] values.
    pub args_area: Option<u32>,
}

/// What the state holds, at these offsets: the call's handle, 0 when none
/// is running, the waitable set it is checked in, 0 until one is made, and
/// the two words of the set's last event.
pub const STATE_HANDLE: u32 = 0;
pub const STATE_SET: u32 = 4;
pub const STATE_EVENT: u32 = 8;

/// The component model's built-ins a block checks its call with, imported
/// from `$root`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    /// `waitable-set.new`, `() -> i32`: a set to check calls in.
    SetNew,
    /// `waitable-set.poll`, `(set, ptr) -> i32`: the set's next event, or 0
    /// when there is none, its two words written at `ptr`. It never waits.
    SetPoll,
    /// `waitable.join`, `(waitable, set)`: a call to check in the set.
    Join,
    /// `subtask.drop`, `(subtask)`: a finished call let go.
    SubtaskDrop,
}

impl Builtin {
    pub const ALL: [Builtin; 4] = [
        Builtin::SetNew,
        Builtin::SetPoll,
        Builtin::Join,
        Builtin::SubtaskDrop,
    ];

    /// Its name among the imports of `$root`.
    pub fn name(self) -> &'static str {
        match self {
            Builtin::SetNew => "[waitable-set-new]",
            Builtin::SetPoll => "[waitable-set-poll]",
            Builtin::Join => "[waitable-join]",
            Builtin::SubtaskDrop => "[subtask-drop]",
        }
    }

    fn params(self) -> usize {
        match self {
            Builtin::SetNew => 0,
            Builtin::SubtaskDrop => 1,
            Builtin::SetPoll | Builtin::Join => 2,
        }
    }

    fn returns(self) -> bool {
        matches!(self, Builtin::SetNew | Builtin::SetPoll)
    }
}

/// Where the component model's built-ins are imported from.
pub const ROOT: &str = "$root";

/// The prefix of an import a core module lowers async.
pub const ASYNC_LOWER: &str = "[async-lower]";

/// The symbol `fb`'s import is called by: `Owner$import`.
pub fn import_symbol(db: &dyn WorkspaceDataBase, fb: FunctionBlock<'_>) -> Ident {
    let owner = crate::lower::naming::qualified_pou_ident(
        db,
        hir::hir_ty::ty::Type::new_pou(db, Pou::FunctionBlock(fb)),
    );
    Ident::new(
        db,
        CompactString::from(format!("{}$import", owner.text(db))),
    )
}

/// `fb`'s call, from its instance as MIR lays it out, `instance`.
pub fn call<'db>(
    db: &'db dyn WorkspaceDataBase,
    fb: FunctionBlock<'db>,
    import: &AsyncImport<'db>,
    instance: &MirStructType,
) -> Result<MirAsyncCall, LowerTypeError> {
    let field = |name: Ident| {
        instance
            .fields
            .iter()
            .find(|field| field.name(db) == name)
            .ok_or_else(|| {
                LowerTypeError::UnsupportedType(format!(
                    "the instance of an async import has no part `{}`",
                    name.text(db)
                ))
            })
    };
    let part = |part: Part| {
        field(crate::lower::lower_type::async_field(
            db,
            import.done.name(db),
            part,
        ))
    };
    let typed = |vars: &[hir::hir_def::pous::variable::VariableDecl<'db>]| {
        vars.iter()
            .map(|var| field(var.name(db)).map(|f| (f.offset, f.ty.clone())))
            .collect::<Result<Vec<_>, _>>()
    };
    Ok(MirAsyncCall {
        import: import_symbol(db, fb),
        done: field(import.done.name(db))?.offset,
        params: typed(&import.params)?,
        results: typed(&import.results)?,
        state: part(Part::AsyncState)?.offset,
        result_area: match import.results.is_empty() {
            true => None,
            false => Some(part(Part::AsyncResult)?.offset),
        },
        args_area: part(Part::AsyncArgs).ok().map(|f| f.offset),
    })
}

/// `fb`'s import: its function, lowered async, typed from `call`'s
/// parameters and result.
pub fn import<'db>(
    db: &'db dyn WorkspaceDataBase,
    fb: FunctionBlock<'db>,
    import: &AsyncImport<'db>,
    call: &MirAsyncCall,
    pragma: &ExternPragma,
    index: u32,
) -> MirExternFunction {
    MirExternFunction {
        name: import_symbol(db, fb),
        index,
        module: pragma.module.clone(),
        import_name: CompactString::from(format!("{ASYNC_LOWER}{}", pragma.name)),
        params: import
            .params
            .iter()
            .zip(&call.params)
            .map(|(var, (_, ty))| MirParam {
                name: var.name(db),
                ty: ty.clone(),
                kind: MirParamKind::Input,
            })
            .collect(),
        out_results: import
            .results
            .iter()
            .zip(&call.results)
            .map(|(var, (_, ty))| (var.name(db), ty.clone()))
            .collect(),
        return_type: None,
        linkage: MirLinkage::Internal,
        asynchronous: true,
    }
}

/// The built-ins, imported from `$root`, the first at `index`.
pub fn builtins(db: &dyn WorkspaceDataBase, index: u32) -> Vec<MirExternFunction> {
    let ident = |text: String| Ident::new(db, CompactString::from(text));
    Builtin::ALL
        .into_iter()
        .zip(index..)
        .map(|(builtin, index)| MirExternFunction {
            name: ident(format!("$async{}", builtin.name())),
            index,
            module: CompactString::from(ROOT),
            import_name: CompactString::from(builtin.name()),
            params: (0..builtin.params())
                .map(|at| MirParam {
                    name: ident(format!("p{at}")),
                    ty: MirType::Elementary(MirElementary::DInt),
                    kind: MirParamKind::Input,
                })
                .collect(),
            out_results: Vec::new(),
            return_type: builtin
                .returns()
                .then_some(MirType::Elementary(MirElementary::DInt)),
            linkage: MirLinkage::Internal,
            asynchronous: false,
        })
        .collect()
}

/// Where `module` imports `builtin` among its imports, when it makes an
/// async call.
pub fn builtin_at(module: &MirModule, builtin: Builtin) -> Option<u32> {
    module
        .extern_functions
        .iter()
        .position(|ext| ext.module == ROOT && ext.import_name == builtin.name())
        .map(|at| at as u32)
}

/// Where `module` imports the function an async call `call` makes.
pub fn import_at(module: &MirModule, call: &MirAsyncCall) -> Option<u32> {
    module
        .extern_functions
        .iter()
        .position(|ext| ext.name == call.import)
        .map(|at| at as u32)
}
