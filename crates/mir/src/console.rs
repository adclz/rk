// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The host's standard output and error, as a module reaches them: WASI
//! 0.3's `wasi:cli/stdout` and `wasi:cli/stderr`, each one function,
//! `write-via-stream`, which takes a stream of bytes.
//!
//! A module that writes makes the stream, hands its reading end to the
//! host, and writes to the other end with the component model's stream
//! built-ins. A core module imports those beside the function they serve,
//! under names no declaration can spell (`[stream-new-0]write-via-stream`),
//! and it keeps the stream for as long as it runs. So no `{extern}`
//! declares them: a `{wasm 'rk.stdout_write'}` writes, lowering adds the
//! imports to a module that has one, and codegen emits the function behind
//! it.

use compact_str::CompactString;
use db::WorkspaceDataBase;
use hir::hir_def::interned::identifier::Ident;

use crate::MirModule;
use crate::function::{MirExternFunction, MirFunction, MirLinkage, MirParam, MirParamKind};
use crate::stmt::MirStmt;
use crate::types::{MirElementary, MirType};

/// One of the host's two outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Stdout,
    Stderr,
}

impl Output {
    pub const ALL: [Output; 2] = [Output::Stdout, Output::Stderr];

    /// The instruction a `{wasm}` pragma writes to it with, one of
    /// [`hir::check::wasm_instructions::CONSOLE`].
    pub fn instruction(self) -> &'static str {
        match self {
            Output::Stdout => "rk.stdout_write",
            Output::Stderr => "rk.stderr_write",
        }
    }

    /// WASI's interface for it, at the release rk carries:
    /// `wasi:cli/stdout@0.3.0`.
    pub fn interface(self) -> String {
        let interface = match self {
            Output::Stdout => "stdout",
            Output::Stderr => "stderr",
        };
        debug_format::wit::interface_of("wasi:cli", interface)
            .expect("rk carries WASI's command line package")
    }
}

/// What a module imports from an output's interface to write to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Import {
    /// `stream.new`: a stream of bytes, `() -> i64`, its reading end in the
    /// low half of the result and its writing end in the high half.
    NewStream,
    /// `write-via-stream`, WASI's function: the reading end handed to the
    /// host, `(i32) -> i32`, which answers with a future of how the writing
    /// went.
    WriteViaStream,
    /// `future.drop-readable`: that future given up unread, `(i32)`.
    DropFuture,
    /// `stream.write`, lowered async: bytes handed to the writing end,
    /// `(end, ptr, len) -> i32`. It returns at once: how many the host took
    /// in the high 28 bits over how the stream is in the low 4 (0 open, 1
    /// its reader gone, 2 the write cancelled), or -1 when the host took
    /// none yet and the write stays pending.
    Write,
}

impl Import {
    pub const ALL: [Import; 4] = [
        Import::NewStream,
        Import::WriteViaStream,
        Import::DropFuture,
        Import::Write,
    ];

    /// The name a core module imports it by. A built-in is named after the
    /// function whose type has the stream, `write-via-stream`, and the
    /// place of the stream or the future among that type's.
    pub fn name(self) -> &'static str {
        match self {
            Import::NewStream => "[stream-new-0]write-via-stream",
            Import::WriteViaStream => "write-via-stream",
            Import::DropFuture => "[future-drop-readable-1]write-via-stream",
            Import::Write => "[async-lower][stream-write-0]write-via-stream",
        }
    }

    /// How many `i32` it takes.
    fn params(self) -> usize {
        match self {
            Import::NewStream => 0,
            Import::WriteViaStream | Import::DropFuture => 1,
            Import::Write => 3,
        }
    }

    fn result(self) -> Option<MirElementary> {
        match self {
            Import::NewStream => Some(MirElementary::LWord),
            Import::WriteViaStream | Import::Write => Some(MirElementary::DInt),
            Import::DropFuture => None,
        }
    }
}

/// The outputs the bodies of `functions` write to.
pub fn written(functions: &[MirFunction]) -> Vec<Output> {
    let mut found = [false; 2];
    for func in functions {
        crate::stmt::for_each_stmt(&func.body, &mut |stmt| {
            if let MirStmt::WasmIntrinsic { instruction, .. } = stmt {
                for (at, output) in Output::ALL.into_iter().enumerate() {
                    found[at] |= instruction == output.instruction();
                }
            }
        });
    }
    Output::ALL
        .into_iter()
        .zip(found)
        .filter_map(|(output, found)| found.then_some(output))
        .collect()
}

/// What a module imports to write to `output`, as extern functions numbered
/// from `index`: symbols no declaration has, since each is the output's
/// instruction, a `$`, and the import's name.
pub fn imports(db: &dyn WorkspaceDataBase, output: Output, index: u32) -> Vec<MirExternFunction> {
    let ident = |text: String| Ident::new(db, CompactString::from(text));
    let interface = CompactString::from(output.interface());
    Import::ALL
        .into_iter()
        .zip(index..)
        .map(|(import, index)| MirExternFunction {
            name: ident(format!("{}${}", output.instruction(), import.name())),
            index,
            module: interface.clone(),
            import_name: CompactString::from(import.name()),
            params: (0..import.params())
                .map(|at| MirParam {
                    name: ident(format!("p{at}")),
                    ty: MirType::Elementary(MirElementary::DInt),
                    kind: MirParamKind::Input,
                })
                .collect(),
            out_results: Vec::new(),
            return_type: import.result().map(MirType::Elementary),
            linkage: MirLinkage::Internal,
            asynchronous: false,
        })
        .collect()
}

/// Where `module` imports `import` of `output` among its imports, when it
/// writes to that output.
pub fn import_at(module: &MirModule, output: Output, import: Import) -> Option<u32> {
    let interface = output.interface();
    module
        .extern_functions
        .iter()
        .position(|ext| ext.module == interface && ext.import_name == import.name())
        .map(|at| at as u32)
}

/// The outputs `module` writes to: those whose imports it has.
pub fn outputs(module: &MirModule) -> Vec<Output> {
    Output::ALL
        .into_iter()
        .filter(|output| {
            Import::ALL
                .into_iter()
                .all(|import| import_at(module, *output, import).is_some())
        })
        .collect()
}
