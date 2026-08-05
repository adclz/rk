//! Topiary-based formatting of every corpus file. Purely CST-driven, so
//! only parsing is primed (by `setup_db`) — no HIR work in the timed region.

use divan::Bencher;
use rk_benchmark::{load_corpus, setup_db};

const CORPORA: &[&str] = &["stdlib"];

#[divan::bench(args = CORPORA, sample_count = 10, sample_size = 1)]
fn format(bencher: Bencher, name: &str) {
    let corpus = load_corpus(name);
    bencher
        .with_inputs(|| setup_db(&corpus))
        .bench_local_refs(|(db, files)| {
            for file in files.iter() {
                divan::black_box(formatter::format(db, *file).expect("formatting failed"));
            }
        });
}

fn main() {
    divan::main();
}
