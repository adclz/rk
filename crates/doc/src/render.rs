use ariadne::{CharSet, Config, FnCache, Source};
use auto_lsp::{
    default::db::{BaseDatabase, FileManager, file::File},
    lsp_types::{DiagnosticSeverity, Url},
};
use db::RootDatabase;
use hir::check::diagnostics_for_file;

/// A diagnostic span for embedding in HTML.
#[derive(Clone, Debug)]
pub struct DiagSpan {
    /// 0-based line within the source (adjusted for trimming)
    pub line: u32,
    /// 0-based start column
    pub col_start: u32,
    /// 0-based end column
    pub col_end: u32,
    /// Error message
    pub message: String,
    /// "error" or "warning" or "info"
    pub severity: &'static str,
    /// Error code like "E0301"
    pub code: Option<String>,
    /// Short description of the error category
    pub code_desc: Option<String>,
    /// Additional notes
    pub notes: Vec<String>,
    /// Related info: (message, line, col)
    pub related: Vec<(String, u32, u32)>,
    /// Quick fix suggestions
    pub fixes: Vec<String>,
    /// Which source index (for multi-source examples)
    pub source_idx: usize,
}

/// Convert a character offset to a visual column, expanding tabs to the tab width used in <pre>.
/// `line_text` is the content of the line, `char_offset` is 0-based character position.
fn char_to_visual_col(line_text: &str, char_offset: u32, tab_width: u32) -> u32 {
    let mut visual = 0;
    for (i, ch) in line_text.chars().enumerate() {
        if i as u32 >= char_offset {
            break;
        }
        if ch == '\t' {
            visual += tab_width - (visual % tab_width);
        } else {
            visual += 1;
        }
    }
    visual
}

/// Get the text of a specific line (0-based) from source.
fn get_line(src: &str, line: u32) -> &str {
    src.lines().nth(line as usize).unwrap_or("")
}

/// Convert a byte offset to (line, col), both 0-based.
fn byte_to_line_col(src: &str, byte: usize) -> (usize, usize) {
    let mut line = 0;
    let mut col = 0;
    for (i, ch) in src.char_indices() {
        if i >= byte {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Compile the given ST sources and return (ariadne ANSI output, diagnostic spans).
///
/// When `lint_rule` is `Some`, only that single lint rule is enabled.
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
        db::config_file::LinterConfig { rules: Some(rules) }
    });

    for (file_idx, file) in files.iter().enumerate() {
        let source_text = file.document(db);
        // Count leading blank lines that will be trimmed
        let leading_blanks = source_text
            .as_str()
            .chars()
            .take_while(|c| *c == '\n')
            .count() as u32;

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
            // Convert character offsets to visual columns (tabs expand)
            let adjusted_line = range.start.line.saturating_sub(leading_blanks);
            let line_text = get_line(source_text.as_str(), range.start.line);
            let tab_w = 4; // match CSS tab-size
            let vis_start = char_to_visual_col(line_text, range.start.character, tab_w);
            let vis_end = if range.end.line == range.start.line {
                char_to_visual_col(line_text, range.end.character, tab_w)
            } else {
                200 // JS will clamp
            };
            let code = d.diagnostic.code.as_ref().map(|c| match c {
                auto_lsp::lsp_types::NumberOrString::Number(n) => n.to_string(),
                auto_lsp::lsp_types::NumberOrString::String(s) => s.clone(),
            });
            spans.push(DiagSpan {
                line: adjusted_line,
                col_start: vis_start,
                col_end: vis_end,
                message: d.diagnostic.message.clone(),
                severity,
                code,
                code_desc: d.code_desc().map(|s| s.to_string()),
                notes: d.notes().to_vec(),
                related: d
                    .related()
                    .iter()
                    .map(|r| {
                        let rel_src = r.file.document(db);
                        let (rel_line, rel_col) =
                            byte_to_line_col(rel_src.as_str(), r.range.start_byte);
                        let rel_line = rel_line.saturating_sub(leading_blanks as usize);
                        (r.message.clone(), rel_line as u32, rel_col as u32)
                    })
                    .collect(),
                fixes: d.fixes().iter().map(|f| f.title.clone()).collect(),
                source_idx: file_idx,
            });

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

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\'', "&#39;")
}

