// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::calls::{CallNode, call_chain, calls, is_recursive, reachable},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "recursion";

/// L0122: a call that leads back to the POU making it. IEC 61131-3 has no
/// recursion; rk runs it as CODESYS does, each call with a frame of its own
/// on the stack, whose depth nothing bounds before the program runs.
struct Recursion;

impl ErrorCode for Recursion {
    fn code(&self) -> &'static str {
        "L0122"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let node = match get_scope(db, scope).kind {
        ScopeKind::Pou(Pou::Function(f)) => CallNode::Function(f),
        ScopeKind::Pou(Pou::FunctionBlock(fb)) => CallNode::Body(fb),
        ScopeKind::MethodDecl(m) => CallNode::Method(m),
        _ => return,
    };
    if !is_recursive(db, node) {
        return;
    }
    let file = scope.file(db);
    let mut sites: Vec<_> = calls(db, node)
        .into_iter()
        .filter_map(|call| {
            // The first body the call may run that leads back here.
            let target = call
                .targets
                .iter()
                .copied()
                .find(|target| *target == node || reachable(db, *target).contains(&node))?;
            let range = hir::denormalize(db, file, &call.site.get_span(db))?;
            Some((range, target))
        })
        .collect();
    sites.sort_by_key(|(range, _)| (range.start.line, range.start.character));
    sites.dedup_by_key(|(range, _)| *range);

    let name = node.display_name(db);
    for (range, target) in sites {
        let message = if target == node {
            format!("'{name}' calls itself")
        } else {
            let chain = call_chain(db, target, node).unwrap_or_default();
            let through: Vec<String> = chain
                .iter()
                .take(chain.len().saturating_sub(1))
                .map(|node| format!("'{}'", node.display_name(db)))
                .collect();
            format!("'{name}' calls itself through {}", through.join(", then "))
        };
        let mut d = diag()
            .message(message)
            .desc(&Recursion)
            .range(range)
            .severity(DiagnosticSeverity::WARNING)
            .call();
        d.with_note(
            "each call gets its own frame on the stack, so too deep a recursion stops the program"
                .to_string(),
        );
        diagnostics.push(d);
    }
}
