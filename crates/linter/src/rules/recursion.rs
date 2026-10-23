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
    hir_ty::calls::{CallGraph, CallNode, calls},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "recursion";

/// L0122: a call that leads back to the POU making it. IEC 61131-3 has no
/// recursion; rk runs it with a frame of its own for each call, on a stack
/// whose depth nothing bounds before the program runs.
struct Recursion;

impl ErrorCode for Recursion {
    fn code(&self) -> &'static str {
        "L0122"
    }
}

/// The bodies a message names before it says how many more: a cycle
/// through thousands of POUs would name every one of them.
const NAMED: usize = 8;

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    graph: &CallGraph<'db>,
    scope: ScopeId<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let node = match get_scope(db, scope).kind {
        ScopeKind::Pou(Pou::Function(f)) => CallNode::Function(f),
        ScopeKind::Pou(Pou::FunctionBlock(fb)) => CallNode::Body(fb),
        ScopeKind::MethodDecl(m) => CallNode::Method(m),
        _ => return,
    };
    if !graph.is_recursive(node) {
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
                .find(|target| graph.on_one_cycle(*target, node))?;
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
            let chain = graph.chain(target, node).unwrap_or_default();
            let between = &chain[..chain.len().saturating_sub(1)];
            // One more name rather than "1 other".
            let named = if between.len() > NAMED + 1 {
                NAMED
            } else {
                between.len()
            };
            let mut through: Vec<String> = between[..named]
                .iter()
                .map(|node| format!("'{}'", node.display_name(db)))
                .collect();
            if named < between.len() {
                through.push(format!("{} others", between.len() - named));
            }
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
