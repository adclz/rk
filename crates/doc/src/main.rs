//! Verifies the site's sources against the compiler, then writes what Zola
//! renders: `site/content/` (every page, its fences and inline code
//! pre-rendered through the grammar), `site/data/` (what the templates and
//! shortcodes read) and `site/static/` (the Markdown twin of every page and
//! the files agents look for). Refuses to write anything when an example
//! disagrees with the compiler.
//!
//!     cargo run --release -p doc -- [--base-url https://…] [site-dir]
//!     cd site && zola build
//!
//! Also refreshes the committed `crates/doc/diagnostics.json`, which
//! `rk explain` embeds at build time.

mod casts;
mod docs;
mod examples;
mod highlight;
mod markdown;
mod pages;
mod render;
mod schema;
mod search;
mod site;
mod skills;
mod verify;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use db::RootDatabase;
use serde_json::json;

use crate::examples::{ErrorExample, SECTIONS};
use crate::highlight::{Mark, StHighlighter, escape, mark_html};
use crate::render::DiagSpan;
use crate::site::{DiagCategory, json_escape, one_line, strip_ansi, write};

struct DiagEntry<'a> {
    ex: &'a ErrorExample,
    report_html: String,
    report_text: String,
    spans: Vec<DiagSpan>,
}

/// What the hand-written pages substitute into their placeholders. A full
/// run derives these from the compiler; `--pages-only` reuses the last run's,
/// so editing prose does not mean recompiling 225 examples.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Substitutions {
    skills_html: String,
    skills_md: String,
    linter_html: String,
    linter_md: String,
    count: String,
}

/// Beside the site, gitignored: a dev-loop cache, never an input to a deploy.
fn cache_path(site_dir: &Path) -> PathBuf {
    site_dir.join(".substitutions.json")
}

fn slug(category: &str) -> String {
    category.to_lowercase().replace(' ', "-")
}

/// A TOML string, for the frontmatter the generator writes.
fn toml_str(s: &str) -> String {
    format!(
        "\"{}\"",
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    )
}

/// The pages the sidebar lists after the README's own, under Tools. Their
/// titles are the pages' own.
const TOOLS: [&str; 3] = ["linter.md", "formatter.md", "lsp.md"];

/// A diagnostic code a page names links to its entry, so it must have one.
/// The pages are rendered before anything is written; the front page is
/// rendered while writing, which `written` says.
fn refuse_unknown_codes(highlighter: &StHighlighter, written: &str) {
    let unknown = highlighter.unknown_codes();
    if unknown.is_empty() {
        return;
    }
    eprintln!(
        "\nThe pages name {} code(s) the diagnostics page has no entry for; {written}.\n",
        unknown.len()
    );
    for code in &unknown {
        eprintln!("  {code}");
    }
    eprintln!("\nA code's entry is its file in crates/doc/examples/.");
    std::process::exit(1);
}

/// The sidebar, or every reason it cannot be built, printed; nothing is
/// written then, as with any other disagreement.
fn sidebar_or_exit(
    repo: &Path,
    doc_pages: &[docs::DocPage],
    pages: &[pages::Page],
) -> docs::Sidebar {
    let tools = TOOLS
        .iter()
        .map(|rel| {
            let page = pages
                .iter()
                .find(|p| p.rel == Path::new(rel))
                .unwrap_or_else(|| panic!("site/pages/{rel} is gone; it is listed in TOOLS"));
            docs::Entry {
                title: page.title.clone(),
                url: format!("/{}/", rel.trim_end_matches(".md")),
            }
        })
        .collect();
    let index = fs::read_to_string(repo.join("docs").join(docs::INDEX)).unwrap();
    docs::Sidebar::from_index(&index, doc_pages, tools).unwrap_or_else(|problems| {
        eprintln!(
            "\nThe list in docs/{} and docs/ disagree in {} place(s); nothing was written.\n",
            docs::INDEX,
            problems.len()
        );
        for p in &problems {
            eprintln!("  {p}");
        }
        std::process::exit(1);
    })
}

/// The skill groups the pages list, in reading order.
const GROUPS: [(&str, &str, &str); 4] = [
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
];

