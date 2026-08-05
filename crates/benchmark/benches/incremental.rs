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

use divan::Bencher;
use rk_benchmark::{
    EDITS, Edit, STDLIB_EXPECTED_DIAGNOSTICS, check_all, edit_file, setup_db, stdlib_corpus,
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

fn main() {
    divan::main();
}
