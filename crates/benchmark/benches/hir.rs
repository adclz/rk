//! Cold HIR pipeline benchmarks, one stage per benchmark.
//!
//! Each stage's prerequisites are primed in the (untimed) setup, so the
//! timed region contains only the stage under measurement — a regression in
//! signature inference shows up in `infer_signature`, not diluted inside a
//! full-pipeline number. `check` is the end-to-end figure.

use divan::Bencher;
use rk_benchmark::{
    bodies_all, check_all, expected_diagnostics, index_all, initializations_all, load_corpus,
    setup_db, signatures_all,
};

const CORPORA: &[&str] = &["stdlib"];

#[divan::bench(args = CORPORA, sample_count = 10, sample_size = 1)]
fn semantic_index(bencher: Bencher, name: &str) {
    let corpus = load_corpus(name);
    bencher
        .with_inputs(|| setup_db(&corpus))
        .bench_local_refs(|(db, files)| index_all(db, files));
}

#[divan::bench(args = CORPORA, sample_count = 10, sample_size = 1)]
fn infer_signature(bencher: Bencher, name: &str) {
    let corpus = load_corpus(name);
    bencher
        .with_inputs(|| {
            let (db, files) = setup_db(&corpus);
            index_all(&db, &files);
            (db, files)
        })
        .bench_local_refs(|(db, files)| signatures_all(db, files));
}

#[divan::bench(args = CORPORA, sample_count = 10, sample_size = 1)]
fn infer_initialization(bencher: Bencher, name: &str) {
    let corpus = load_corpus(name);
    bencher
        .with_inputs(|| {
            let (db, files) = setup_db(&corpus);
            index_all(&db, &files);
            signatures_all(&db, &files);
            (db, files)
        })
        .bench_local_refs(|(db, files)| initializations_all(db, files));
}

#[divan::bench(args = CORPORA, sample_count = 10, sample_size = 1)]
fn infer_body(bencher: Bencher, name: &str) {
    let corpus = load_corpus(name);
    bencher
        .with_inputs(|| {
            let (db, files) = setup_db(&corpus);
            index_all(&db, &files);
            signatures_all(&db, &files);
            initializations_all(&db, &files);
            (db, files)
        })
        .bench_local_refs(|(db, files)| bodies_all(db, files));
}

#[divan::bench(args = CORPORA, sample_count = 10, sample_size = 1)]
fn check(bencher: Bencher, name: &str) {
    let corpus = load_corpus(name);
    let expected = expected_diagnostics(name);
    bencher
        .with_inputs(|| setup_db(&corpus))
        .bench_local_refs(|(db, files)| {
            assert_eq!(check_all(db, files), expected);
        });
}

fn main() {
    divan::main();
}
