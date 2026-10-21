// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Incremental re-analysis: apply one edit to a fully-checked workspace and
//! re-check *everything*, timed. The setup primes every Salsa cache, so the
//! timed region is exactly what an editor session pays per change: reparse
//! of the edited file + whatever the edit genuinely invalidates.
//!
//! The three scenarios bracket the invalidation spectrum — `comment_edit`
//! (span-only: the per-edit floor of reparse + the edited file's own
//! re-analysis), `body_edit` (signatures stable, dependents must reuse),
//! `add_pou` (name indexes rebuild). Compare against `hir::check` for the
//! from-scratch ceiling on the same corpus.
//!
//! On the generated workspace, `body_edit` and `add_pou` change a file
//! that most others call: what they inferred from it is asked again.

use divan::Bencher;
use rk_benchmark::{
    EDITS, Edit, GENERATED_EDITS, STDLIB_EXPECTED_DIAGNOSTICS, check_all, edit_file,
    expected_diagnostics, load_corpus, setup_db, stdlib_corpus,
};

#[divan::bench(args = EDITS, sample_count = 10, sample_size = 1)]
fn recheck_stdlib(bencher: Bencher, edit: &Edit) {
    let corpus = stdlib_corpus();
    let (idx, edited) = edit.apply(&corpus);
    bencher
        .with_inputs(|| {
            let (db, files) = setup_db(&corpus);
            assert_eq!(check_all(&db, &files), STDLIB_EXPECTED_DIAGNOSTICS);
            (db, files)
        })
        .bench_local_refs(|(db, files)| {
            edit_file(db, files[idx], &edited);
            assert_eq!(check_all(db, files), STDLIB_EXPECTED_DIAGNOSTICS);
        });
}

#[divan::bench(args = GENERATED_EDITS, sample_count = 10, sample_size = 1)]
fn recheck_generated(bencher: Bencher, edit: &Edit) {
    let corpus = load_corpus("generated_1k");
    let expected = expected_diagnostics("generated_1k");
    let (idx, edited) = edit.apply(&corpus);
    bencher
        .with_inputs(|| {
            let (db, files) = setup_db(&corpus);
            assert_eq!(check_all(&db, &files), expected);
            (db, files)
        })
        .bench_local_refs(|(db, files)| {
            edit_file(db, files[idx], &edited);
            assert_eq!(check_all(db, files), expected);
        });
}

fn main() {
    divan::main();
}
