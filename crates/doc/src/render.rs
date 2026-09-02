use ariadne::{CharSet, Config, FnCache, Source};
use auto_lsp::{
    default::db::{BaseDatabase, FileManager, file::File},
    lsp_types::Url,
};
use db::RootDatabase;
use hir::check::diagnostics_for_file;

pub fn compile_and_render(
    db: &mut RootDatabase,
    sources_str: &[&str],
    lint_rule: Option<&str>,
) -> String {
    for (i, source) in sources_str.iter().enumerate() {
        let url = Url::parse(&format!("file:///example{i}.st")).unwrap();
        let file = File::from_string()
            .db(db)
            .parsers(&ast::RK_PARSER)
            .url(&url)
            .source(source.trim_start_matches('\n').to_string())
            .call()
            .unwrap();
        db.add_file(file).unwrap();
    }

    let mut output = Vec::new();

    let mut files: Vec<File> = db.get_files().iter().map(|entry| *entry.value()).collect();
    files.sort_by_key(|file: &File| {
        let url_str = file.url(db).as_str().to_owned();
        url_str
            .strip_prefix("file:///example")
            .and_then(|s| s.strip_suffix(".st"))
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
    });

    let file_sources: Vec<(&str, &str)> = files
        .iter()
        .map(|file: &File| (file.url(db).as_str(), file.document(db).as_str()))
        .collect();

    let config = Config::default()
        .with_color(true)
        .with_char_set(CharSet::Unicode);

    let linter_config = lint_rule.map(|rule| {
        use std::collections::BTreeMap;
        // Disable all rules, then enable only the target one
        let mut rules = BTreeMap::new();
        for name in linter::ALL_RULE_NAMES {
            rules.insert(name.to_string(), *name == rule);
        }
        db::config_file::LinterConfig {
            select: Some(db::config_file::Select::All),
            rules: Some(rules),
        }
    });

    for file in files.iter() {
        let source_text = file.document(db);
        let mut all = diagnostics_for_file(db, *file).as_ref().clone();
        if let Some(ref linter_config) = linter_config {
            linter::lint_file(db, *file, linter_config, &mut all);
        }
        for d in all.iter() {
            d.create_report(db, file.url(db), source_text.as_str(), Some(config), true)
                .write(
                    FnCache::new(
                        (move |id: &&str| Err(format!("Failed to fetch source '{id}'")))
                            as fn(&&str) -> _,
                    )
                    .with_sources(
                        file_sources
                            .iter()
                            .map(|(id, s)| (*id, Source::from(*s)))
                            .collect(),
                    ),
                    &mut output,
                )
                .unwrap();
        }
    }

    String::from_utf8(output).unwrap()
}

/// Convert an ANSI-colored string to an HTML fragment with inline styles.
pub fn ansi_to_html_fragment(ansi: &str) -> String {
    ansi_to_html::convert(ansi).unwrap_or_else(|_| html_escape(ansi))
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
