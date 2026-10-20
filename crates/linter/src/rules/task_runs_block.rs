// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{config::ProgConfElement, semantic_index::semantic_index},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "task-runs-block";

/// L0316: a task associated with a FUNCTION_BLOCK instance, `fb1 WITH Fast`.
struct TaskRunsBlock;

impl ErrorCode for TaskRunsBlock {
    fn code(&self) -> &'static str {
        "L0316"
    }
}

pub fn check(db: &dyn WorkspaceDataBase, file: File, diagnostics: &mut Vec<IdeDiagnostic>) {
    for config in semantic_index(db, file).configs.iter() {
        for resource in config.resources(db) {
            for program in resource.programs(db) {
                for element in program.conf_elements(db) {
                    let ProgConfElement::FbTask(fb_task) = element else {
                        continue;
                    };
                    let instance = fb_task.path.ident(db).with_case.text(db);
                    let task = fb_task.task.with_case.text(db);
                    let mut d = diag()
                        .message(format!(
                            "the task '{task}' runs the instance '{instance}' instead of a PROGRAM"
                        ))
                        .desc(&TaskRunsBlock)
                        .range(
                            hir::denormalize(db, file, &fb_task.path.get_span(db))
                                .unwrap_or_default(),
                        )
                        .severity(DiagnosticSeverity::HINT)
                        .call();
                    d.with_note(
                        "the instance then runs at the task's rate, not when its PROGRAM runs"
                            .to_string(),
                    );
                    d.with_help(format!(
                        "call '{instance}' from a PROGRAM that '{task}' runs"
                    ));
                    diagnostics.push(d);
                }
            }
        }
    }
}
