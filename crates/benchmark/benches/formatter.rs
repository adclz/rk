// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Topiary-based formatting of every corpus file. Purely CST-driven, so
//! only parsing is primed (by `setup_db`) — no HIR work in the timed region.

use std::sync::LazyLock;

use divan::Bencher;
use rk_benchmark::{load_corpus, setup_db};

const CORPORA: &[&str] = &["stdlib"];

#[divan::bench(args = CORPORA, sample_count = 10, sample_size = 1)]
fn format(bencher: Bencher, name: &str) {
    let corpus = load_corpus(name);
    // The formatter builds its query on first use, once per process. CodSpeed
    // times a single call, so built there, the query was most of the figure.
    LazyLock::force(&formatter::TOPIARY_LANG);
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
