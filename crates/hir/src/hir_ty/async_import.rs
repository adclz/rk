// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! An `{extern async 'module' 'name'}` FUNCTION_BLOCK: an async import, a
//! call its instance holds from one scan to the next, never waiting for it.
//! The declaration says it is async: rk reads WIT it does not know yet only
//! at the build, which checks it against WASI's.
//!
//! Each call of the block takes its call one step: it starts the call when
//! none is running, and otherwise checks it. Its VAR_OUTPUT `DONE : BOOL` is
//! TRUE in the call of the block that saw it finish, when the other
//! VAR_OUTPUTs, the fields of its result, are written. Its VAR_INPUTs are
//! the call's parameters, read when the call starts. A block in ST that
//! calls it gives it a REQ or a BUSY where its users need one.

use db::WorkspaceDataBase;

use crate::HasPragmas;
use crate::hir_def::expressions::spec::ElementarySpec;
use crate::hir_def::pous::function_block::FunctionBlock;
use crate::hir_def::pous::variable::{VariableDecl, VariableKind};
use crate::hir_ty::infer::Infer;
use crate::hir_ty::ty::Type;

/// The output an async import says its call finished with, as written.
pub const DONE: &str = "DONE";

/// Whether `var` is named `DONE`, whatever its section and type: names are
/// caseless.
pub fn named_done(db: &dyn WorkspaceDataBase, var: VariableDecl<'_>) -> bool {
    var.name(db).text(db).as_str() == DONE.to_lowercase()
}

/// Whether `var` is `DONE` as an async import or a `{test}` FUNCTION_BLOCK
/// declares it: a VAR_OUTPUT of type BOOL.
pub fn is_done<'db>(db: &'db dyn WorkspaceDataBase, var: VariableDecl<'db>) -> bool {
    named_done(db, var)
        && var.kind(db) == VariableKind::Output
        && matches!(
            var.spec(db).infer(db).normalize(db),
            Type::Elementary(ElementarySpec::Bool)
        )
}

/// `fb`'s `DONE`, when it declares one ([`is_done`]): an async import's, or
/// a `{test}` FUNCTION_BLOCK's, which the runner reads after each cycle.
pub fn done_output<'db>(
    db: &'db dyn WorkspaceDataBase,
    fb: FunctionBlock<'db>,
) -> Option<VariableDecl<'db>> {
    fb.variables(db)
        .iter()
        .copied()
        .find(|var| is_done(db, *var))
}

/// What an async import's call is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum AsyncKind {
    /// A call of the host's `async func`: a subtask, whose result is
    /// written into the block's instance.
    Call,
    /// The component model's `stream.write`, imported as
    /// `[stream-write-N]f` beside a function `f` whose type has the stream:
    /// a copy of bytes into the stream's writing end, which ends when the
    /// reader has taken some or left. It takes the end and the bytes, a
    /// STRING, unchecked, and gives how many went and whether the reader
    /// left.
    StreamWrite,
    /// The component model's `stream.read`, imported as `[stream-read-N]f`:
    /// a copy of bytes from the stream's reading end into the block's
    /// buffer, a STRING or an ARRAY OF BYTE, which ends when the writer has
    /// given some or left. It takes the end and gives the bytes, how many
    /// came and whether the writer left.
    StreamRead,
    /// The component model's `future.read`, imported as `[future-read-N]f`:
    /// the one value the future's writer gives, written into the block's
    /// instance as an async call's result is. It takes the future's reading
    /// end and gives the value's fields.
    FutureRead,
}

impl AsyncKind {
    /// The kind of the import an `{extern async}` names `name`.
    pub fn of(name: &str) -> AsyncKind {
        match crate::hir_def::pous::pragma::builtin_name(name) {
            Some(("stream-write", _)) => AsyncKind::StreamWrite,
            Some(("stream-read", _)) => AsyncKind::StreamRead,
            Some(("future-read", _)) => AsyncKind::FutureRead,
            _ => AsyncKind::Call,
        }
    }

    /// Whether the call is a copy into or out of a stream or a future,
    /// checked on the end it copies on rather than on a subtask.
    pub fn copies(self) -> bool {
        self != AsyncKind::Call
    }
}

/// An async import's variables, each in its part.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct AsyncImport<'db> {
    pub kind: AsyncKind,
    pub done: VariableDecl<'db>,
    /// The call's parameters: the VAR_INPUTs, in declaration order.
    pub params: Vec<VariableDecl<'db>>,
    /// The fields of its result: the other VAR_OUTPUTs, in declaration
    /// order.
    pub results: Vec<VariableDecl<'db>>,
}

/// Whether `fb` is an async import: an `{extern async}` FUNCTION_BLOCK.
pub fn is_async_import(db: &dyn WorkspaceDataBase, fb: FunctionBlock<'_>) -> bool {
    fb.extern_pragma(db)
        .is_some_and(|(_, pragma)| pragma.asynchronous)
}

/// `fb`'s call, when it is an async import that declares its `DONE`;
/// `None` otherwise, a fault the check of its declaration reports (E1514).
pub fn async_import<'db>(
    db: &'db dyn WorkspaceDataBase,
    fb: FunctionBlock<'db>,
) -> Option<AsyncImport<'db>> {
    let (_, pragma) = fb
        .extern_pragma(db)
        .filter(|(_, pragma)| pragma.asynchronous)?;
    let kind = AsyncKind::of(&pragma.name);
    let variables = fb.variables(db);
    let done = done_output(db, fb)?;
    let of_kind = |kind: VariableKind| {
        variables
            .iter()
            .copied()
            .filter(|var| var.kind(db) == kind && *var != done)
            .collect::<Vec<_>>()
    };
    Some(AsyncImport {
        kind,
        done,
        params: of_kind(VariableKind::Input),
        results: of_kind(VariableKind::Output),
    })
}

