//! Lint-rule execution over a fully-checked workspace: the HIR pipeline is
//! primed in setup, so the timed region is the linter's own walking and
//! rule logic, with every rule enabled.

use divan::Bencher;
use rk_benchmark::{all_rules_config, check_all, expected_lints, lint_all, load_corpus, setup_db};

const CORPORA: &[&str] = &["stdlib"];

#[divan::bench(args = CORPORA, sample_count = 10, sample_size = 1)]
fn lint(bencher: Bencher, name: &str) {
    let corpus = load_corpus(name);
    let config = all_rules_config();
    let expected = expected_lints(name);
    bencher
        .with_inputs(|| {
            let (db, files) = setup_db(&corpus);
            check_all(&db, &files);
            (db, files)
        })
        .bench_local_refs(|(db, files)| {
            assert_eq!(lint_all(db, files, &config), expected);
        });
}

fn main() {
    divan::main();
}
