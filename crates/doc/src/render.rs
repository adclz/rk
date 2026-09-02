use ariadne::{CharSet, Config, FnCache, Source};
use auto_lsp::{
    default::db::{BaseDatabase, FileManager, file::File},
    lsp_types::{DiagnosticSeverity, NumberOrString, Url},
};
use db::RootDatabase;
use hir::check::diagnostics_for_file;

/// One diagnostic the compiler reported on an example, located so the page
/// can mark the exact source range.
#[derive(Clone, Debug)]
pub struct DiagSpan {
    /// Which of the example's sources.
    pub source_idx: usize,
    /// Byte range in that source, as the site displays it.
    pub start: usize,
    pub end: usize,
    pub message: String,
    /// `error`, `warning` or `info`.
    pub severity: &'static str,
    pub code: Option<String>,
    pub code_desc: Option<String>,
    pub notes: Vec<String>,
    /// (message, source index, 1-based line, 1-based column)
    pub related: Vec<(String, usize, usize, usize)>,
    pub fixes: Vec<String>,
}

fn byte_offset(src: &str, line: u32, character: u32) -> usize {
    let mut offset = 0;
    for (i, l) in src.split_inclusive('\n').enumerate() {
        if i as u32 == line {
            let col = l
                .char_indices()
                .nth(character as usize)
                .map(|(b, _)| b)
                .unwrap_or(l.trim_end_matches('\n').len());
            return offset + col;
        }
        offset += l.len();
    }
    src.len()
}

fn line_col(src: &str, byte: usize) -> (usize, usize) {
    let before = &src[..byte.min(src.len())];
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (line, col)
}

/// Compile the example and render its report the way `rk check` prints it.
/// The sources are added with their leading blank lines trimmed, which is
/// also how the site displays them, so the spans line up.
pub fn compile_and_render(
    db: &mut RootDatabase,
    sources_str: &[&str],
    lint_rule: Option<&str>,
) -> (String, Vec<DiagSpan>) {
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
    let mut spans = Vec::new();

    let mut files: Vec<File> = db.get_files().iter().map(|entry| *entry.value()).collect();
    let index_of = |file: &File| -> usize {
        file.url(db)
            .as_str()
            .strip_prefix("file:///example")
            .and_then(|s| s.strip_suffix(".st"))
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0)
    };
    files.sort_by_key(index_of);

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
        let source_idx = index_of(file);
        let source_text = file.document(db);
        let src = source_text.as_str();

        let mut all = diagnostics_for_file(db, *file).as_ref().clone();
        if let Some(ref linter_config) = linter_config {
            linter::lint_file(db, *file, linter_config, &mut all);
        }
        for d in all.iter() {
            let range = d.range();
            let severity = match d.diagnostic.severity {
                Some(DiagnosticSeverity::ERROR) => "error",
                Some(DiagnosticSeverity::WARNING) => "warning",
                _ => "info",
            };
            let start = byte_offset(src, range.start.line, range.start.character);
            let end = byte_offset(src, range.end.line, range.end.character).max(start + 1);
            spans.push(DiagSpan {
                source_idx,
                start,
                end: end.min(src.len().max(start + 1)),
                message: d.diagnostic.message.clone(),
                severity,
                code: d.diagnostic.code.as_ref().map(|c| match c {
                    NumberOrString::Number(n) => n.to_string(),
                    NumberOrString::String(s) => s.clone(),
                }),
                code_desc: d.code_desc().map(|s| s.to_string()),
                notes: d.notes().to_vec(),
                related: d
                    .related()
                    .iter()
                    .map(|r| {
                        let rel_src = r.file.document(db);
                        let (line, col) = line_col(rel_src.as_str(), r.range.start_byte);
                        (r.message.clone(), index_of(&r.file), line, col)
                    })
                    .collect(),
                fixes: d.fixes().iter().map(|f| f.title.clone()).collect(),
            });

            d.create_report(db, file.url(db), src, Some(config), true)
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

    (String::from_utf8(output).unwrap(), spans)
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
