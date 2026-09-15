//! Builds the site: the skills as documentation, the diagnostics reference,
//! and the files agents look for. Refuses to write anything when an example
//! disagrees with the compiler.
//!
//!     cargo run --release -p doc -- site/dist [--base-url https://…]
//!
//! Also refreshes the committed `crates/doc/diagnostics.json`, which
//! `rk explain` embeds at build time.

mod examples;
mod highlight;
mod markdown;
mod render;
mod site;
mod skills;
mod verify;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use db::RootDatabase;

use crate::highlight::{Mark, StHighlighter, escape, mark_html};
use crate::render::DiagSpan;
use crate::site::{DiagCategory, Page, one_line, strip_ansi, write};

struct DiagEntry {
    code: &'static str,
    category: &'static str,
    title: &'static str,
    description: &'static str,
    sources: &'static [&'static str],
    report_html: String,
    report_text: String,
    spans: Vec<DiagSpan>,
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// The reference's sections in reading order. A code's first two digits name
/// its section, so a new code is filed by its number and nothing else.
const SECTIONS: &[(&str, &str)] = &[
    ("E00", "Syntax"),
    ("E01", "Duplicates"),
    ("E02", "Resolution"),
    ("E03", "Type System"),
    ("E04", "Initializers"),
    ("E05", "Arrays"),
    ("E06", "Enums"),
    ("E07", "Subranges"),
    ("E08", "Calls"),
    ("E09", "References"),
    ("E10", "Visibility"),
    ("E11", "OOP"),
    ("E12", "Control Flow"),
    ("E13", "Recursion"),
    ("E14", "Configuration"),
    ("E15", "Pragmas"),
    ("L00", "Lint pragmas"),
    ("L01", "Linter Warning"),
    ("L02", "Linter Info"),
    ("L03", "Linter Hint"),
];

fn section_of(code: &str) -> &'static str {
    SECTIONS
        .iter()
        .find(|(prefix, _)| code.starts_with(prefix))
        .map(|(_, name)| *name)
        .unwrap_or_else(|| panic!("{code} is outside every section range"))
}

fn slug(category: &str) -> String {
    category.to_lowercase().replace(' ', "-")
}

