//! Back-end benchmarks on the stdlib (the corpus that compiles clean):
//! HIR → MIR lowering, and MIR → WASM emission, each isolated by priming
//! everything earlier in setup.

use divan::Bencher;
use hir::hir_def::semantic_index::semantic_index;
use rk_benchmark::{STDLIB_EXPECTED_DIAGNOSTICS, check_all, setup_db, stdlib_corpus};
use mir::lower::lower_module::lower_modules;

#[divan::bench(sample_count = 10, sample_size = 1)]
fn mir_lower(bencher: Bencher) {
    let corpus = stdlib_corpus();
    bencher
        .with_inputs(|| {
            let (db, files) = setup_db(&corpus);
            assert_eq!(check_all(&db, &files), STDLIB_EXPECTED_DIAGNOSTICS);
            (db, files)
        })
        .bench_local_refs(|(db, files)| {
            let indices: Vec<_> = files.iter().map(|f| semantic_index(db, *f)).collect();
            divan::black_box(lower_modules(db, &indices).expect("MIR lowering failed"));
        });
}

#[divan::bench(sample_count = 10, sample_size = 1)]
fn wasm_emit(bencher: Bencher) {
    let corpus = stdlib_corpus();
    bencher
        .with_inputs(|| {
            let (db, files) = setup_db(&corpus);
            assert_eq!(check_all(&db, &files), STDLIB_EXPECTED_DIAGNOSTICS);
            let indices: Vec<_> = files.iter().map(|f| semantic_index(&db, *f)).collect();
            let module = lower_modules(&db, &indices).expect("MIR lowering failed");
            (db, module)
        })
        .bench_local_refs(|(db, module)| {
            divan::black_box(wasm_codegen::generate_wasm(db, module).finish());
        });
}

fn main() {
    divan::main();
}
