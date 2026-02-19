use std::sync::Arc;
use std::sync::RwLock;

use ariadne::Cache;
use ariadne::CharSet;
use ariadne::Config;
use ariadne::FnCache;
use ariadne::Source;
use auto_lsp::default::db::BaseDatabase;
use auto_lsp::salsa::Event;
use auto_lsp::{
    default::db::{FileManager, file::File},
    lsp_types::Url,
};
use db::RootDatabase;
use db::WorkspaceDataBase;
use hir::HasName;
use hir::check::diagnostics_for_file;
use hir::hir_def::namespace::NamespaceDecl;
use hir::hir_def::pous::pou::Pou;
use hir::hir_def::semantic_index::semantic_index;
use rstest::fixture;

#[fixture]
pub fn with_db() -> RootDatabase {
    RootDatabase::default()
}

#[fixture]
pub fn with_log_db() -> (RootDatabase, Arc<RwLock<Vec<Event>>>) {
    let log = Arc::new(RwLock::new(vec![]));
    let cloned_log = log.clone();
    let db = RootDatabase::new(Some(Box::new(move |l| {
        cloned_log.write().unwrap().push(l);
    })));
    (db, log)
}

pub fn no_color_and_ascii() -> Config {
    Config::default()
        .with_color(false)
        // Using Ascii so that the inline snapshots display correctly
        // even with fonts where characters like '┬' take up more space.
        .with_char_set(CharSet::Ascii)
}

pub fn add_sources(db: &mut RootDatabase, sources: &[&str]) {
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

pub fn sources<Id, S, I>(iter: I) -> impl Cache<Id>
where
    Id: std::fmt::Display + std::hash::Hash + PartialEq + Eq + Clone,
    I: IntoIterator<Item = (Id, S)>,
    S: AsRef<str>,
{
    FnCache::new((move |id| Err(format!("Failed to fetch source '{id}'"))) as fn(&_) -> _)
        .with_sources(
            iter.into_iter()
                .map(|(id, s)| (id, Source::from(s)))
                .collect(),
        )
}

pub fn test_diagnostics<'db>(db: &'db mut RootDatabase, source: &'db [&'db str]) -> String {
    add_sources(db, source);
    let mut cache = vec![];

    // we need to sort the files by their URL
    let mut files = db.get_files().iter().map(|file| *file).collect::<Vec<_>>();
    files.sort_by_key(|file| {
        let url_str = file.url(db).as_str();
        // Extract number from "file:///testN.st" format
        url_str
            .strip_prefix("file:///test")
            .and_then(|s| s.strip_suffix(".st"))
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
    });

    let file_sources = files
        .iter()
        .map(|file| (file.url(db).as_str(), file.document(db).as_str()))
        .collect::<Vec<_>>();

    for file in files {
        diagnostics_for_file(db, file).iter().for_each(|d| {
            d.create_report(db, file.url(db), file.document(db).as_str(), Some(no_color_and_ascii()), false)
                .write(sources(file_sources.clone()), &mut cache)
                .unwrap();
        });
    }

    String::from_utf8(cache).unwrap()
}

pub fn find_pou_with_name<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    name: &str,
) -> Option<Pou<'db>> {
    let sema = semantic_index(db, file);

    for pou in sema.global_pous.iter().copied() {
        if pou.get_name_ident(db).text(db).as_str() == name {
            return Some(pou);
        }
    }

    for ns in sema.global_namespaces.iter() {
        for pou in ns.pous(db) {
            if pou.get_name_ident(db).text(db).as_str() == name {
                return Some(*pou);
            }
        }
    }

    None
}

pub fn find_namespace_with_name<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    name: &str,
) -> Option<NamespaceDecl<'db>> {
    let sema = semantic_index(db, file);

    for ns in sema.global_namespaces.iter() {
        if ns.path(db).to_string(db) == name {
            return Some(*ns);
        }
    }

    None
}

