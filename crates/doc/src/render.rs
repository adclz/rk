use ariadne::{CharSet, Config, FnCache, Source};
use auto_lsp::{
    default::db::{BaseDatabase, FileManager, file::File},
    lsp_types::Url,
};
use db::RootDatabase;
use hir::check::diagnostics_for_file;

/// Compile the given ST sources and return the ariadne report as a string
/// with ANSI color codes.
pub fn compile_and_render(db: &mut RootDatabase, sources_str: &[&str], run_linter: bool) -> String {
    for (i, source) in sources_str.iter().enumerate() {
        let url = Url::parse(&format!("file:///example{i}.st")).unwrap();
        let file = File::from_string()
            .db(db)
            .parsers(ast::RK_PARSER.get("structured_text").unwrap())
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();
        db.add_file(file).unwrap();
    }

    let mut output = Vec::new();

    let mut files: Vec<File> = db
        .get_files()
        .iter()
        .map(|entry| *entry.value())
        .collect();
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

    let linter_config = db::config_file::LinterConfig::default();

    for file in &files {
        let mut all = diagnostics_for_file(db, *file).as_ref().clone();
        if run_linter {
            linter::lint_file(db, *file, &linter_config, &mut all);
        }
        for d in all.iter() {
            d.create_report(
                db,
                file.url(db),
                file.document(db).as_str(),
                Some(config.clone()),
                true,
            )
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

/// Render a single error/warning entry as an HTML fragment.
pub fn render_entry_html(
    code: &str,
    title: &str,
    description: &str,
    sources: &[&str],
    report_html: &str,
) -> String {
    let mut html = format!(
        r##"<section class="entry" id="{code}">
  <h2><a href="#{code}">{code}</a> &mdash; {title}</h2>
  <p class="description">{description}</p>
  <h3>Example</h3>
"##
    );

    for source in sources {
        let source_trimmed = source
            .trim_matches('\n')
            .lines()
            .collect::<Vec<_>>()
            .join("\n");
        html.push_str(&format!(
            "  <pre><code class=\"language-iecst\">{}</code></pre>\n",
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
    html.push_str(r#"<div class="sidebar-title">IEC 61131-3<br>Diagnostics</div>"#);

    for (category, entries) in categories {
        let slug = category.to_lowercase().replace(' ', "-");
        html.push_str(&format!(
            "<h3><a href=\"#cat-{slug}\">{category}</a></h3>"
        ));
        for (code, title) in entries {
            html.push_str(&format!(
                "<a href=\"#{code}\" title=\"{title}\">{code}</a>"
            ));
        }
    }

    html.push_str("</nav>");
    html
}

/// Assemble the full single-page HTML document.
pub fn render_page(sidebar: &str, body: &str, tm_grammar_json: &str, success: usize, fail: usize) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>IEC 61131-3 Diagnostics Reference</title>
  <style>
    :root {{ --sidebar-width: 260px; }}
    * {{ box-sizing: border-box; }}
    body {{ margin: 0; font-family: 'Inter', system-ui, -apple-system, sans-serif; background: #0f1117; color: #c9d1d9; line-height: 1.6; }}
    .layout {{ display: flex; min-height: 100vh; }}

    /* Sidebar */
    nav.sidebar {{
      width: var(--sidebar-width); position: fixed; top: 0; left: 0; bottom: 0;
      overflow-y: auto; background: #161b22; padding: 16px 12px;
      border-right: 1px solid #30363d; font-size: 0.82em;
      scrollbar-width: thin; scrollbar-color: #30363d transparent;
    }}
    .sidebar-title {{ font-weight: 700; font-size: 1.1em; color: #58a6ff; padding: 8px 4px 16px; border-bottom: 1px solid #30363d; margin-bottom: 8px; }}
    nav.sidebar h3 {{ color: #8b949e; margin: 16px 0 6px; font-size: 0.8em; text-transform: uppercase; letter-spacing: 0.05em; padding-left: 4px; }}
    nav.sidebar h3 a {{ color: inherit; text-decoration: none; }}
    nav.sidebar h3 a:hover {{ color: #c9d1d9; }}
    nav.sidebar a {{ display: block; color: #8b949e; text-decoration: none; padding: 2px 4px 2px 12px; border-radius: 4px; }}
    nav.sidebar a:hover {{ color: #c9d1d9; background: #1c2128; }}

    /* Main */
    main {{ margin-left: var(--sidebar-width); padding: 40px 56px; max-width: 960px; }}
    main > h1 {{ color: #58a6ff; font-size: 1.8em; margin-bottom: 8px; }}
    main > .subtitle {{ color: #8b949e; margin-bottom: 40px; font-size: 0.95em; }}
    .category-heading {{ color: #58a6ff; font-size: 1.4em; border-bottom: 1px solid #30363d; padding-bottom: 6px; margin-top: 56px; }}

    /* Entries */
    section.entry {{ margin-bottom: 56px; }}
    section.entry h2 {{ color: #c9d1d9; font-size: 1.2em; margin-bottom: 4px; }}
    section.entry h2 a {{ color: inherit; text-decoration: none; }}
    section.entry h2 a:hover {{ text-decoration: underline; }}
    section.entry h3 {{ color: #8b949e; font-size: 0.85em; text-transform: uppercase; letter-spacing: 0.04em; margin: 20px 0 8px; }}
    .description {{ color: #8b949e; margin: 4px 0 16px; }}

    /* Code blocks (pre-Shiki fallback) */
    pre:has(> code.language-iecst) {{
      background: #161b22; border: 1px solid #30363d; border-radius: 6px; padding: 16px; overflow-x: auto;
    }}
    code.language-iecst {{ font-family: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace; font-size: 13px; color: #c9d1d9; }}

    /* Shiki overrides */
    .shiki {{ border: 1px solid #30363d; border-radius: 6px; padding: 16px; overflow-x: auto; }}
    .shiki code {{ font-family: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace; font-size: 13px; }}

    /* Diagnostic output */
    .diagnostic-output {{
      background: #161b22; border: 1px solid #30363d; border-radius: 6px; padding: 16px; overflow-x: auto; margin: 8px 0;
    }}
    .diagnostic-output pre {{
      margin: 0; font-family: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace;
      font-size: 13px; line-height: 1.5; white-space: pre; color: #c9d1d9;
    }}
    .warning {{ background: #2d1f00; border-color: #614a00; padding: 12px 16px; border-radius: 6px; margin: 8px 0; color: #e3b341; font-style: italic; }}

    /* Stats */
    .stats {{ color: #8b949e; font-size: 0.85em; margin-top: 48px; padding-top: 16px; border-top: 1px solid #30363d; }}
  </style>
</head>
<body>
  <div class="layout">
    {sidebar}
    <main>
      <h1>IEC 61131-3 Diagnostics Reference</h1>
      <p class="subtitle">Compiler diagnostics for the <strong>rk</strong> IEC 61131-3 Structured Text compiler.<br>
      Each entry includes a description, example code, and actual compiler output.</p>
      {body}
      <p class="stats">Generated from the compiler source. {success} diagnostics documented, {fail} examples failed to produce output.</p>
    </main>
  </div>

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

      document.querySelectorAll('code.language-iecst').forEach(el => {{
        const code = el.textContent;
        const html = highlighter.codeToHtml(code, {{ lang: 'iecst', theme: 'dark-plus' }});
        el.parentElement.outerHTML = html;
      }});
    }} catch (e) {{
      console.warn('Shiki highlighting failed, falling back to plain text:', e);
    }}
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
