use ariadne::CharSet;
use ariadne::Config;
use ariadne::Source;
use auto_lsp::default::db::BaseDatabase;
use auto_lsp::{
    default::db::{FileManager, file::File},
    lsp_types::Url,
};
use db::RootDatabase;
use hir::check::diagnostics_for_file;
use hir::hir_def::pous::pou::PouDecl;
use hir::hir_def::semantic_index::semantic_index;
use rstest::fixture;

#[fixture]
pub fn with_db() -> RootDatabase {
    RootDatabase::default()
}

pub fn no_color_and_ascii() -> Config {
    Config::default()
        .with_color(false)
        // Using Ascii so that the inline snapshots display correctly
        // even with fonts where characters like '┬' take up more space.
        .with_char_set(CharSet::Ascii)
}

pub fn add_sources<'db>(db: &'db mut RootDatabase, sources: &[&str]) {
    for (i, source) in sources.iter().enumerate() {
        let url = Url::parse(&format!("file:///test{i}.st")).unwrap();

        let file = File::from_string()
            .db(db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();

        db.add_file(file).unwrap();
    }
}

pub fn test_diagnostics(db: &mut RootDatabase, source: &[&str]) -> String {
    add_sources(db, source);
        let mut cache = vec![];

    for file in db.get_files().iter() {
        diagnostics_for_file(db, *file).iter().for_each(|d| {
            d.create_report(db, *file, Some(no_color_and_ascii()))
                .write(
                    (
                        file.url(db).as_str(),
                        Source::from(file.document(db).as_str()),
                    ),
                    &mut cache,
                )
                .unwrap();
        });
    }

    String::from_utf8(cache).unwrap()
}

pub fn find_pou_with_name<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    name: &str,
) -> Option<PouDecl<'db>> {
    let sema = semantic_index(db, file);

    for pou in sema.global_pous.iter().copied() {
        if pou.name(db).text(db).as_str() == name {
            return Some(pou);
        }
    }

    for ns in sema.namespaces.iter() {
        for pou in ns.pous(db) {
            if pou.name(db).text(db).as_str() == name {
                return Some(*pou);
            }
        }
    }

    None
}