fn json_str_array(items: &[String]) -> String {
    let escaped: Vec<String> = items
        .iter()
        .map(|s| format!("\"{}\"", json_escape(s)))
        .collect();
    format!("[{}]", escaped.join(","))
}

fn json_related_array(items: &[(String, u32, u32)]) -> String {
    let escaped: Vec<String> = items
        .iter()
        .map(|(msg, line, col)| {
            format!(
                r#"{{"msg":"{}","line":{},"col":{}}}"#,
                json_escape(msg),
                line,
                col
            )
        })
        .collect();
    format!("[{}]", escaped.join(","))
}

/// Serialize diagnostic spans for a given source index into a JSON array.
fn spans_to_json(spans: &[DiagSpan], source_idx: usize) -> String {
    let items: Vec<String> = spans
        .iter()
        .filter(|s| s.source_idx == source_idx)
        .map(|s| {
            let code = match &s.code {
                Some(c) => format!("\"{}\"", json_escape(c)),
                None => "null".to_string(),
            };
            let code_desc = match &s.code_desc {
                Some(c) => format!("\"{}\"", json_escape(c)),
                None => "null".to_string(),
            };
            format!(
                r#"{{"line":{},"start":{},"end":{},"msg":"{}","sev":"{}","code":{},"desc":{},"notes":{},"related":{},"fixes":{}}}"#,
                s.line,
                s.col_start,
                s.col_end,
                json_escape(&s.message),
                s.severity,
                code,
                code_desc,
                json_str_array(&s.notes),
                json_related_array(&s.related),
                json_str_array(&s.fixes),
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

/// Render a single error/warning entry as an HTML fragment.
pub fn render_entry_html(
    code: &str,
    title: &str,
    description: &str,
    sources: &[&str],
    report_html: &str,
    diag_spans: &[DiagSpan],
) -> String {
    let mut html = format!(
        r##"<section class="entry" id="{code}">
  <h2><a href="#{code}">{code}</a> - {title}</h2>
  <p class="description">{description}</p>
  <h3>Example</h3>
"##
    );

    for (i, source) in sources.iter().enumerate() {
        let source_trimmed = source
            .trim_matches('\n')
            .lines()
            .collect::<Vec<_>>()
            .join("\n");
        let diag_json = spans_to_json(diag_spans, i);
        html.push_str(&format!(
            "  <pre data-diags='{diag_json}'><code class=\"language-iecst\">{}</code></pre>\n",
            html_escape(&source_trimmed)
        ));
    }

    html.push_str(&format!(
        r#"  <h3>Compiler Output</h3>
  <div class="diagnostic-output"><pre>{report_html}</pre></div>
</section>
"#
    ));

    html
}

/// Render the sidebar navigation.
pub fn render_sidebar_html(categories: &[(&str, Vec<(&str, &str)>)]) -> String {
    let mut html = String::from(r#"<nav class="sidebar">"#);
    html.push_str(r#"<div class="sidebar-header">"#);
    html.push_str(
        r#"<a class="sidebar-back" href="/"><svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 12H5M12 19l-7-7 7-7"/></svg> Home</a>"#,
    );
    html.push_str(
        r#"<div class="sidebar-title"><span class="sidebar-icon"><svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"><path d="M2.25.749h19.5s1.5 0 1.5 1.5v19.5s0 1.5-1.5 1.5H2.25s-1.5 0-1.5-1.5v-19.5s0-1.5 1.5-1.5"/><path d="m12 4.499l-4.5 6l-3-3m9.75.75h4.5M12 13.499l-4.5 6l-3-3m9.75.75h4.5"/></svg></span> RK Diagnostics</div>"#,
    );
    html.push_str(
        r#"<input type="text" id="search" placeholder="Search E0301, type mismatch..." autocomplete="off">"#,
    );
    html.push_str("</div>");
    html.push_str(r#"<div class="sidebar-cats">"#);

    for (category, entries) in categories {
        let slug = category.to_lowercase().replace(' ', "-");
        html.push_str(&format!(
            "<h3 class=\"sidebar-cat collapsed\" data-cat=\"{slug}\" onclick=\"this.classList.toggle('collapsed');this.nextElementSibling.classList.toggle('collapsed')\"><span class=\"cat-arrow\"></span>{category}</h3>"
        ));
        html.push_str("<div class=\"sidebar-group collapsed\">");
        for (code, title) in entries {
            html.push_str(&format!(
                "<a href=\"#{code}\" class=\"sidebar-entry\" data-code=\"{code}\" data-title=\"{title}\">{code}</a>"
            ));
        }
        html.push_str("</div>");
    }

    html.push_str("</div></nav>");
    html
}

/// Assemble the full single-page HTML document.
pub fn render_page(
    sidebar: &str,
    body: &str,
    tm_grammar_json: &str,
    success: usize,
    fail: usize,
) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>RK - Diagnostics Reference</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500;600&display=swap" rel="stylesheet">
  <style>
    :root {{
      --sidebar-width: 260px;
      --primary: #f0c040;
      --primary-dim: #d4a017;
      --bg: #0e0e0e;
      --bg-surface: #161616;
      --border: #2a2a2a;
      --text: #e0e0e0;
      --text-muted: #787878;
      --text-subtle: #b0b0b0;
      --mono: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', Consolas, monospace;
    }}
    * {{ box-sizing: border-box; }}
    body {{ margin: 0; font-family: system-ui, -apple-system, 'Segoe UI', Roboto, Ubuntu, Cantarell, 'Noto Sans', sans-serif; background: var(--bg); color: var(--text); line-height: 1.6; font-size: 16px; }}
    .layout {{ display: flex; min-height: 100vh; }}

    /* Sidebar */
    nav.sidebar {{
      width: var(--sidebar-width); position: fixed; top: 0; left: 0; bottom: 0;
      display: flex; flex-direction: column;
      background: var(--bg-surface); padding: 0 12px;
      border-right: 1px solid var(--border); font-size: 0.875em;
    }}
    .sidebar-back {{ display: inline-flex; align-items: center; gap: 6px; color: var(--text-muted, #888); font-size: 0.9em; font-weight: 600; text-decoration: none; padding: 6px 4px 0; margin-bottom: 4px; transition: color 0.15s; }}
    .sidebar-back svg {{ vertical-align: middle; position: relative; top: -0.5px; }}
    .sidebar-back:hover {{ color: var(--primary); }}
    .sidebar-title {{ font-weight: 700; font-size: 1.1em; color: var(--primary); padding: 8px 4px 16px; border-bottom: 1px solid var(--border); margin-bottom: 8px; display: flex; align-items: center; gap: 8px; }}
    .sidebar-icon {{ display: inline-flex; align-items: center; justify-content: center; width: 28px; height: 28px; border-radius: 6px; background: var(--primary); color: #000; font-weight: 800; font-size: 0.75em; font-family: var(--mono); letter-spacing: -0.5px; flex-shrink: 0; }}
    .sidebar-header {{ flex-shrink: 0; padding-top: 16px; padding-bottom: 8px; }}
    .sidebar-cats {{ overflow-y: auto; flex: 1; padding-bottom: 16px; scrollbar-width: thin; scrollbar-color: var(--border) transparent; }}
    nav.sidebar h3 {{ color: var(--text); margin: 0; font-size: 0.9em; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; padding: 6px 4px; cursor: pointer; display: flex; align-items: center; gap: 4px; user-select: none; position: sticky; top: 0; background: var(--bg-surface); z-index: 1; }}
    nav.sidebar h3:hover {{ color: var(--primary); }}
    .cat-arrow {{ display: inline-block; font-size: 0.65em; transition: transform 0.15s; margin-right: 4px; }}
    .cat-arrow::before {{ content: '\25B6'; }}
    nav.sidebar h3:not(.collapsed) .cat-arrow {{ transform: rotate(90deg); }}
    .sidebar-group {{ }}
    .sidebar-group.collapsed {{ display: none; }}
    nav.sidebar a {{ display: block; color: var(--text-subtle); text-decoration: none; padding: 3px 4px 3px 12px; margin-bottom: 2px; border-radius: 4px; font-weight: 400; transition: background 0.15s, color 0.15s, border-color 0.15s; border-left: 3px solid transparent; }}
    nav.sidebar a:hover {{ color: var(--text); background: #1a1a1a; }}
    nav.sidebar a.active {{ color: var(--primary); background: rgba(240, 192, 64, 0.08); border-left-color: var(--primary); font-weight: 500; }}
    nav.sidebar a.hidden, nav.sidebar h3.hidden, nav.sidebar .sidebar-group.hidden {{ display: none; }}

    /* Search */
    #search {{
      width: 100%; padding: 8px 10px; margin-bottom: 12px;
      background: var(--bg); border: 1px solid var(--border); border-radius: 6px;
      color: var(--text); font-size: 0.9em; font-family: inherit; outline: none;
    }}
    #search:focus {{ border-color: var(--primary-dim); }}
    #search::placeholder {{ color: var(--text-muted); }}

    /* Main */
    main {{ margin-left: var(--sidebar-width); padding: 40px 56px; max-width: 960px; }}
    main > h1 {{ color: var(--primary); font-size: 2.8em; margin-bottom: 16px; }}
    main > .subtitle {{ color: var(--text-subtle); margin-bottom: 48px; font-size: 1.3em; font-weight: 420; line-height: 1.7; }}
    .category-heading {{ color: var(--primary); font-size: 1.4em; border-bottom: 1px solid var(--border); padding-bottom: 6px; margin-top: 56px; }}

    /* Entries */
    section.entry {{ margin-bottom: 56px; }}
    section.entry h2 {{ color: var(--text); font-size: 1.2em; margin-bottom: 4px; }}
    section.entry h2 a {{ color: var(--primary-dim); text-decoration: none; }}
    section.entry h2 a:hover {{ text-decoration: underline; }}
    section.entry h3 {{ color: var(--text-muted); font-size: 0.85em; font-weight: 600; text-transform: uppercase; letter-spacing: 0.04em; margin: 20px 0 8px; }}
    .description {{ color: var(--text-subtle); margin: 4px 0 16px; font-weight: 420; }}

    /* Code blocks (pre-Shiki fallback) */
    pre:has(> code.language-iecst) {{
      background: var(--bg-surface); border: 1px solid var(--border); border-radius: 6px; padding: 16px; overflow-x: auto; tab-size: 4; -moz-tab-size: 4;
    }}
    code.language-iecst {{ font-family: var(--mono); font-size: 14px; color: var(--text); }}

    /* Shiki overrides */
    .shiki {{ position: relative; border: 1px solid var(--border); border-radius: 6px; padding: 16px; overflow-x: auto; tab-size: 4; -moz-tab-size: 4; }}
    .shiki code {{ font-family: var(--mono); font-size: 14px; }}

    /* Copy button */
    .copy-btn {{
      position: absolute; top: 8px; right: 8px;
      background: var(--border); border: none; border-radius: 4px;
      color: var(--text-muted); cursor: pointer;
      padding: 4px 8px; font-size: 0.75em; font-family: inherit;
      opacity: 0; transition: opacity 0.15s;
    }}
    .shiki:hover .copy-btn, pre:hover .copy-btn {{ opacity: 1; }}
    .copy-btn:hover {{ background: var(--primary-dim); color: #000; }}
    pre:has(> code.language-iecst) {{ position: relative; }}

    /* Monaco-style inline diagnostics */
    .shiki .line {{ position: relative; }}

    .diag-squiggle {{
      position: absolute;
      bottom: 0;
      height: 3px;
      cursor: default;
      background: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='6' height='3'%3E%3Cpath d='M0 3 L1.5 0 L3 3 L4.5 0 L6 3' fill='none' stroke='%23f44658' stroke-width='0.7'/%3E%3C/svg%3E") repeat-x bottom left;
    }}
    .diag-squiggle.warning {{
      background: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='6' height='3'%3E%3Cpath d='M0 3 L1.5 0 L3 3 L4.5 0 L6 3' fill='none' stroke='%23e3b341' stroke-width='0.7'/%3E%3C/svg%3E") repeat-x bottom left;
    }}
    .diag-squiggle.info {{
      background: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='6' height='3'%3E%3Cpath d='M0 3 L1.5 0 L3 3 L4.5 0 L6 3' fill='none' stroke='%234da6ff' stroke-width='0.7'/%3E%3C/svg%3E") repeat-x bottom left;
    }}

    /* Hover zone over the full line height */
    .diag-hover {{
      position: absolute;
      bottom: 0;
      height: 100%;
      cursor: default;
      pointer-events: auto;
      z-index: 1;
    }}

    /* Shiki block: allow popups to overflow */
    .shiki {{ overflow: visible !important; }}

    /* Popup container inside the error span - hidden by default */
    .diag-popup {{
      position: absolute;
      left: 0;
      bottom: calc(100% + 4px);
      z-index: 100;
      opacity: 0;
      transition: opacity 0.15s;
      pointer-events: none;
      /* Monaco styling */
      background: #252526;
      color: #e0e0e0;
      border: 1px solid #454545;
      border-radius: 3px;
      box-shadow: 0 2px 8px rgba(0,0,0,0.36);
      font-family: Menlo, Monaco, Consolas, 'Droid Sans Mono', 'Courier New', monospace;
      font-size: 14px;
      font-weight: 450;
      line-height: 19px;
      -webkit-font-smoothing: antialiased;
      white-space: normal;
      cursor: default;
      user-select: text;
      padding: 4px 8px;
      width: max-content;
      max-width: 560px;
    }}

    .diag-hover:hover .diag-popup {{
      opacity: 1;
      pointer-events: auto;
    }}

    .diag-popup .marker-msg {{ white-space: pre-wrap; }}
    .diag-popup .marker-source {{ opacity: 0.6; padding-left: 6px; }}
    .diag-popup .code-link {{ color: #3794ff; }}
    .diag-popup .marker-note {{ white-space: pre-wrap; }}
    .diag-popup .marker-related {{ margin-top: 8px; }}
    .diag-popup .marker-related-loc {{ color: #3794ff; cursor: pointer; }}
    .diag-popup .marker-related-msg {{ white-space: pre-wrap; }}
    .diag-popup .marker-fix {{ white-space: pre-wrap; }}

    /* Diagnostic output */
    .diagnostic-output {{
      background: var(--bg-surface); border: 1px solid var(--border); border-radius: 6px; padding: 16px; overflow-x: auto; margin: 8px 0;
    }}
    .diagnostic-output pre {{
      margin: 0; font-family: var(--mono);
      font-size: 14px; line-height: 1.5; white-space: pre; color: var(--text);
    }}
    span.no-output {{ background: #1a1400; border: 1px solid #3d3000; padding: 12px 16px; border-radius: 6px; margin: 8px 0; display: block; color: var(--primary); font-style: italic; }}

    /* Back to top */
    .back-to-top {{
      position: fixed; bottom: 32px; right: 32px;
      width: 44px; height: 44px; border-radius: 50%;
      background: var(--primary); color: #000; border: none;
      font-size: 1.4em; cursor: pointer; display: flex;
      align-items: center; justify-content: center;
      box-shadow: 0 2px 12px rgba(0,0,0,0.4);
      opacity: 0; pointer-events: none;
      transition: opacity 0.2s;
      z-index: 10;
    }}
    .back-to-top.visible {{ opacity: 1; pointer-events: auto; }}
    .back-to-top:hover {{ background: var(--primary-dim); }}

    /* Stats */
    .stats {{ color: var(--text-muted); font-size: 0.85em; margin-top: 48px; padding-top: 16px; border-top: 1px solid var(--border); }}
  </style>
</head>
<body>
  <div class="layout">
    {sidebar}
    <main>
      <h1>RK - Diagnostics Reference</h1>
      <p class="subtitle">Compiler diagnostics for <strong>RK</strong>.<br>
      Each entry includes a description, example code, and actual compiler output.<br>
      Hover over underlined errors in code snippets to see how the diagnostic would be rendered in an <strong>IDE</strong>.<br>
      For AI agents: machine-readable API available at <a href="diagnostics.json" style="color: var(--primary)">diagnostics.json</a></p>
      {body}
      <p class="stats">Generated from the compiler source. {success} diagnostics documented, {fail} examples failed to produce output.</p>
    </main>
  </div>

  <button class="back-to-top" id="backToTop" onclick="window.scrollTo({{top:0,behavior:'smooth'}})" title="Back to top"><svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M18 15l-6-6-6 6"/></svg></button>

  <script id="tm-grammar" type="application/json">
{tm_grammar_json}
  </script>
  <script type="module">
    import {{ createHighlighter }} from 'https://esm.sh/shiki@1.29.2';

    try {{
      const grammarJson = JSON.parse(document.getElementById('tm-grammar').textContent);
      const iecstLang = {{
        name: 'iecst',
        scopeName: grammarJson.scopeName,
        patterns: grammarJson.patterns,
        repository: grammarJson.repository,
      }};

      const highlighter = await createHighlighter({{
        themes: ['dark-plus'],
        langs: [iecstLang],
      }});

      // Highlight each code block and inject inline diagnostics
      document.querySelectorAll('code.language-iecst').forEach(el => {{
        const pre = el.parentElement;
        const diagsRaw = pre.dataset.diags;
        const diags = diagsRaw ? JSON.parse(diagsRaw) : [];
        const code = el.textContent;

        // Replace with Shiki-highlighted HTML
        const html = highlighter.codeToHtml(code, {{ lang: 'iecst', theme: 'dark-plus' }});
        const tmp = document.createElement('div');
        tmp.innerHTML = html;
        const shikiPre = tmp.firstElementChild;
        pre.replaceWith(shikiPre);

        // Inject squiggles and messages into Shiki's line spans
        if (diags.length > 0) {{
          const codeEl = shikiPre.querySelector('code');
          const lines = codeEl.querySelectorAll('.line');

          // Group diags by line
          const byLine = {{}};
          for (const d of diags) {{
            (byLine[d.line] ??= []).push(d);
          }}

          // Inject squiggles with hover zones
          for (const [ln, diagList] of Object.entries(byLine)) {{
            const lineEl = lines[Number(ln)];
            if (!lineEl) continue;

            const lineLen = lineEl.textContent.length;
            for (const d of diagList) {{
              const w = d.sev === 'warning' ? ' warning' : d.sev === 'info' ? ' info' : '';
              const clampedEnd = Math.min(d.end, lineLen);
              const len = Math.max(1, clampedEnd - d.start);

              // Wavy underline
              const squig = document.createElement('span');
              squig.className = 'diag-squiggle' + w;
              squig.style.left = d.start + 'ch';
              squig.style.width = len + 'ch';
              lineEl.appendChild(squig);

              // Hover zone
              const hover = document.createElement('span');
              hover.className = 'diag-hover';
              hover.style.left = d.start + 'ch';
              hover.style.width = len + 'ch';
              hover.diagData = d;
              lineEl.appendChild(hover);
            }}
          }}
        }}
      }});

      // Add copy buttons to all code blocks
      document.querySelectorAll('.shiki, pre:has(> code.language-iecst)').forEach(pre => {{
        const btn = document.createElement('button');
        btn.className = 'copy-btn';
        btn.textContent = 'Copy';
        btn.addEventListener('click', () => {{
          const code = pre.querySelector('code')?.textContent || pre.textContent;
          navigator.clipboard.writeText(code).then(() => {{
            btn.textContent = 'Copied!';
            setTimeout(() => btn.textContent = 'Copy', 1500);
          }});
        }});
        pre.appendChild(btn);
      }});
    }} catch (e) {{
      console.warn('Shiki highlighting failed, falling back to plain text:', e);
    }}

    // Build popup HTML inline inside each .diag-hover (pure CSS show/hide like twoslash)
    function esc(s) {{ return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;'); }}

    document.querySelectorAll('.diag-hover').forEach(hover => {{
      const d = hover.diagData;
      if (!d) return;

      let html = '<span class="diag-popup">';
      html += '<span class="marker-msg">' + esc(d.msg) + '</span>';

      if (d.code) {{
        html += '<span class="marker-source">rk(<span class="code-link">' + esc(d.code) + '</span>)';
        if (d.desc) html += ' - ' + esc(d.desc);
        html += '</span>';
      }}

      if (d.notes && d.notes.length) {{
        for (const n of d.notes) {{
          html += '<div class="marker-note">Note: ' + esc(n) + '</div>';
        }}
      }}

      if (d.related && d.related.length) {{
        for (const r of d.related) {{
          html += '<div class="marker-related"><span class="marker-related-loc">example.st(' + (r.line + 1) + ', ' + (r.col + 1) + '): </span><span class="marker-related-msg">' + esc(r.msg) + '</span></div>';
        }}
      }}

      if (d.fixes && d.fixes.length) {{
        for (const f of d.fixes) {{
          html += '<div class="marker-fix">Help: ' + esc(f) + '</div>';
        }}
      }}

      html += '</span>';
      hover.insertAdjacentHTML('beforeend', html);
    }});
  </script>
  <script>
    const search = document.getElementById('search');
    const entries = document.querySelectorAll('.sidebar-entry');
    const cats = document.querySelectorAll('.sidebar-cat');
    const groups = document.querySelectorAll('.sidebar-group');

    search.addEventListener('input', () => {{
      const q = search.value.toLowerCase().trim();
      const visibleCats = new Set();

      entries.forEach(a => {{
        const code = (a.dataset.code || '').toLowerCase();
        const title = (a.dataset.title || '').toLowerCase();
        const match = !q || code.includes(q) || title.includes(q);
        a.classList.toggle('hidden', !match);
        if (match) {{
          const group = a.closest('.sidebar-group');
          if (group) {{
            const cat = group.previousElementSibling;
            if (cat) visibleCats.add(cat);
          }}
        }}
      }});

      cats.forEach(h => {{
        const visible = !q || visibleCats.has(h);
        h.classList.toggle('hidden', !visible);
        const group = h.nextElementSibling;
        if (group && group.classList.contains('sidebar-group')) {{
          group.classList.toggle('hidden', !visible);
          // Expand matching groups while searching
          if (q && visible) {{
            h.classList.remove('collapsed');
            group.classList.remove('collapsed');
          }}
        }}
      }});
    }});

    // Focus search on / key
    document.addEventListener('keydown', e => {{
      if (e.key === '/' && document.activeElement !== search) {{
        e.preventDefault();
        search.focus();
      }}
    }});

    // Highlight active sidebar entry based on scroll position
    const sidebarLinks = Object.fromEntries(
      [...document.querySelectorAll('.sidebar-entry')].map(a => [a.dataset.code, a])
    );
    let activeLink = null;

    const allGroups = [...document.querySelectorAll('.sidebar-group')];
    const allCats = [...document.querySelectorAll('.sidebar-cat')];

    function collapseAll() {{
      allGroups.forEach(g => g.classList.add('collapsed'));
      allCats.forEach(c => c.classList.add('collapsed'));
    }}

    const observer = new IntersectionObserver(items => {{
      for (const item of items) {{
        if (item.isIntersecting) {{
          const id = item.target.id;
          if (activeLink) activeLink.classList.remove('active');
          const link = sidebarLinks[id];
          if (link) {{
            link.classList.add('active');
            activeLink = link;
            // Collapse all, then expand only the active group
            const group = link.closest('.sidebar-group');
            if (group) {{
              collapseAll();
              group.classList.remove('collapsed');
              const cat = group.previousElementSibling;
              if (cat) cat.classList.remove('collapsed');
            }}
            link.scrollIntoView({{ block: 'nearest' }});
          }}
        }}
      }}
    }}, {{ rootMargin: '-10% 0px -70% 0px' }});

    document.querySelectorAll('section.entry').forEach(s => observer.observe(s));

    // Back to top button
    const topBtn = document.getElementById('backToTop');
    window.addEventListener('scroll', () => {{
      topBtn.classList.toggle('visible', window.scrollY > 400);
    }}, {{ passive: true }});
  </script>
</body>
</html>"##,
        sidebar = sidebar,
        body = body,
        tm_grammar_json = tm_grammar_json,
        success = success,
        fail = fail,
    )
}