fn main() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = crate_dir.join("../..").canonicalize().unwrap();

    let mut args = std::env::args().skip(1);
    let mut site_dir = repo.join("site");
    let mut base_url = std::env::var("SITE_URL").unwrap_or_else(|_| "http://localhost:8787".into());
    let mut pages_only = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--base-url" => base_url = args.next().expect("--base-url needs a value"),
            "--pages-only" => pages_only = true,
            other => site_dir = PathBuf::from(other),
        }
    }
    let base_url = base_url.trim_end_matches('/').to_string();

    let highlighter = StHighlighter::new().with_codes(examples::codes(&crate_dir.join("examples")));
    let content = site_dir.join("content");
    let statics = site_dir.join("static");

    // `--pages-only`: the authoring loop. Re-renders the hand-written pages
    // and the front page against the last full run's derived values, so
    // editing prose does not mean recompiling every example. Their fences are
    // still checked — that guarantee is the point of the whole generator —
    // but the 225 diagnostics and the skills are left alone.
    // The cast tables come from the compiler, not from anyone's memory of
    // the standard, and are refreshed before the page is built from them. CI
    // diffs the committed copies, as it does diagnostics.json.
    let convert = fs::read_to_string(repo.join("stdlib/Convert.st")).unwrap();
    for file in [
        "docs/strict-casts.md",
        "skills/programming-st/references/types.md",
    ] {
        if casts::refresh_readme(&repo.join(file), &convert) {
            eprintln!("{file}: cast tables refreshed");
        }
    }

    if pages_only {
        let subs: Substitutions = match fs::read_to_string(cache_path(&site_dir)) {
            Ok(text) => serde_json::from_str(&text).expect("the cache is this generator's own"),
            Err(_) => {
                eprintln!(
                    "--pages-only needs a full run first: `cargo run --release -p doc`.\nIt reuses that run's skills list and linter table."
                );
                std::process::exit(1);
            }
        };
        let pages = pages::discover(&site_dir.join("pages"), &highlighter);
        let doc_pages = docs::discover(&repo, &highlighter);
        let mut docs: Vec<skills::Doc> = pages
            .iter()
            .map(|p| skills::Doc {
                shown: format!("site/pages/{}", p.rel.display()),
                fences: &p.fences,
            })
            .collect();
        docs.extend(doc_pages.iter().map(|d| skills::Doc {
            shown: format!("docs/{}.md", d.stem),
            fences: &d.fences,
        }));
        let problems = skills::verify(&docs);
        if !problems.is_empty() {
            eprintln!(
                "\n{} example(s) in the pages do not hold; nothing was written.\n",
                problems.len()
            );
            for p in &problems {
                eprintln!("  {p}");
            }
            std::process::exit(1);
        }
        let sidebar = sidebar_or_exit(&repo, &doc_pages, &pages);
        refuse_unknown_codes(&highlighter, "nothing was written");
        let data = site_dir.join("data");
        let n = write_pages(
            &pages,
            &doc_pages,
            &sidebar,
            &subs,
            (&content, &data, &statics),
            &repo,
            &highlighter,
        )
        .len();
        refuse_unknown_codes(&highlighter, "the front page was written without its link");
        eprintln!(
            "\nDone: {n} page(s) re-rendered. The rest of the site is from the last full run."
        );
        return;
    }

    // ── Diagnostics: every example must produce the code it documents ──
    let examples = examples::load(&crate_dir.join("examples"));
    let mut entries: Vec<DiagEntry> = Vec::new();
    let mut produced: Vec<(&str, BTreeSet<String>)> = Vec::new();
    eprintln!("Diagnostics");
    for ex in &examples {
        eprint!("  {}...", ex.code);
        // Described without a runnable example: about the workspace, not a
        // file. Listed all the same, so `rk explain` has an answer.
        if ex.sources.is_empty() {
            let text = "No source example: this diagnostic is about the workspace, not a file.";
            entries.push(DiagEntry {
                ex,
                report_html: format!("<p>{text}</p>"),
                report_text: text.to_string(),
                spans: Vec::new(),
            });
            eprintln!(" described, no example");
            continue;
        }
        let mut db = RootDatabase::default();
        let sources: Vec<&str> = ex.sources.iter().map(String::as_str).collect();
        let (ansi, spans) = render::compile_and_render(&mut db, &sources, ex.lint_rule.as_deref());
        produced.push((&ex.code, verify::codes_in_output(&ansi)));
        entries.push(DiagEntry {
            ex,
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

    // ── Skills and pages: every fence must hold ────────────────────────
    let skills = skills::discover(&repo.join("skills"), &highlighter);
    let pages = pages::discover(&site_dir.join("pages"), &highlighter);
    let doc_pages = docs::discover(&repo, &highlighter);
    let mut docs = skills::skill_docs(&skills, &repo);
    for p in &pages {
        docs.push(skills::Doc {
            shown: format!("site/pages/{}", p.rel.display()),
            fences: &p.fences,
        });
    }
    for d in &doc_pages {
        docs.push(skills::Doc {
            shown: format!("docs/{}.md", d.stem),
            fences: &d.fences,
        });
    }
    eprintln!("Skills and pages");
    let problems = skills::verify(&docs);
    if !problems.is_empty() {
        eprintln!(
            "\n{} example(s) in the skills or pages do not hold; nothing was written.\n",
            problems.len()
        );
        for p in &problems {
            eprintln!("  {p}");
        }
        eprintln!(
            "\nSee crates/doc/src/skills.rs for the fence markers (fragment, decl, continues, syntax, sketch, expect=)."
        );
        std::process::exit(1);
    }
    let sidebar = sidebar_or_exit(&repo, &doc_pages, &pages);
    refuse_unknown_codes(&highlighter, "nothing was written");

    // ── Everything agreed: write ──────────────────────────────────────
    let data = site_dir.join("data");
    for dir in [&content, &data, &statics] {
        if dir.exists() {
            fs::remove_dir_all(dir).unwrap();
        }
        fs::create_dir_all(dir).unwrap();
    }
    // (HTML path, Markdown twin) of every page, for `_headers`.
    let mut twins: Vec<(String, String)> = Vec::new();

    // Sections in reading order (SECTIONS), entries sorted by code within
    // each: the order the pages use, and the order the committed JSON keeps.
    let mut by_category: BTreeMap<&str, Vec<&DiagEntry>> = BTreeMap::new();
    for e in &entries {
        by_category.entry(e.ex.category).or_default().push(e);
    }
    let order: Vec<&str> = SECTIONS
        .iter()
        .map(|(_, name)| *name)
        .filter(|name| by_category.contains_key(name))
        .collect();
    let categories: Vec<DiagCategory> = order
        .iter()
        .map(|c| DiagCategory {
            name: c.to_string(),
            slug: slug(c),
            count: by_category[c].len(),
        })
        .collect();

    // diagnostics.json: committed beside the generator and served by the
    // site. Hand-written JSON, in a fixed key order, so the committed file
    // only changes when the reference does.
    {
        let items: Vec<String> = order
            .iter()
            .flat_map(|c| by_category[c].iter().copied())
            .map(|e| {
                let sources: Vec<String> = e
                    .ex
                    .sources
                    .iter()
                    .map(|s| format!("\"{}\"", json_escape(s)))
                    .collect();
                format!(
                    r#"  {{"code":"{}","category":"{}","title":"{}","description":"{}","sources":[{}]}}"#,
                    json_escape(&e.ex.code),
                    json_escape(e.ex.category),
                    json_escape(&e.ex.title),
                    json_escape(&e.ex.description),
                    sources.join(",")
                )
            })
            .collect();
        let json = format!("[\n{}\n]\n", items.join(",\n"));
        fs::write(crate_dir.join("diagnostics.json"), &json).unwrap();
        write(&statics, "diagnostics.json", &json);
    }

    // The reference's data, one Markdown twin per category and per code,
    // and the index twin.
    {
        let mut cats = Vec::new();
        let mut index_md =
            String::from("# Diagnostics\n\nEvery code the compiler and the linter can report");
        for c in &categories {
            let mut md = format!("# Diagnostics: {}\n\n", c.name);
            let mut items = Vec::new();
            for e in &by_category[c.name.as_str()] {
                let text =
                    format!("{} {} {}", e.ex.code, e.ex.title, e.ex.description).to_lowercase();
                let mut sources_html = Vec::new();
                let mut entry_md = format!(
                    "## {} {}\n\n{}\n\n",
                    e.ex.code, e.ex.title, e.ex.description
                );
                for (i, s) in e.ex.sources.iter().enumerate() {
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
                    sources_html.push(mark_html(&highlighter.html(s), &marks));
                    entry_md.push_str(&format!("```iecst\n{s}\n```\n\n"));
                }
                entry_md.push_str(&format!(
                    "Compiler output:\n\n```\n{}```\n\n",
                    e.report_text
                ));
                let (_, rest) = entry_md.split_once("\n\n").unwrap();
                write(
                    &statics,
                    &format!("diagnostics/{}.md", e.ex.code),
                    &format!("# {} {}\n\n{}", e.ex.code, e.ex.title, rest),
                );
                md.push_str(&entry_md);
                items.push(json!({
                    "code": e.ex.code,
                    "title": e.ex.title,
                    "description": e.ex.description,
                    "description_html": e
                        .ex
                        .description
                        .split("\n\n")
                        .map(|para| format!("<p class=\"description\">{}</p>", escape(para)))
                        .collect::<String>(),
                    "text": text,
                    "sources_html": sources_html,
                    "report_html": e.report_html,
                }));
            }
            write(&statics, &format!("diagnostics/{}.md", c.slug), &md);
            index_md.push_str(&format!(
                "- [{}]({}/diagnostics/{}.md): {} codes\n",
                c.name, base_url, c.slug, c.count
            ));
            cats.push(
                json!({ "name": c.name, "slug": c.slug, "count": c.count, "entries": items }),
            );
        }
        write(&statics, "diagnostics/index.md", &index_md);
        write(
            &data,
            "diagnostics.json",
            &serde_json::to_string(&json!({ "count": entries.len(), "categories": cats })).unwrap(),
        );
        twins.push(("/diagnostics/".into(), "/diagnostics/index.md".into()));
    }

    // The linter's rule table. DERIVED: names come from each example's
    // `lint_rule`, severities from what the compiler actually printed, and
    // the default column from the linter's own recommended set, so it cannot
    // drift from the binary the way a hand-kept table does.
    let linter_md;
    let mut linter_html = String::new();
    {
        let severity_of = |e: &DiagEntry| -> &'static str {
            let marker = format!("[{}] ", e.ex.code);
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
        let mut md = String::new();
        for (prefix, title, blurb) in groups {
            let mut rows: Vec<&DiagEntry> = entries
                .iter()
                .filter(|e| e.ex.code.starts_with(prefix))
                .collect();
            rows.sort_by_key(|e| &e.ex.code);
            if rows.is_empty() {
                continue;
            }
            md.push_str(&format!(
                "## {title}\n\n| Code | Rule | Severity | Default | What it catches |\n| --- | --- | --- | --- | --- |\n"
            ));
            let mut html_rows: Vec<String> = Vec::new();
            for e in rows {
                let rule = e.ex.lint_rule.as_deref().unwrap_or("");
                let on = linter::RECOMMENDED_RULE_NAMES.contains(&rule);
                let sev = severity_of(e);
                md.push_str(&format!(
                    "| {} | `{}` | {} | {} | {} |\n",
                    e.ex.code,
                    rule,
                    sev,
                    if on { "on" } else { "—" },
                    one_line(&e.ex.description).replace('|', "\\|")
                ));
                html_rows.push(format!(
                    "<tr><td class=\"k\"><a href=\"/diagnostics/#{code}\">{code}</a></td><td class=\"k\">{rule}</td><td>{sev}</td><td>{on}</td><td>{what}</td></tr>\n",
                    code = e.ex.code,
                    rule = escape(rule),
                    on = if on { "on" } else { "—" },
                    what = escape(&one_line(&e.ex.description)),
                ));
            }
            md.push('\n');
            linter_html.push_str(&format!(
                "<h2>{title}</h2>\n<p>{blurb}</p>\n<div class=\"tablewrap\"><table><thead><tr><th>Code</th><th>Rule</th><th>Severity</th><th>Default</th><th>What it catches</th></tr></thead><tbody>\n{rows}</tbody></table></div>\n",
                rows = html_rows.concat()
            ));
        }
        linter_md = md;
    }

    // Skills: a section per skill with its references as pages, the raw
    // files as twins, the JSON the Worker's tools read, the list the pages
    // show, and the archive the front page unpacks.
    let mut skills_md = String::new();
    let mut skills_html = String::new();
    {
        let mut skills_json: Vec<String> = Vec::new();
        for skill in &skills {
            let dir = format!("skills/{}", skill.name);
            write(&statics, &format!("{dir}/SKILL.md"), &skill.text);
            let mut files = vec!["SKILL.md".to_string()];
            let mut refs = Vec::new();
            for r in &skill.references {
                write(&statics, &format!("{dir}/{}", r.rel), &r.text);
                files.push(r.rel.clone());
                let stem = r
                    .rel
                    .trim_start_matches("references/")
                    .trim_end_matches(".md");
                let html_path = format!("/skills/{}/references/{stem}/", skill.name);
                let md_path = format!("/skills/{}/{}", skill.name, r.rel);
                refs.push(format!(
                    "{{ title = {}, url = {} }}",
                    toml_str(&r.title),
                    toml_str(&html_path)
                ));
                write(
                    &content,
                    &format!("{dir}/references/{stem}.md"),
                    &format!(
                        "+++\ntitle = {title}\ndescription = {desc}\ntemplate = \"page.html\"\n\n[extra]\nhead_title = {head}\nno_h1 = {no_h1}\nmd = {md}\nbreadcrumb = {crumb}\n+++\n{body}",
                        title = toml_str(&r.title),
                        no_h1 = !r.has_h1,
                        head = toml_str(&format!("{} · {}", skill.name, r.title)),
                        desc =
                            toml_str(&format!("Reference material for the {} skill.", skill.name)),
                        md = toml_str(&md_path),
                        crumb = toml_str(&format!(
                            "<a href=\"/skills/{n}/\">{n}</a> / references",
                            n = escape(&skill.name)
                        )),
                        body = r.body,
                    ),
                );
                twins.push((html_path, md_path));
            }
            let html_path = format!("/skills/{}/", skill.name);
            let md_path = format!("/skills/{}/SKILL.md", skill.name);
            write(
                &content,
                &format!("{dir}/_index.md"),
                &format!(
                    "+++\ntitle = {title}\ndescription = {desc}\ntemplate = \"skill.html\"\nsort_by = \"none\"\n\n[extra]\nmd = {md}\nreferences = [{refs}]\n+++\n{body}",
                    title = toml_str(&skill.name),
                    desc = toml_str(&one_line(&skill.description)),
                    md = toml_str(&md_path),
                    refs = refs.join(", "),
                    body = skill.body,
                ),
            );
            twins.push((html_path, md_path));
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
            &statics,
            "skills.json",
            &format!("[\n{}\n]\n", skills_json.join(",\n")),
        );

        for (key, heading, blurb) in GROUPS {
            let members: Vec<&skills::Skill> = skills.iter().filter(|s| s.group() == key).collect();
            skills_md.push_str(&format!("## {heading}\n\n"));
            for s in &members {
                skills_md.push_str(&format!(
                    "- [{}]({}/skills/{}/SKILL.md): {}\n",
                    s.name,
                    base_url,
                    s.name,
                    one_line(&s.description)
                ));
            }
            skills_md.push('\n');
            skills_html.push_str(&format!(
                "<h2 class=\"group\">{icon}{heading}</h2>\n<p>{blurb}</p>\n<ul class=\"skills\">\n",
                icon = group_icon(key)
            ));
            for s in &members {
                skills_html.push_str(&format!(
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
            skills_html.push_str("</ul>\n");
        }

        // The archive: one directory per skill, no wrapper, so it unpacks
        // straight into a skills directory with nothing to rename.
        let file = fs::File::create(statics.join("skills.tar.gz")).unwrap();
        let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);
        tar.follow_symlinks(false);
        for skill in &skills {
            tar.append_dir_all(&skill.name, &skill.dir).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap();
    }

    let subs = Substitutions {
        skills_html: skills_html.clone(),
        skills_md: skills_md.clone(),
        linter_html: linter_html.clone(),
        linter_md: linter_md.clone(),
        count: entries.len().to_string(),
    };
    // So `--pages-only` can re-render prose without recompiling anything.
    fs::write(cache_path(&site_dir), serde_json::to_string(&subs).unwrap()).unwrap();

    twins.extend(write_pages(
        &pages,
        &doc_pages,
        &sidebar,
        &subs,
        (&content, &data, &statics),
        &repo,
        &highlighter,
    ));

    // Agent-facing files. The documentation is listed in the sidebar's
    // order, each page by its Markdown twin.
    let readme = fs::read_to_string(repo.join("README.md")).unwrap();
    let documentation: Vec<site::DocLink> = sidebar
        .pages()
        .into_iter()
        .map(|e| {
            let md = format!("{}index.md", e.url);
            let description = if e.url == "/" {
                one_line(&strip_tags(readme_tagline(&readme)))
            } else if let Some(d) = doc_pages.iter().find(|d| d.url() == e.url) {
                d.description.clone()
            } else {
                pages
                    .iter()
                    .find(|p| p.md.as_deref() == Some(md.as_str()))
                    .map(|p| p.description.clone())
                    .unwrap_or_default()
            };
            site::DocLink {
                title: e.title.clone(),
                md,
                description,
            }
        })
        .collect();
    write(
        &statics,
        "llms.txt",
        &site::llms_txt(&base_url, &skills, &categories, &documentation),
    );
    write(&statics, "llms-full.txt", &site::llms_full_txt(&skills));
    twins.sort();
    twins.dedup();
    write(&statics, "_headers", &site::headers_file(&twins));
    write(
        &statics,
        ".well-known/agent-skills/index.json",
        &site::agent_skills_index(&skills),
    );
    let card = site::mcp_server_card(&base_url);
    write(&statics, ".well-known/mcp/server-card.json", &card);
    write(&statics, ".well-known/mcp.json", &card);
    write(
        &statics,
        ".well-known/api-catalog",
        &site::api_catalog(&base_url),
    );
    write(
        &statics,
        ".well-known/ai-catalog.json",
        &site::ard_manifest(&base_url),
    );

    refuse_unknown_codes(&highlighter, "the front page was written without its link");
    eprintln!(
        "\nDone: {} diagnostics, {} skills, {} pages → {} (now `zola build` in site/)",
        entries.len(),
        skills.len(),
        twins.len(),
        site_dir.display()
    );
}

/// Write every hand-written page, the documentation, the README front page
/// and the sidebar that joins them, expanding the placeholders to HTML for
/// the page and to Markdown for its twin. `out` is the content, data and
/// static directories. Returns the (page, twin) pairs for `_headers`.
fn write_pages(
    pages: &[pages::Page],
    doc_pages: &[docs::DocPage],
    sidebar: &docs::Sidebar,
    subs: &Substitutions,
    (content, data, statics): (&Path, &Path, &Path),
    repo: &Path,
    highlighter: &StHighlighter,
) -> Vec<(String, String)> {
    let mut twins = Vec::new();
    // The search field's index: every documentation page, then the tools.
    let mut index = Vec::new();
    for d in doc_pages {
        index.extend(search::entries(&d.title, &d.url(), &d.source));
    }
    for rel in TOOLS {
        if let Some(p) = pages.iter().find(|p| p.rel == Path::new(rel)) {
            let url = format!("/{}/", rel.trim_end_matches(".md"));
            index.extend(search::entries(&p.title, &url, &p.source));
        }
    }
    write(statics, "search.json", &search::to_json(&index));
    let expand = |text: &str, skills: &str, linter: &str| -> String {
        text.replace("{{ skills() }}", skills)
            .replace("{{ diagnostics_count() }}", &subs.count)
            .replace("{{ linter_table() }}", linter)
    };
    for p in pages {
        let rel = p.rel.to_string_lossy().replace('\\', "/");
        write(
            content,
            &rel,
            &format!(
                "+++\n{}\n+++\n{}",
                p.frontmatter,
                expand(
                    &p.body,
                    subs.skills_html.trim_end(),
                    subs.linter_html.trim_end()
                )
            ),
        );
        if let Some(md) = &p.md {
            let twin = p.twin_rel();
            let body = expand(
                &p.source,
                subs.skills_md.trim_end(),
                subs.linter_md.trim_end(),
            );
            let lede = if p.lede.is_empty() {
                String::new()
            } else {
                format!("{}\n\n", p.lede)
            };
            write(
                statics,
                &twin.to_string_lossy(),
                &format!("# {}\n\n{lede}{}", p.title, body.trim_start()),
            );
            let html_path = {
                let t = twin.to_string_lossy().replace('\\', "/");
                format!("/{}", t.trim_end_matches("index.md"))
            };
            twins.push((html_path, md.clone()));
        }
    }

    // The documentation: each file in docs/ is a page, and its twin is the
    // file as written, links pointed at the site. Its list is the sidebar, so
    // /docs/ has no page of its own and goes to the first one.
    let first = sidebar.pages().first().map_or("/", |e| e.url.as_str());
    write(
        content,
        "docs/_index.md",
        &format!(
            "+++\ntitle = \"Documentation\"\nredirect_to = {}\n+++\n",
            toml_str(first)
        ),
    );
    for d in doc_pages {
        let md = format!("{}index.md", d.url());
        write(
            content,
            &format!("docs/{}.md", d.stem),
            &format!(
                "+++\ntitle = {title}\ndescription = {desc}\n\n[extra]\nmd = {md}\n+++\n{body}",
                title = toml_str(&d.title),
                desc = toml_str(&d.description),
                md = toml_str(&md),
                body = d.body,
            ),
        );
        write(statics, &format!("docs/{}/index.md", d.stem), &d.source);
        twins.push((d.url(), md));
    }
    write(
        data,
        "sidebar.json",
        &serde_json::to_string(&sidebar.json()).unwrap(),
    );

    // The front page IS the repository's README, so the project says one thing
    // in both places and neither can drift. Its fences are illustrative — a
    // few deliberately show code that does not compile — so they are
    // highlighted but never handed to the fence gate.
    let readme = docs::links_for_site(&fs::read_to_string(repo.join("README.md")).unwrap(), "");
    // The template prints the title and the lede above the body, so both come
    // out of it: `preprocess` lifts the H1, and the tagline is cut here.
    let tagline = readme_tagline(&readme);
    // The epigraph sits between the two in the README, so it is lifted as
    // well: left in the body it would land under the lede, out of order.
    let epigraph = readme_epigraph(&readme, tagline);
    let body_src = readme.replacen(tagline, "", 1).replacen(epigraph, "", 1);
    let body_src = landing(&body_src, &repo.join("site/assets")).unwrap_or_else(|problems| {
        eprintln!("\nThe front page cannot be laid out; nothing was written.\n");
        for p in &problems {
            eprintln!("  {p}");
        }
        std::process::exit(1);
    });
    let pre = markdown::preprocess(&body_src, highlighter);
    let title = pre.title.as_deref().unwrap_or("rk");
    let lede = one_line(&strip_tags(tagline));
    let epigraph = one_line(
        &epigraph
            .lines()
            .map(|l| l.trim_start().trim_start_matches('>').trim())
            .collect::<Vec<_>>()
            .join(" "),
    );
    write(
        content,
        "_index.md",
        &format!(
            "+++\ntitle = {title}\ndescription = {lede}\n\n[extra]\nepigraph = {epigraph}\nlede = {lede}\nmd = \"/index.md\"\nlanding = true\n+++\n{body}",
            title = toml_str(title),
            epigraph = toml_str(&epigraph),
            lede = toml_str(&lede),
            body = pre.body,
        ),
    );
    write(statics, "index.md", &readme);
    twins.push(("/".into(), "/index.md".into()));
    twins
}

/// What an item of the front page shows.
enum Picture {
    /// An icon beside its title, from `site/assets/icons/`: the item is a card.
    Icon(&'static str),
    /// A drawing above its title, from `site/assets/drawings/`: the item is a
    /// tile.
    Drawing(&'static str),
}

/// The picture of each item of the front page, by its title as the README
/// writes it.
const PICTURES: &[(&str, Picture)] = &[
    ("Portable", Picture::Icon("portable")),
    ("Sandboxed", Picture::Icon("sandboxed")),
    ("Agnostic host", Picture::Icon("host")),
    ("Expressive", Picture::Drawing("expressive")),
    ("Text only", Picture::Drawing("text")),
    ("Highly strict", Picture::Drawing("strict")),
    (
        "One core module, with memory dedicated once",
        Picture::Drawing("memory"),
    ),
    ("Bundled traps", Picture::Drawing("trap")),
    ("Lightweight", Picture::Drawing("pipeline")),
];

/// The README laid out as the front page: its links for GitHub readers left
/// out, since the page's buttons stand for them, and each `##` section with
/// `###` items a grid, one item each: cards when the items have icons,
/// larger tiles when they have drawings.
///
/// Refuses an item with no picture in [`PICTURES`], and a section that mixes
/// the two, so a new item of the README cannot reach the site without one.
/// The README shows GitHub the site's own pictures, as `<img>` tags pointing
/// into `site/assets/`. The front page inlines each item's picture itself,
/// so the tags go, with the line they leave empty.
fn without_readme_pictures(item: &str) -> String {
    let mut out = String::with_capacity(item.len());
    for line in item.split_inclusive('\n') {
        let mut kept = String::new();
        let mut rest = line;
        while let Some(i) = rest.find("<img ") {
            let Some(end) = rest[i..].find('>') else {
                break;
            };
            let tag = &rest[i..i + end + 1];
            kept.push_str(&rest[..i]);
            rest = &rest[i + end + 1..];
            if tag.contains("src=\"site/assets/") {
                rest = rest.trim_start_matches(' ');
            } else {
                kept.push_str(tag);
            }
        }
        kept.push_str(rest);
        if kept.trim().is_empty() && !line.trim().is_empty() {
            continue;
        }
        out.push_str(&kept);
    }
    out
}

fn landing(readme: &str, assets: &Path) -> Result<String, Vec<String>> {
    let mut out = String::new();
    let mut problems = Vec::new();
    let (intro, sections) = readme.split_at(readme.find("\n## ").map_or(readme.len(), |i| i + 1));
    let intro: Vec<&str> = intro
        .lines()
        .filter(|l| !l.trim_start().starts_with("- "))
        .collect();
    let intro = intro.join("\n");
    if !intro.trim().is_empty() {
        out.push_str(&format!(
            "<div class=\"intro\">\n\n{}\n\n</div>\n\n",
            intro.trim()
        ));
    }
    for section in sections.split("\n## ").map(|s| s.trim_start_matches("## ")) {
        let (head, items) =
            section.split_at(section.find("\n### ").map_or(section.len(), |i| i + 1));
        let head = head.trim();
        if items.is_empty() {
            out.push_str(&format!(
                "<section class=\"closing\">\n\n## {head}\n\n</section>\n\n"
            ));
            continue;
        }
        let mut cards = Vec::new();
        let mut tiles = Vec::new();
        for item in items.split("\n### ").map(|s| s.trim_start_matches("### ")) {
            let item = without_readme_pictures(item);
            let item = item.as_str();
            let title = item.lines().next().unwrap_or_default().replace("**", "");
            let title = title.trim();
            let (dir, name, tile) = match PICTURES.iter().find(|(t, _)| *t == title) {
                Some((_, Picture::Icon(name))) => ("icons", name, false),
                Some((_, Picture::Drawing(name))) => ("drawings", name, true),
                None => {
                    problems.push(format!(
                        "README.md: `{title}` has no picture; add one to PICTURES in crates/doc/src/main.rs"
                    ));
                    continue;
                }
            };
            let path = assets.join(dir).join(format!("{name}.svg"));
            let Ok(svg) = fs::read_to_string(&path) else {
                problems.push(format!("{} is missing", path.display()));
                continue;
            };
            // A blank line would end the HTML block the picture sits in.
            let svg: String = svg.lines().filter(|l| !l.trim().is_empty()).collect();
            if tile {
                tiles.push(format!(
                    "<div class=\"tile\">\n<div class=\"tile-figure\">{svg}</div>\n\n### {}\n\n</div>\n\n",
                    item.trim()
                ));
            } else {
                cards.push(format!(
                    "<div class=\"card\">\n\n### {svg} {}\n\n</div>\n\n",
                    item.trim()
                ));
            }
        }
        let (class, grid, items) = match (cards.is_empty(), tiles.is_empty()) {
            (false, true) => ("features", "cards", cards),
            (true, false) => ("showcase", "tiles", tiles),
            _ => {
                problems.push(format!(
                    "README.md: `{head}` mixes items with icons and items with drawings"
                ));
                continue;
            }
        };
        out.push_str(&format!(
            "<section class=\"{class}\">\n\n## {head}\n\n<div class=\"{grid}\">\n\n{}</div>\n\n</section>\n\n",
            items.concat()
        ));
    }
    if problems.is_empty() {
        Ok(out)
    } else {
        Err(problems)
    }
}

/// The README's tagline, which becomes the page's lede: the first block that
/// is neither the H1, the epigraph under it nor a list.
fn readme_tagline(readme: &str) -> &str {
    readme
        .split("\n\n")
        .map(str::trim)
        .find(|b| !(b.is_empty() || b.starts_with(['#', '>', '-', '*'])))
        .unwrap_or_default()
}

/// The quote between the H1 and the tagline, if the README opens with one.
fn readme_epigraph<'a>(readme: &'a str, tagline: &str) -> &'a str {
    let head = &readme[..readme.find(tagline).unwrap_or(0)];
    head.split("\n\n")
        .map(str::trim)
        .find(|b| b.starts_with('>'))
        .unwrap_or_default()
}

/// The text of a block, without its tags: the README centers the tagline in a
/// `<div>` for GitHub, and none of that belongs in a `description`.
fn strip_tags(block: &str) -> String {
    let mut out = String::with_capacity(block.len());
    let mut depth = 0usize;
    for c in block.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// What the reference shows on hover: the message, the code and its
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
    site::icon(inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_readme_pictures_are_left_to_the_front_page() {
        let item = "<img src=\"site/assets/icons/portable.svg\" width=\"20\" alt=\"\"> **Portable**\nOne binary.\n<img src=\"site/assets/drawings/strict.svg\" width=\"380\" alt=\"x\">\n\nText <img src=\"https://example.com/a.png\"> stays.\n";
        assert_eq!(
            without_readme_pictures(item),
            "**Portable**\nOne binary.\n\nText <img src=\"https://example.com/a.png\"> stays.\n"
        );
    }
}
