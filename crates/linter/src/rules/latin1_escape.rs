// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::HashMap;

use auto_lsp::lsp_types::{CodeAction, DiagnosticSeverity, TextEdit, WorkspaceEdit};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{Elementary, Expr, ExprKind, PrimaryExpr},
            spec::ElementarySpec,
        },
        scope::ScopeId,
    },
    hir_ty::ty::Type,
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "latin1-escape";

/// L0121: a STRING literal whose bytes are no UTF-8 text, because a `$hh`
/// escape of $80 or more stands where a character was meant. A `$hh` is one
/// byte, and a STRING is UTF-8: `'caf$E9'` ends in the lone byte 0xE9, not in
/// `é`, which is `$C3$A9`. Code written for the Latin-1 of the 3rd edition
/// uses it that way, so `'caf$E9' = 'café'` is FALSE there, while `CHAR_AT`,
/// which reads a stray byte as its Latin-1 code point, agrees with the old
/// meaning. A CHAR literal is a code point, where `$E9` is `é`: not reported.
struct Latin1Escape;

impl ErrorCode for Latin1Escape {
    fn code(&self) -> &'static str {
        "L0121"
    }

    fn description(&self) -> &'static str {
        "escape is not UTF-8 text"
    }
}

/// Every STRING literal of `scope`, in its body and in its declarations'
/// initializers, read with the type inference gave it.
pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let seen: rustc_hash::FxHashMap<Expr<'db>, bool> = scope
        .inference(db)
        .typed_exprs()
        .map(|(expr, ty)| {
            let is_char = matches!(ty.normalize(db), Type::Elementary(ElementarySpec::Char));
            (expr, is_char)
        })
        .collect();
    // In source order: the maps are not.
    let mut literals: Vec<(Expr<'db>, bool)> = seen.into_iter().collect();
    literals.sort_by_key(|(expr, _)| expr.get_span(db).start_byte);

    let file = scope.file(db);
    for (expr, is_char) in literals {
        let ident = match expr.expr(db) {
            ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::String(ident))) => *ident,
            // A bare literal is a CHAR where one is expected, and a CHAR is a
            // code point: `$E9` is `é` there.
            ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::InferString(ident)))
                if !is_char =>
            {
                *ident
            }
            _ => continue,
        };
        let Ok(bytes) = ident.as_single_string(db) else {
            continue;
        };
        let Some(stray) = bytes
            .utf8_chunks()
            .find_map(|chunk| chunk.invalid().first().copied())
        else {
            continue;
        };

        let meant = char::from(stray);
        let latin1: String = bytes
            .utf8_chunks()
            .flat_map(|chunk| {
                chunk
                    .valid()
                    .chars()
                    .chain(chunk.invalid().iter().map(|b| char::from(*b)))
            })
            .collect();
        // The literal as written, `STRING#` included, which the span covers.
        let written = hir::CallSite::from_scoped(db, &expr).to_string(db);
        let prefix = written.find('\'').map_or("", |quote| &written[..quote]);
        let replacement = format!("{prefix}'{}'", literal_text(&latin1));

        let range = hir::denormalize(db, file, &expr.get_span(db)).unwrap_or_default();
        let mut diagnostic = diag()
            .message(format!(
                "`${stray:02X}` is one byte, not '{meant}': a STRING is UTF-8, where '{meant}' is `{}`",
                utf8_escapes(meant)
            ))
            .desc(&Latin1Escape)
            .range(range)
            .severity(DiagnosticSeverity::WARNING)
            .call();
        diagnostic.with_fix(CodeAction {
            title: format!("write {replacement}"),
            edit: Some(WorkspaceEdit::new(HashMap::from([(
                file.url(db).clone(),
                vec![TextEdit {
                    range,
                    new_text: replacement,
                }],
            )]))),
            is_preferred: Some(true),
            ..Default::default()
        });
        diagnostics.push(diagnostic);
    }
}

/// `c` as the `$hh` escapes of its UTF-8 bytes: `é` is `$C3$A9`.
fn utf8_escapes(c: char) -> String {
    let mut buffer = [0; 4];
    c.encode_utf8(&mut buffer)
        .bytes()
        .map(|b| format!("${b:02X}"))
        .collect()
}

/// `text` between quotes: a character as itself, and `$`, `'` and control
/// characters escaped.
fn literal_text(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '$' => "$$".to_string(),
            '\'' => "$'".to_string(),
            c if c.is_control() => utf8_escapes(c),
            c => c.to_string(),
        })
        .collect()
}
