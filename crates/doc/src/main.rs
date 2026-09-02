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

use crate::highlight::{StHighlighter, escape};
use crate::site::{DiagCategory, Page, one_line, strip_ansi, write};

struct DiagEntry {
    code: &'static str,
    category: &'static str,
    title: &'static str,
    description: &'static str,
    sources: &'static [&'static str],
    report_html: String,
    report_text: String,
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
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
    let mut entries: Vec<DiagEntry> = Vec::new();
    let mut produced: Vec<(&str, std::collections::BTreeSet<String>)> = Vec::new();
    eprintln!("Diagnostics");
    for ex in &examples {
        eprint!("  {}...", ex.code);
        let mut db = RootDatabase::default();
        let ansi = render::compile_and_render(&mut db, ex.sources, ex.lint_rule);
        produced.push((ex.code, verify::codes_in_output(&ansi)));
        entries.push(DiagEntry {
            code: ex.code,
            category: ex.category,
            title: ex.title,
            description: ex.description,
            sources: ex.sources,
            report_html: render::ansi_to_html_fragment(&ansi),
            report_text: strip_ansi(&ansi),
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

    // Categories in first-seen order, entries sorted by code within each:
    // the order the pages use, and the order the committed JSON keeps.
    let mut by_category: BTreeMap<&str, Vec<&DiagEntry>> = BTreeMap::new();
    let mut order: Vec<&str> = Vec::new();
    for e in &entries {
        if !by_category.contains_key(e.category) {
            order.push(e.category);
        }
        by_category.entry(e.category).or_default().push(e);
    }
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

    // Diagnostics index.
    {
        let mut body = String::new();
        body.push_str("<h1>Diagnostics</h1>\n<p class=\"lede\">Every code the compiler and the linter can report, each with an example that produces it. The generator runs every example before publishing, so a page here never disagrees with the binary.</p>\n");
        body.push_str("<p>The same data as <a href=\"/diagnostics.json\">JSON</a>, which <code>rk explain &lt;code&gt;</code> embeds.</p>\n");
        body.push_str("<div class=\"tablewrap\"><table><thead><tr><th>Category</th><th>Codes</th><th>Markdown</th></tr></thead><tbody>\n");
        for c in &categories {
            body.push_str(&format!(
                "<tr><td><a href=\"/diagnostics/{s}/\">{n}</a></td><td>{k}</td><td><a href=\"/diagnostics/{s}.md\">{s}.md</a></td></tr>\n",
                s = c.slug, n = escape(&c.name), k = c.count
            ));
        }
        body.push_str("</tbody></table></div>\n");
        let mut md = String::from(
            "# Diagnostics\n\nEvery code the compiler and the linter can report, each with a compiler-verified example.\n\n",
        );
        for c in &categories {
            md.push_str(&format!(
                "- [{}]({}/diagnostics/{}.md): {} codes\n",
                c.name, base_url, c.slug, c.count
            ));
        }
        write(&out_dir, "diagnostics/index.md", &md);
        write(
            &out_dir,
            "diagnostics/index.html",
            &site::shell(
                &Page {
                    title: "Diagnostics",
                    description: "Every diagnostic code rk reports, with compiler-verified examples.",
                    path: "/diagnostics/",
                    md: Some("/diagnostics/index.md"),
                    eyebrow: "reference",
                    body: &body,
                },
                &base_url,
            ),
        );
        sitemap.push("/diagnostics/".into());
        twins.push(("/diagnostics/".into(), "/diagnostics/index.md".into()));
    }

    // One page per category, anchors per code, a Markdown twin per category
    // and per code.
    for c in &categories {
        let mut body = format!("<h1>{}</h1>\n", escape(&c.name));
        let mut md = format!("# Diagnostics: {}\n\n", c.name);
        for e in &by_category[c.name.as_str()] {
            body.push_str(&format!(
                "<section class=\"entry\" id=\"{code}\">\n<h2><a href=\"#{code}\">{code}</a> {title}</h2>\n<p class=\"description\">{desc}</p>\n",
                code = e.code, title = escape(e.title), desc = escape(e.description)
            ));
            let mut entry_md = format!("## {} {}\n\n{}\n\n", e.code, e.title, e.description);
            for s in e.sources {
                let s = s.trim_matches('\n');
                body.push_str(&format!(
                    "<pre><code class=\"language-iecst\">{}\n</code></pre>\n",
                    highlighter.html(s)
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
            write(
                &out_dir,
                &format!("diagnostics/{}.md", e.code),
                &format!(
                    "# {} {}\n\n{}",
                    e.code,
                    e.title,
                    &entry_md[entry_md.find("\n\n").unwrap() + 2..]
                ),
            );
            md.push_str(&entry_md);
        }
        let html_path = format!("/diagnostics/{}/", c.slug);
        let md_path = format!("/diagnostics/{}.md", c.slug);
        write(&out_dir, &md_path, &md);
        write(
            &out_dir,
            &format!("diagnostics/{}/index.html", c.slug),
            &site::shell(
                &Page {
                    title: &format!("{} diagnostics", c.name),
                    description: &format!(
                        "The {} diagnostics rk reports, with compiler-verified examples.",
                        c.name.to_lowercase()
                    ),
                    path: &html_path,
                    md: Some(&md_path),
                    eyebrow: "diagnostics",
                    body: &body,
                },
                &base_url,
            ),
        );
        sitemap.push(html_path.clone());
        twins.push((html_path, md_path));
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
                },
                &base_url,
            ),
        );
        sitemap.push("/skills/".into());
        twins.push(("/skills/".into(), "/skills/index.md".into()));
    }

    // The front page.
    {
        let install = format!("npx skills add {base_url}/skills.tar.gz --all");
        let body = format!(
            r#"<h1 class="small-caps">Structured Text, checked by a compiler, written for agents.</h1>
<p class="lede"><strong>rk</strong> compiles IEC 61131-3 Structured Text to WebAssembly. One binary checks, tests, formats and compiles a workspace, and deploys the result to a controller. The documentation is a set of <a href="https://agentskills.io">Agent Skills</a>: an agent loads the one its task needs, and the compiler has run every example on this site before it was published.</p>
<h2>Install the skills</h2>
<pre><code>{install}</code></pre>
<p>Works with any agent that reads the Agent Skills format. One skill: <code>npx skills add {base_url}/skills/programming-st/SKILL.md</code>. Every skill is also a plain file at <code>/skills/&lt;name&gt;/SKILL.md</code>.</p>
{list}
<h2>Diagnostics</h2>
<p>{codes} codes, one page per <a href="/diagnostics/">category</a>, each entry with the example that produces it and the compiler's own output. The same data as <a href="/diagnostics.json">JSON</a>, which <code>rk explain</code> embeds.</p>
<h2>For agents</h2>
<ul>
<li>Every page has a Markdown twin. Send <code>Accept: text/markdown</code> to any URL, follow the <code>rel="alternate"</code> link, or append <code>.md</code>. The response carries <code>x-markdown-tokens</code>.</li>
<li><a href="/llms.txt">/llms.txt</a> indexes everything; <a href="/llms-full.txt">/llms-full.txt</a> is every skill in one file, with its token count in the header.</li>
<li><a href="/mcp">/mcp</a> is a read-only MCP server: <code>explain_diagnostic</code>, <code>search_diagnostics</code>, <code>list_skills</code>, <code>read_skill</code>. No authentication.</li>
<li><a href="/robots.txt">robots.txt</a> allows every agent and states <code>Content-Signal: search=yes, ai-input=yes, ai-train=yes</code>.</li>
<li>Nothing here needs JavaScript to read.</li>
</ul>
"#,
            list = skill_list_html(&skills),
            codes = entries.len()
        );
        let mut md = format!(
            "# rk\n\naa compiles IEC 61131-3 Structured Text to WebAssembly. The documentation is a set of Agent Skills whose examples the compiler verifies before publishing.\n\nInstall: `{install}`\n\n## Skills\n\n"
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
                    eyebrow: "structured text · webassembly",
                    body: &body,
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

    // The archive `npx skills add <url>` installs from.
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

fn skill_list_html(skills: &[skills::Skill]) -> String {
    let mut body = String::new();
    for (group, heading, blurb) in [
        ("cli", "Toolchain", "One skill per <code>rk</code> command."),
        (
            "programming",
            "Language",
            "Structured Text as <code>rk</code> compiles it, and the standard library.",
        ),
        ("tool", "Tools", "The linter and the language server."),
    ] {
        body.push_str(&format!(
            "<h2>{heading}</h2>\n<p>{blurb}</p>\n<ul class=\"skills\">\n"
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
