// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::{expression::VariableAccessKind, statement::Stmt},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

use super::accesses::{self, Place};

pub const NAME: &str = "address-in-body";

/// L0132: an address written in a body instead of a variable located at it.
struct AddressInBody;

impl ErrorCode for AddressInBody {
    fn code(&self) -> &'static str {
        "L0132"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    statements: &[Stmt<'db>],
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    accesses::statements(db, statements, &mut |place, _| {
        let Place::Access(access) = place else {
            return;
        };
        let VariableAccessKind::Direct(direct) = access.kind(db) else {
            return;
        };
        let address = direct.to_address(db);
        let mut d = diag()
            .message(format!("the address '{address}' is used directly"))
            .desc(&AddressInBody)
            .range(
                hir::denormalize(db, access.get_scope_id(db).file(db), &access.get_span(db))
                    .unwrap_or_default(),
            )
            .severity(DiagnosticSeverity::WARNING)
            .call();
        d.with_note(
            "a variable declared at the address names what is wired there, and is the one place to change when the wiring moves"
                .to_string(),
        );
        let ty = match direct.size_letter(db) {
            Some('B') => "BYTE",
            Some('W') => "WORD",
            Some('D') => "DWORD",
            Some('L') => "LWORD",
            _ => "BOOL",
        };
        d.with_help(format!(
            "declare a variable at it, 'name AT {address} : {ty};', and use its name"
        ));
        diagnostics.push(d);
    });
}
