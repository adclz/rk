// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

pub mod checks;
pub mod init_inference;
pub mod instances;
pub mod signature;

use ide_diagnostic::IdeDiagnostic;

/// Drops from `errors[from..]` each report already made in
/// `errors[since..from]`, the same place, code and message. Each name of
/// `x, y : ARRAY[5..1] OF INT` has a spec of its own, and what is wrong with
/// the type, written once, is reported once. What is wrong with a variable
/// is placed at its name, and stays reported for each.
pub(crate) fn drop_repeated(errors: &mut Vec<IdeDiagnostic>, since: usize, from: usize) {
    let mut at = from;
    while at < errors.len() {
        let repeated = errors[since..from]
            .iter()
            .any(|made| made.diagnostic == errors[at].diagnostic);
        if repeated {
            errors.remove(at);
        } else {
            at += 1;
        }
    }
}
