use rk_benchmark::{
    CASES, TestCase, bench_infer_body, bench_infer_signature, bench_semantic_index,
    collect_diagnostics, edit_file, setup_db,
};

fn cases() -> &'static [TestCase] {
    CASES
}

// ---------------------------------------------------------------------------
// Cold (from-scratch) benchmarks
// ---------------------------------------------------------------------------

#[divan::bench(args = cases())]
fn cold_diagnostics(bencher: divan::Bencher, case: &TestCase) {
    bencher
        .with_inputs(|| setup_db(case.sources()))
        .bench_local_refs(|(db, files)| {
            for file in files {
                collect_diagnostics(db, *file);
            }
        });
}

#[divan::bench(args = cases())]
fn cold_semantic_index(bencher: divan::Bencher, case: &TestCase) {
    bencher
        .with_inputs(|| setup_db(case.sources()))
        .bench_local_refs(|(db, files)| {
            for file in files {
                bench_semantic_index(db, *file);
            }
        });
}

#[divan::bench(args = cases())]
fn cold_infer_signature(bencher: divan::Bencher, case: &TestCase) {
    bencher
        .with_inputs(|| setup_db(case.sources()))
        .bench_local_refs(|(db, files)| {
            for file in files {
                bench_infer_signature(db, *file);
            }
        });
}

#[divan::bench(args = cases())]
fn cold_infer_body(bencher: divan::Bencher, case: &TestCase) {
    bencher
        .with_inputs(|| setup_db(case.sources()))
        .bench_local_refs(|(db, files)| {
            for file in files {
                bench_infer_body(db, *file);
            }
        });
}

// ---------------------------------------------------------------------------
// Incremental benchmarks — measure Salsa re-analysis after a body-only edit.
//
// Following Ruff's pattern: setup primes all caches via a full diagnostic
// pass, then applies the edit. Only the re-analysis query is timed.
// Because the edit is body-only, signature caches should be reused.
// ---------------------------------------------------------------------------

#[divan::bench(args = cases())]
fn incremental_diagnostics(bencher: divan::Bencher, case: &TestCase) {
    bencher
        .with_inputs(|| {
            let (mut db, files) = setup_db(case.sources());
            // Prime every Salsa cache by running the full pipeline.
            for f in &files {
                collect_diagnostics(&db, *f);
            }
            // Apply the edit during setup so only re-analysis is timed.
            edit_file(&mut db, files[0], case.edited_source());
            (db, files)
        })
        .bench_local_refs(|(db, files)| {
            collect_diagnostics(db, files[0]);
        });
}

#[divan::bench(args = cases())]
fn incremental_infer_body(bencher: divan::Bencher, case: &TestCase) {
    bencher
        .with_inputs(|| {
            let (mut db, files) = setup_db(case.sources());
            for f in &files {
                collect_diagnostics(&db, *f);
            }
            edit_file(&mut db, files[0], case.edited_source());
            (db, files)
        })
        .bench_local_refs(|(db, files)| {
            bench_infer_body(db, files[0]);
        });
}

fn main() {
    divan::main();
}