/// Whether `import`, a stream write, declares the built-in's variables:
/// the stream's writing end, a UDINT, then the bytes, a STRING, and after
/// DONE how many went, a UDINT, then whether the reader left, a BOOL.
pub fn stream_write_shaped(db: &dyn WorkspaceDataBase, import: &AsyncImport<'_>) -> bool {
    let is = |var: &VariableDecl<'_>, spec: ElementarySpec| {
        var.spec(db).infer(db).normalize(db) == Type::Elementary(spec)
    };
    matches!(
        import.params.as_slice(),
        [end, data] if is(end, ElementarySpec::UDInt) && is(data, ElementarySpec::String)
    ) && matches!(
        import.results.as_slice(),
        [written, closed] if is(written, ElementarySpec::UDInt) && is(closed, ElementarySpec::Bool)
    )
}

/// Whether `import`, a stream read, declares the built-in's variables: the
/// stream's reading end, a UDINT, and after DONE the bytes, a STRING or a
/// one-dimension ARRAY OF BYTE, how many came, a UDINT, then whether the
/// writer left, a BOOL.
pub fn stream_read_shaped(db: &dyn WorkspaceDataBase, import: &AsyncImport<'_>) -> bool {
    let is = |var: &VariableDecl<'_>, spec: ElementarySpec| {
        var.spec(db).infer(db).normalize(db) == Type::Elementary(spec)
    };
    matches!(import.params.as_slice(), [end] if is(end, ElementarySpec::UDInt))
        && matches!(
            import.results.as_slice(),
            [data, read, closed] if read_buffer(db, *data).is_some()
                && is(read, ElementarySpec::UDInt)
                && is(closed, ElementarySpec::Bool)
        )
}

/// What a stream read's bytes land in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadBuffer {
    /// A STRING, whose length becomes how many came.
    String,
    /// A one-dimension ARRAY OF BYTE, a byte per element.
    Bytes,
}

/// What `var` takes a stream read's bytes in, when it can.
pub fn read_buffer(db: &dyn WorkspaceDataBase, var: VariableDecl<'_>) -> Option<ReadBuffer> {
    match var.spec(db).infer(db).normalize(db) {
        Type::Elementary(ElementarySpec::String) => Some(ReadBuffer::String),
        Type::Array(array)
            if array.subranges(db).len() == 1
                && array.of_type(db).infer(db).normalize(db)
                    == Type::Elementary(ElementarySpec::Byte) =>
        {
            Some(ReadBuffer::Bytes)
        }
        _ => None,
    }
}

/// Whether `import`, a future read, takes the built-in's one parameter: the
/// future's reading end, a UDINT. Its value's fields are the VAR_OUTPUTs
/// after DONE, as an async call's result's are.
pub fn future_read_shaped(db: &dyn WorkspaceDataBase, import: &AsyncImport<'_>) -> bool {
    matches!(
        import.params.as_slice(),
        [end] if end.spec(db).infer(db).normalize(db) == Type::Elementary(ElementarySpec::UDInt)
    )
}

/// How many values the call's parameters flatten to, a STRING counting
/// two: past [`MAX_FLAT_ASYNC_PARAMS`], the call passes them in memory.
pub fn flat_params(db: &dyn WorkspaceDataBase, import: &AsyncImport<'_>) -> usize {
    import
        .params
        .iter()
        .map(|var| match var.spec(db).infer(db).normalize(db) {
            Type::Elementary(ElementarySpec::String) => 2,
            _ => 1,
        })
        .sum()
}

/// The values an async call passes as parameters, at most: past them, the
/// canonical ABI passes the address of a tuple of them.
pub const MAX_FLAT_ASYNC_PARAMS: usize = 4;

/// Whether a value of `ty` holds an async import's instance: is one, an
/// array of them, or an instance with one among its members, at any depth.
pub fn holds_async_import<'db>(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> bool {
    holds(db, ty, &mut rustc_hash::FxHashSet::default())
}

fn holds<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: Type<'db>,
    seen: &mut rustc_hash::FxHashSet<crate::hir_def::pous::pou::Pou<'db>>,
) -> bool {
    use crate::hir_def::pous::pou::Pou;
    let pou = match ty.element_under_arrays(db).normalize(db) {
        Type::FunctionBlock(fb) if is_async_import(db, fb) => return true,
        Type::FunctionBlock(fb) => Pou::FunctionBlock(fb),
        Type::Class(class) => Pou::Class(class),
        _ => return false,
    };
    if !seen.insert(pou) {
        return false;
    }
    // A VAR_IN_OUT member holds an address, a VAR_TEMP one call's value, and
    // a VAR_EXTERNAL a global's.
    crate::hir_ty::oop::instance_members(db, pou)
        .iter()
        .filter(|member| {
            !member.var.is_in_out(db)
                && member.var.kind(db) != VariableKind::Temp
                && member.var.kind(db) != VariableKind::External
        })
        .any(|member| holds(db, member.var.spec(db).infer(db), seen))
}
