//! Raw tree-sitter parsing over the real corpora — the front of the
//! pipeline, isolated from the database and every later stage.

use rk_benchmark::load_corpus;

const CORPORA: &[&str] = &["stdlib"];

#[divan::bench(args = CORPORA)]
fn parse(bencher: divan::Bencher, name: &str) {
    let corpus = load_corpus(name);
    bencher.bench_local(|| {
        let mut parser = ast::RK_PARSER.parser.write();
        for (_, source) in &corpus.files {
            divan::black_box(
                parser
                    .parse(source.as_bytes(), None)
                    .expect("tree-sitter parse failed"),
            );
        }
    });
}

fn main() {
    divan::main();
}