fn main() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = crate_dir.join("../..").canonicalize().unwrap();

    let mut args = std::env::args().skip(1);
    let mut out_dir = repo.join("site/dist");
    let mut base_url = std::env::var("SITE_URL").unwrap_or_else(|_| "http://localhost:8788".into());
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--base-url" => base_url = args.next().expect("--base-url needs a value"),
            other => out_dir = PathBuf::from(other),
        }
    }
    let base_url = base_url.trim_end_matches('/').to_string();

    // ── Diagnostics: every example must produce the code it documents ──
    let examples = examples::all_examples();
    for ex in &examples {
        assert_eq!(
            ex.category,
            section_of(ex.code),
            "{}: the code's range says `{}`, the example says `{}`",
            ex.code,
            section_of(ex.code),
            ex.category
        );
    }
    let mut entries: Vec<DiagEntry> = Vec::new();
    let mut produced: Vec<(&str, std::collections::BTreeSet<String>)> = Vec::new();
    eprintln!("Diagnostics");
    for ex in &examples {
        eprint!("  {}...", ex.code);
        // Described without a runnable example: about the workspace, not a
        // file. Listed all the same, so `rk explain` has an answer.
        if ex.sources.is_empty() {
            let text = "No source example: this diagnostic is about the workspace, not a file.";
            entries.push(DiagEntry {
                code: ex.code,
                category: ex.category,
                title: ex.title,
                description: ex.description,
                sources: ex.sources,
                report_html: format!("<p>{text}</p>"),
                report_text: text.to_string(),
                spans: Vec::new(),
            });
            eprintln!(" described, no example");
            continue;
        }
        let mut db = RootDatabase::default();
        let (ansi, spans) = render::compile_and_render(&mut db, ex.sources, ex.lint_rule);
        produced.push((ex.code, verify::codes_in_output(&ansi)));
        entries.push(DiagEntry {
            code: ex.code,
            category: ex.category,
            title: ex.title,
            description: ex.description,
            sources: ex.sources,
            report_html: render::ansi_to_html_fragment(&ansi),
            report_text: strip_ansi(&ansi),
            spans,
        });
        eprintln!(" ok");
    }
    let problems = verify::problems(&examples, &produced);
    if !problems.is_empty() {
        eprintln!(
            "\nThe diagnostics reference disagrees with the compiler in {} place(s); nothing was written.\n",
            problems.len()
        );
        for p in &problems {
            eprintln!("  {p}");
        }
        eprintln!(
            "\nEach example must produce the diagnostic it documents, and every code the compiler\ndefines must have one. Fix the example (or the code), then re-run."
        );
        std::process::exit(1);
    }

    // ── Skills: every fence must hold ─────────────────────────────────
    let highlighter = StHighlighter::new();
    let skills = skills::discover(&repo.join("skills"), &highlighter);
    eprintln!("Skills");
    let problems = skills::verify(&skills);
    if !problems.is_empty() {
        eprintln!(
            "\n{} example(s) in the skills do not hold; nothing was written.\n",
            problems.len()
        );
        for p in &problems {
            eprintln!("  {p}");
        }
        eprintln!(
            "\nSee crates/doc/src/skills.rs for the fence markers (fragment, decl, continues, syntax, expect=)."
        );
        std::process::exit(1);
    }

    // ── Everything agreed: write ──────────────────────────────────────
    if out_dir.exists() {
        fs::remove_dir_all(&out_dir).unwrap();
    }
    fs::create_dir_all(&out_dir).unwrap();

    let mut sitemap: Vec<String> = Vec::new();
    let mut twins: Vec<(String, String)> = Vec::new();

    // Sections in reading order (SECTIONS), entries sorted by code within
    // each: the order the pages use, and the order the committed JSON keeps.
    let mut by_category: BTreeMap<&str, Vec<&DiagEntry>> = BTreeMap::new();
    for e in &entries {
        by_category.entry(e.category).or_default().push(e);
    }
    let order: Vec<&str> = SECTIONS
        .iter()
        .map(|(_, name)| *name)
        .filter(|name| by_category.contains_key(name))
        .collect();
    for list in by_category.values_mut() {
        list.sort_by_key(|e| e.code);
    }
    let categories: Vec<DiagCategory> = order
        .iter()
        .map(|c| DiagCategory {
            name: c.to_string(),
            slug: slug(c),
            count: by_category[c].len(),
        })
        .collect();

    // diagnostics.json: committed beside the generator and served by the site.
    let json = {
        let items: Vec<String> = order
            .iter()
            .flat_map(|c| by_category[c].iter().copied())
            .map(|e| {
                let sources: Vec<String> = e
                    .sources
                    .iter()
                    .map(|s| format!("\"{}\"", json_escape(s.trim_matches('\n'))))
                    .collect();
                format!(
                    r#"  {{"code":"{}","category":"{}","title":"{}","description":"{}","sources":[{}]}}"#,
                    json_escape(e.code),
                    json_escape(e.category),
                    json_escape(e.title),
                    json_escape(e.description),
                    sources.join(",")
                )
            })
            .collect();
        format!("[\n{}\n]\n", items.join(",\n"))
    };
    fs::write(crate_dir.join("diagnostics.json"), &json).unwrap();
    write(&out_dir, "diagnostics.json", &json);

    // The reference: one page for humans, with a sidebar, a search box and
    // the compiler's own markers on every example; a Markdown twin per
    // category and per code for agents.
    {
        let mut side = String::from(
            "<nav class=\"side\" aria-label=\"Diagnostics\">\n<input id=\"search\" type=\"search\" placeholder=\"E0301, duplicate, … (/)\" autocomplete=\"off\">\n",
        );
        let mut body = String::new();
        let mut index_md = String::from(
            "# Diagnostics\n\nEvery code the compiler and the linter can report, each with a compiler-verified example.\n\n",
        );
        for c in &categories {
            side.push_str(&format!(
                "<details open data-cat=\"{s}\"><summary>{n}</summary>\n",
                s = c.slug,
                n = escape(&c.name)
            ));
            body.push_str(&format!(
                "<h1 class=\"category-heading\" id=\"cat-{s}\" data-cat=\"{s}\">{n}</h1>\n",
                s = c.slug,
                n = escape(&c.name)
            ));
            let mut md = format!("# Diagnostics: {}\n\n", c.name);
            for e in &by_category[c.name.as_str()] {
                side.push_str(&format!(
                    "<a href=\"#{code}\" data-code=\"{code}\">{code}</a>\n",
                    code = e.code
                ));
                let text = format!("{} {} {}", e.code, e.title, e.description).to_lowercase();
                body.push_str(&format!(
                    "<section class=\"entry\" id=\"{code}\" data-cat=\"{cat}\" data-text=\"{text}\">\n<h2><a href=\"#{code}\">{code}</a> {title}</h2>\n<p class=\"description\">{desc}</p>\n",
                    code = e.code, cat = c.slug, text = escape(&text), title = escape(e.title), desc = escape(e.description)
                ));
                let mut entry_md = format!("## {} {}\n\n{}\n\n", e.code, e.title, e.description);
                for (i, s) in e.sources.iter().enumerate() {
                    let s = s.trim_matches('\n');
                    let marks: Vec<Mark> = e
                        .spans
                        .iter()
                        .filter(|d| d.source_idx == i && d.start < s.len())
                        .map(|d| Mark {
                            start: d.start,
                            end: d.end.min(s.len()),
                            class: d.severity.to_string(),
                            title: escape(&d.message),
                            popup: popup_html(d),
                        })
                        .collect();
                    body.push_str(&format!(
                        "<div class=\"codewrap\"><pre><code class=\"language-iecst\">{}\n</code></pre></div>\n",
                        mark_html(&highlighter.html(s), &marks)
                    ));
                    entry_md.push_str(&format!("```iecst\n{s}\n```\n\n"));
                }
                body.push_str(&format!(
                    "<h3>Compiler output</h3>\n<pre class=\"output\">{}</pre>\n</section>\n",
                    e.report_html
                ));
                entry_md.push_str(&format!(
                    "Compiler output:\n\n```\n{}```\n\n",
                    e.report_text
                ));
                let (_, rest) = entry_md.split_once("\n\n").unwrap();
                write(
                    &out_dir,
                    &format!("diagnostics/{}.md", e.code),
                    &format!("# {} {}\n\n{}", e.code, e.title, rest),
                );
                md.push_str(&entry_md);
            }
            side.push_str("</details>\n");
            write(&out_dir, &format!("diagnostics/{}.md", c.slug), &md);
            index_md.push_str(&format!(
                "- [{}]({}/diagnostics/{}.md): {} codes\n",
                c.name, base_url, c.slug, c.count
            ));
        }
        side.push_str("</nav>\n");
        let page = format!(
            "<h1>Diagnostics</h1>\n<p class=\"lede\">Every code the compiler and the linter can report, each with the example that produces it and the compiler's own output. Hover a marked range for the message. The generator runs every example before publishing, so this page never disagrees with the binary.</p>\n<p><br>The same data as <a href=\"/diagnostics.json\">JSON</a>, which <code>rk explain &lt;code&gt;</code> embeds.</p>\n<div class=\"ref\">\n{side}<div>\n{body}</div>\n</div>\n<a id=\"top\" href=\"#\">top</a>\n"
        );
        write(&out_dir, "diagnostics/index.md", &index_md);
        write(
            &out_dir,
            "diagnostics/index.html",
            &site::shell_with(
                &Page {
                    title: "Diagnostics",
                    description: "Every diagnostic code rk reports, with compiler-verified examples.",
                    path: "/diagnostics/",
                    md: Some("/diagnostics/index.md"),
                    eyebrow: "reference",
                    body: &page,
                    wide: true,
                },
                &base_url,
                site::REFERENCE_JS,
            ),
        );
        sitemap.push("/diagnostics/".into());
        twins.push(("/diagnostics/".into(), "/diagnostics/index.md".into()));
    }

    // The linter page. The rule table is DERIVED: names come from each
    // example's `lint_rule`, severities from what the compiler actually
    // printed, and the default column from the linter's own recommended
    // set — so it cannot drift from the binary the way a hand-kept table does.
    {
        let severity_of = |e: &DiagEntry| -> &'static str {
            let marker = format!("[{}] ", e.code);
            e.report_text
                .find(&marker)
                .map(|i| &e.report_text[i + marker.len()..])
                .and_then(|rest| rest.split(':').next())
                .map(|word| match word.trim() {
                    "Warning" => "warning",
                    "Info" => "info",
                    "Hint" => "hint",
                    other if other.eq_ignore_ascii_case("error") => "error",
                    _ => "info",
                })
                .unwrap_or("info")
        };
        let rule_of = |code: &str| -> Option<&'static str> {
            examples
                .iter()
                .find(|ex| ex.code == code)
                .and_then(|ex| ex.lint_rule)
        };
        let groups: [(&str, &str, &str); 5] = [
            (
                "L00",
                "Pragmas",
                "Misuse of a <code>{…}</code> pragma, and the notices <code>{info}</code> and <code>{warn}</code> raise on purpose.",
            ),
            (
                "L01",
                "Declarations",
                "Declarations that are unused, shadowed or redundant.",
            ),
            (
                "L02",
                "Style",
                "Matters of taste. Off unless you ask for them.",
            ),
            ("L03", "Suspicious code", "Probably a bug. On by default."),
            (
                "L04",
                "Globals",
                "Reaching a CONFIGURATION global without declaring it. On by default.",
            ),
        ];
        let mut body = String::from(
            "<h1>Linter</h1>\n<p class=\"lede\">Rules that read the same tree the compiler does, so a lint knows what a name means rather than how it is spelled.</p>\n",
        );
        body.push_str("<p>Lints appear in <code>rk check</code> and in your editor. They never run during <code>rk compile</code> or <code>rk test</code>, and they never change an exit code, so a lint cannot block a build or fail a pipeline on its own.</p>\n");
        body.push_str("<h2>Configuration</h2>\n<p>The linter is on by default with the <em>recommended</em> set: the rules that report a probable bug rather than a preference. A workspace that never mentions the linter still gets them. <code>[linter]</code> tunes that set, it does not switch the linter on.</p>\n");
        body.push_str("<div class=\"tablewrap\"><table><thead><tr><th><code>select</code></th><th>Rules that run</th></tr></thead><tbody>\n<tr><td>absent</td><td>the recommended set</td></tr>\n<tr><td><code>\"recommended\"</code></td><td>the same, said out loud</td></tr>\n<tr><td><code>\"all\"</code></td><td>every rule below</td></tr>\n<tr><td><code>\"none\"</code></td><td>none, unless <code>[linter.rules]</code> names one</td></tr>\n</tbody></table></div>\n");
        body.push_str(&format!(
            "<pre><code class=\"language-toml\">{}</code></pre>\n",
            crate::highlight::toml_html(
                "[linter]\nselect = \"all\"           # the default is \"recommended\"\n\n[linter.rules]\nyoda-condition = false   # opt out of one that select turned on"
            )
        ));
        body.push_str("<p><code>[linter.rules]</code> overrides <code>select</code> both ways, so a style rule can be adopted one at a time rather than all at once. An unknown <code>select</code> value is a configuration error naming the three that exist; an unknown rule <em>name</em> is accepted and does nothing.</p>\n");
        body.push_str("<h2>Silencing one place</h2>\n<p><code>{allow 'rule-name'}</code> silences a rule exactly where the code is deliberate, instead of turning it off everywhere. Above a POU it covers that POU; as a statement it covers the next statement and everything nested in it. One pragma takes several names. A name that does not exist is reported as <code>L0005</code> and silences nothing, because a typo must not silence the typo.</p>\n");
        body.push_str(&format!(
            "<pre><code class=\"language-iecst\">{}</code></pre>\n",
            highlighter.html(
                "{allow 'input-assignment'}\nFUNCTION_BLOCK Rebinder\n\t…\nEND_FUNCTION_BLOCK\n\n\t{allow 'missing-input-param'}\n\tmb(REQ := TRUE, MODE := USINT#1);"
            )
        ));

        let mut md = String::from(
            "# Linter\n\nRules that read the same tree the compiler does. Lints appear in `rk check` and in your editor; they never run during `rk compile` or `rk test`, and they never change an exit code.\n\nThe linter is on by default with the recommended set. `[linter] select` takes `\"recommended\"` (the default), `\"all\"` or `\"none\"`, and `[linter.rules]` overrides it either way. `{allow 'rule-name'}` silences one place.\n\n",
        );

        for (prefix, title, blurb) in groups {
            let mut rows: Vec<&DiagEntry> = entries
                .iter()
                .filter(|e| e.code.starts_with(prefix))
                .collect();
            rows.sort_by_key(|e| e.code);
            if rows.is_empty() {
                continue;
            }
            body.push_str(&format!("<h2>{title}</h2>\n<p>{blurb}</p>\n"));
            body.push_str("<div class=\"tablewrap\"><table><thead><tr><th>Code</th><th>Rule</th><th>Severity</th><th>Default</th><th>What it catches</th></tr></thead><tbody>\n");
            md.push_str(&format!("## {title}\n\n| Code | Rule | Severity | Default | What it catches |\n| --- | --- | --- | --- | --- |\n"));
            for e in rows {
                let rule = rule_of(e.code).unwrap_or("");
                let on = linter::RECOMMENDED_RULE_NAMES.contains(&rule);
                let sev = severity_of(e);
                body.push_str(&format!(
                    "<tr><td class=\"k\"><a href=\"/diagnostics/#{code}\">{code}</a></td><td class=\"k\">{rule}</td><td>{sev}</td><td>{on}</td><td>{what}</td></tr>\n",
                    code = e.code,
                    rule = escape(rule),
                    sev = sev,
                    on = if on { "on" } else { "—" },
                    what = escape(e.description),
                ));
                md.push_str(&format!(
                    "| {} | `{}` | {} | {} | {} |\n",
                    e.code,
                    rule,
                    sev,
                    if on { "on" } else { "—" },
                    e.description.replace('|', "\\|")
                ));
            }
            body.push_str("</tbody></table></div>\n");
            md.push('\n');
        }
        write(&out_dir, "linter/index.md", &md);
        write(
            &out_dir,
            "linter/index.html",
            &site::shell(
                &Page {
                    title: "Linter",
                    description: "The lint rules rk applies, what each one catches, and how to configure or silence it.",
                    path: "/linter/",
                    md: Some("/linter/index.md"),
                    eyebrow: "tools",
                    body: &body,
                    wide: false,
                },
                &base_url,
            ),
        );
        sitemap.push("/linter/".into());
        twins.push(("/linter/".into(), "/linter/index.md".into()));
    }

    // The formatter page.
    {
        let body = site::formatter_html(&highlighter);
        let md = "# Formatter\n\n`rk fmt` rewrites every .st file; `rk fmt --check` reports what would change and exits 1 if anything would. In an editor it is the language server's Format Document.\n\nIt works on the parsed syntax tree, not the text, so it cannot produce a file that no longer parses, and it refuses a file that does not parse going in. Its suite formats twice and requires the second pass to change nothing; CI reformats the standard library and the grammar's fixtures on every change and checks that meaning never moved.\n\nIt has no line-width target and never reflows expressions: a long condition stays on one line if that is how you wrote it. For parameter lists and initialisers a line break inside the list is the instruction, so a list written on one line stays inline and a list containing a newline is expanded one element per line, with the closing bracket at the statement's indent. The parser tolerates a missing semicolon and the formatter writes it in, so every declaration, statement and directive comes back terminated, and a `USING` naming several namespaces takes one terminator at the end rather than one per name. What it normalises is indentation (one tab per level, every block), declarations (one per line, one space after the colon), spacing (one space around binary operators and assignment, none around `.`, `#` or `[]`), and it keeps comments where you put them. Comments and the insides of string literals are never touched. A blank line between declarations is kept as a paragraph break; several in a row collapse to one.\n";
        write(&out_dir, "formatter/index.md", md);
        write(
            &out_dir,
            "formatter/index.html",
            &site::shell(
                &Page {
                    title: "Formatter",
                    description: "How rk fmt formats Structured Text, and what it deliberately leaves alone.",
                    path: "/formatter/",
                    md: Some("/formatter/index.md"),
                    eyebrow: "tools",
                    body: &body,
                    wide: false,
                },
                &base_url,
            ),
        );
        sitemap.push("/formatter/".into());
        twins.push(("/formatter/".into(), "/formatter/index.md".into()));
    }

    // Skills: raw files copied byte-identical, a rendered page beside each.
    let mut skills_json: Vec<String> = Vec::new();
    for skill in &skills {
        let dir = format!("skills/{}", skill.name);
        write(&out_dir, &format!("{dir}/SKILL.md"), &skill.text);
        let mut files = vec!["SKILL.md".to_string()];
        let mut body = format!(
            "<h1>{}</h1>\n<p class=\"lede\">{}</p>\n<p><a class=\"pill\" href=\"/skills/{n}/SKILL.md\">SKILL.md</a> <a class=\"pill\" href=\"/skills.tar.gz\">install</a></p>\n",
            escape(&skill.name),
            escape(&one_line(&skill.description)),
            n = skill.name
        );
        body.push_str(&skill.html);
        if !skill.references.is_empty() {
            body.push_str("<h2>References</h2>\n<p>Loaded on demand by the skill.</p>\n<ul>\n");
            for r in &skill.references {
                let stem = r
                    .rel
                    .trim_start_matches("references/")
                    .trim_end_matches(".md");
                body.push_str(&format!(
                    "<li><a href=\"/skills/{n}/references/{stem}/\">{}</a></li>\n",
                    escape(&r.title),
                    n = skill.name
                ));
            }
            body.push_str("</ul>\n");
        }
        let html_path = format!("/skills/{}/", skill.name);
        let md_path = format!("/skills/{}/SKILL.md", skill.name);
        write(
            &out_dir,
            &format!("{dir}/index.html"),
            &site::shell(
                &Page {
                    title: &skill.name,
                    description: &one_line(&skill.description),
                    path: &html_path,
                    md: Some(&md_path),
                    eyebrow: &format!("skill · {}", skill.group()),
                    body: &body,
                    wide: false,
                },
                &base_url,
            ),
        );
        sitemap.push(html_path.clone());
        twins.push((html_path, md_path));

        for r in &skill.references {
            write(&out_dir, &format!("{dir}/{}", r.rel), &r.text);
            files.push(r.rel.clone());
            let stem = r
                .rel
                .trim_start_matches("references/")
                .trim_end_matches(".md");
            let html_path = format!("/skills/{}/references/{stem}/", skill.name);
            let md_path = format!("/skills/{}/{}", skill.name, r.rel);
            let body = format!(
                "<p><a href=\"/skills/{n}/\">{n}</a> / references</p>\n{}",
                r.html,
                n = skill.name
            );
            write(
                &out_dir,
                &format!("{dir}/references/{stem}/index.html"),
                &site::shell(
                    &Page {
                        title: &format!("{} · {}", skill.name, r.title),
                        description: &format!("Reference material for the {} skill.", skill.name),
                        path: &html_path,
                        md: Some(&md_path),
                        eyebrow: "reference",
                        body: &body,
                        wide: false,
                    },
                    &base_url,
                ),
            );
            sitemap.push(html_path.clone());
            twins.push((html_path, md_path));
        }
        skills_json.push(format!(
            r#"  {{"name":"{}","group":"{}","description":"{}","files":[{}]}}"#,
            json_escape(&skill.name),
            json_escape(skill.group()),
            json_escape(&one_line(&skill.description)),
            files
                .iter()
                .map(|f| format!("\"{}\"", json_escape(f)))
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    write(
        &out_dir,
        "skills.json",
        &format!("[\n{}\n]\n", skills_json.join(",\n")),
    );

    // Skills index.
    {
        let mut body = String::from(
            "<h1>Skills</h1>\n<p class=\"lede\">Each skill is a folder an agent loads when a task matches its description. Together they are the language and toolchain documentation.</p>\n",
        );
        body.push_str(&skill_list_html(&skills));
        let mut md = String::from("# Skills\n\n");
        for s in &skills {
            md.push_str(&format!(
                "- [{}]({}/skills/{}/SKILL.md): {}\n",
                s.name,
                base_url,
                s.name,
                one_line(&s.description)
            ));
        }
        write(&out_dir, "skills/index.md", &md);
        write(
            &out_dir,
            "skills/index.html",
            &site::shell(
                &Page {
                    title: "Skills",
                    description: "The rk documentation as Agent Skills.",
                    path: "/skills/",
                    md: Some("/skills/index.md"),
                    eyebrow: "documentation",
                    body: &body,
                    wide: false,
                },
                &base_url,
            ),
        );
        sitemap.push("/skills/".into());
        twins.push(("/skills/".into(), "/skills/index.md".into()));
    }

    // The front page.
    {
        // No package for them yet: the archive is the distribution, and it
        // holds one directory per skill, so it unpacks straight into a skills
        // directory with nothing to rename.
        let install = format!(
            "mkdir -p .claude/skills\ncurl -fsSL {base_url}/skills.tar.gz | tar xz -C .claude/skills"
        );
        let body = format!(
            r#"<h1>rk</h1>
<p class="lede"><strong>rk</strong> compiles IEC 61131-3 Structured Text to WebAssembly: check, test, compile, in one binary, from any editor.</p>
<h2>Install the skills</h2>
<pre><code class="language-sh">{install_hl}</code></pre>
<p>Unpack it wherever your agent keeps its skills; any agent that reads the Agent Skills format can use them. Every skill is also a plain file at <code>/skills/&lt;name&gt;/SKILL.md</code>, if you want one on its own. New here? <a href="/skills/getting-started/">getting-started</a> is the first one to read.</p>
{list}
<h2>Diagnostics</h2>
<p>{codes} codes, one page per <a href="/diagnostics/">category</a>, each entry with the example that produces it and the compiler's own output. <br>The same data as <a href="/diagnostics.json">JSON</a>, which <code>rk explain</code> embeds.</p>
<h2>For agents too</h2>
<p>Every page here is also Markdown, and this documentation is also a set of tools an agent can call.</p>
"#,
            list = skill_list_html(&skills),
            install_hl = crate::highlight::shell_html(&install),
            codes = entries.len()
        );
        // Flush-left: a continuation line that kept its indentation would make
        // the whole section an indented code block in Markdown.
        let mut md = format!(
            r#"# rk

rk compiles IEC 61131-3 Structured Text to WebAssembly: check, test, compile, in one binary, from any editor. The documentation is a set of Agent Skills whose examples the compiler verifies before publishing.

## Install the skills

```sh
{install}
```

Unpack it wherever your agent keeps its skills.
Every skill is also a plain file at `/skills/<name>/SKILL.md`, if you want one on its own.

## Skills

"#
        );
        for s in &skills {
            md.push_str(&format!(
                "- [{}]({}/skills/{}/SKILL.md): {}\n",
                s.name,
                base_url,
                s.name,
                one_line(&s.description)
            ));
        }
        md.push_str(&format!("\n## Diagnostics\n\n[{} codes]({base_url}/diagnostics/index.md), also as [JSON]({base_url}/diagnostics.json).\n\n## For agents\n\n- Every page answers `Accept: text/markdown`.\n- [/llms.txt]({base_url}/llms.txt), [/llms-full.txt]({base_url}/llms-full.txt)\n- [/mcp]({base_url}/mcp): read-only MCP server\n", entries.len()));
        write(&out_dir, "index.md", &md);
        write(
            &out_dir,
            "index.html",
            &site::shell(
                &Page {
                    title: "rk",
                    description: "A compiler and toolchain for IEC 61131-3 Structured Text, documented as Agent Skills the compiler verifies.",
                    path: "/",
                    md: Some("/index.md"),
                    eyebrow: "",
                    body: &body,
                    wide: false,
                },
                &base_url,
            ),
        );
        sitemap.insert(0, "/".into());
        twins.push(("/".into(), "/index.md".into()));
    }

    // Agent-facing files.
    write(
        &out_dir,
        "llms.txt",
        &site::llms_txt(&base_url, &skills, &categories),
    );
    write(&out_dir, "llms-full.txt", &site::llms_full_txt(&skills));
    write(&out_dir, "robots.txt", &site::robots_txt(&base_url));
    write(
        &out_dir,
        "sitemap.xml",
        &site::sitemap_xml(&base_url, &sitemap),
    );
    write(&out_dir, "_headers", &site::headers_file(&twins));
    write(
        &out_dir,
        ".well-known/agent-skills/index.json",
        &site::agent_skills_index(&skills),
    );
    let card = site::mcp_server_card(&base_url);
    write(&out_dir, ".well-known/mcp/server-card.json", &card);
    write(&out_dir, ".well-known/mcp.json", &card);
    write(
        &out_dir,
        ".well-known/api-catalog",
        &site::api_catalog(&base_url),
    );
    write(
        &out_dir,
        ".well-known/ai-catalog.json",
        &site::ard_manifest(&base_url),
    );

    // The archive the front page unpacks: one directory per skill, no wrapper.
    {
        let file = fs::File::create(out_dir.join("skills.tar.gz")).unwrap();
        let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);
        tar.follow_symlinks(false);
        for skill in &skills {
            tar.append_dir_all(&skill.name, &skill.dir).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap();
    }

    eprintln!(
        "\nDone: {} diagnostics, {} skills, {} pages → {}",
        entries.len(),
        skills.len(),
        sitemap.len(),
        out_dir.display()
    );
}

/// What the old reference showed on hover: the message, the code and its
/// category, then notes, related locations and fixes.
fn popup_html(d: &DiagSpan) -> String {
    let mut h = escape(&d.message);
    if let Some(code) = &d.code {
        h.push_str(&format!(
            " <span class=\"source\">rk(<span class=\"code\">{}</span>)",
            escape(code)
        ));
        if let Some(desc) = &d.code_desc {
            h.push_str(&format!(" - {}", escape(desc)));
        }
        h.push_str("</span>");
    }
    for n in &d.notes {
        h.push_str(&format!("<span class=\"note\">Note: {}</span>", escape(n)));
    }
    for (msg, idx, line, col) in &d.related {
        h.push_str(&format!("<span class=\"related\"><span class=\"loc\">example{idx}.st({line}, {col}):</span> {}</span>", escape(msg)));
    }
    for f in &d.fixes {
        h.push_str(&format!("<span class=\"fix\">Help: {}</span>", escape(f)));
    }
    h
}

/// A line mark for each group, in the same weight as the page's other
/// drawings: a flag to start, a prompt for the commands, `</>` for the
/// language, sliders for the tools.
fn group_icon(group: &str) -> String {
    let inner = match group {
        "getting" => r#"<path d="M6 20V4"/><path d="M6 5h11l-2.2 3.5L17 12H6z"/>"#,
        "cli" => {
            r#"<rect x="2.5" y="4.5" width="19" height="15" rx="2.5"/><path d="M6.5 9.5l3 2.5-3 2.5"/><path d="M12.5 15.5h5"/>"#
        }
        "programming" => {
            r#"<path d="M8.5 8L4.5 12l4 4"/><path d="M15.5 8l4 4-4 4"/><path d="M13.4 5.5l-2.8 13"/>"#
        }
        _ => {
            r#"<path d="M4 8h9.5M18.5 8H20M4 16h3.5M12.5 16H20"/><circle cx="16" cy="8" r="2.3"/><circle cx="10" cy="16" r="2.3"/>"#
        }
    };
    format!(
        r#"<svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">{inner}</svg>"#
    )
}

fn skill_list_html(skills: &[skills::Skill]) -> String {
    let mut body = String::new();
    for (group, heading, blurb) in [
        (
            "getting",
            "Start here",
            "Install <code>rk</code>, make a workspace, run the loop once.",
        ),
        ("cli", "Toolchain", "One skill per <code>rk</code> command."),
        (
            "programming",
            "Language",
            "Structured Text as <code>rk</code> compiles it, and the standard library.",
        ),
        ("tool", "Tools", "The linter and the language server."),
    ] {
        body.push_str(&format!(
            "<h2 class=\"group\">{icon}{heading}</h2>\n<p>{blurb}</p>\n<ul class=\"skills\">\n",
            icon = group_icon(group)
        ));
        for s in skills.iter().filter(|s| s.group() == group) {
            body.push_str(&format!(
                "<li><a href=\"/skills/{n}/\">{n}</a><span>{d}</span></li>\n",
                n = escape(&s.name),
                d = escape(
                    one_line(&s.description)
                        .split(". Use when")
                        .next()
                        .unwrap_or("")
                )
            ));
        }
        body.push_str("</ul>\n");
    }
    body
}
