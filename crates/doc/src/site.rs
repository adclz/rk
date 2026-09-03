//! The site's shell and its agent-facing surfaces: one stylesheet, the page
//! frame, `llms.txt`, `robots.txt`, the sitemap, `_headers`, and the
//! Markdown twin every HTML page has.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::highlight::escape;
use crate::markdown::estimate_tokens;
use crate::skills::Skill;

pub const SITE_NAME: &str = "rk";

pub struct Page<'a> {
    pub title: &'a str,
    pub description: &'a str,
    /// Site-relative path of the HTML page, e.g. `/skills/programming-oop/`.
    pub path: &'a str,
    /// Site-relative path of the Markdown twin, if the page has one.
    pub md: Option<&'a str>,
    pub eyebrow: &'a str,
    pub body: &'a str,
    /// The reference page: a sidebar beside the content, more room.
    pub wide: bool,
}

pub const CSS: &str = r#"
:root {
  --paper: #fbfbf9; --ink: #111; --muted: #6f6f6a; --rule: #e6e5df; --panel: #f3f2ed;
  --link: #0050bd; --code: #1a1a1a;
  --hl-keyword: #7a3e9d; --hl-type: #005f87; --hl-string: #8a4b08; --hl-number: #1c6b3a;
  --hl-comment: #7a7a72; --hl-attr: #5c5c8a; --hl-fn: #0050bd;
  --red: #b3261e; --yellow: #8a6d00; --blue: #0050bd; --bright-blue: #0050bd; --green: #1c6b3a;
  --serif: 'IBM Plex Serif', Georgia, 'Times New Roman', serif;
  --mono: 'IBM Plex Mono', 'SF Mono', Consolas, monospace;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --paper: #131312; --ink: #e8e6df; --muted: #979489; --rule: #2b2a27; --panel: #1c1b19;
    --link: #8ab4f8; --code: #e8e6df;
    --hl-keyword: #c792ea; --hl-type: #82aaff; --hl-string: #e2b070; --hl-number: #a6d189;
    --hl-comment: #7f7d75; --hl-attr: #b0aee0; --hl-fn: #8ab4f8;
    --red: #f28b82; --yellow: #e8c077; --blue: #8ab4f8; --bright-blue: #8ab4f8; --green: #a6d189;
  }
}
:root[data-theme="dark"] {
  --paper: #131312; --ink: #e8e6df; --muted: #979489; --rule: #2b2a27; --panel: #1c1b19;
  --link: #8ab4f8; --code: #e8e6df;
  --hl-keyword: #c792ea; --hl-type: #82aaff; --hl-string: #e2b070; --hl-number: #a6d189;
  --hl-comment: #7f7d75; --hl-attr: #b0aee0; --hl-fn: #8ab4f8;
  --red: #f28b82; --yellow: #e8c077; --blue: #8ab4f8; --bright-blue: #8ab4f8; --green: #a6d189;
}
* { box-sizing: border-box; }
html { -webkit-text-size-adjust: 100%; }
body { margin: 0; background: var(--paper); color: var(--ink); font-family: var(--serif); font-size: 17px; line-height: 1.6; }
main { max-width: 800px; margin: 0 auto; padding: 2.5rem 1.25rem 5rem; }
nav.top { display: flex; flex-wrap: wrap; gap: 0.5rem 1.5rem; align-items: baseline; font-family: var(--mono); font-size: 0.8rem; margin-bottom: 3.5rem; }
nav.top a { color: var(--ink); text-decoration: none; }
nav.top a:hover { text-decoration: underline; }
nav.top .brand { font-weight: 600; margin-right: auto; }
.eyebrow { font-family: var(--mono); font-size: 0.72rem; letter-spacing: 0.08em; text-transform: uppercase; color: var(--muted); margin: 0 0 0.75rem; }
h1 { font-family: var(--serif); font-weight: 400; font-size: 2.3rem; line-height: 1.15; margin: 0 0 1rem; text-wrap: balance; }
h1.small-caps { font-variant: small-caps; letter-spacing: 0.01em; }
h2 { font-family: var(--serif); font-weight: 500; font-size: 1.35rem; margin: 2.75rem 0 0.75rem; border-bottom: 1px solid var(--rule); padding-bottom: 0.35rem; text-wrap: balance; }
h3 { font-family: var(--mono); font-weight: 600; font-size: 0.9rem; margin: 1.75rem 0 0.5rem; }
p { margin: 0 0 1rem; }
p.lede { font-size: 1.15rem; color: var(--ink); max-width: 38em; }
a { color: var(--link); text-decoration-thickness: 1px; text-underline-offset: 2px; }
a:focus-visible { outline: 2px solid var(--link); outline-offset: 2px; }
ul, ol { padding-left: 1.3rem; margin: 0 0 1rem; }
li { margin: 0.2rem 0; }
li p { margin: 0; }
code { font-family: var(--mono); font-size: 0.85em; color: var(--code); background: var(--panel); padding: 0.05em 0.35em; border-radius: 3px; }
pre { font-family: var(--mono); font-size: 0.82rem; line-height: 1.55; background: var(--panel); border: 1px solid var(--rule); padding: 0.9rem 1rem; overflow-x: auto; margin: 0.75rem 0 1.25rem; tab-size: 4; }
pre code { background: none; padding: 0; color: inherit; font-size: inherit; }
blockquote { margin: 0 0 1rem; padding-left: 1rem; border-left: 2px solid var(--rule); color: var(--muted); }
table { border-collapse: collapse; width: 100%; font-size: 0.92rem; margin: 0.75rem 0 1.25rem; }
th { text-align: left; font-family: var(--mono); font-size: 0.72rem; letter-spacing: 0.06em; text-transform: uppercase; color: var(--muted); font-weight: 500; border-bottom: 1px solid var(--ink); padding: 0.4rem 0.75rem 0.4rem 0; }
td { border-bottom: 1px solid var(--rule); padding: 0.45rem 0.75rem 0.45rem 0; vertical-align: top; }
.tablewrap { overflow-x: auto; }
hr { border: 0; border-top: 1px solid var(--rule); margin: 2.5rem 0; }
.pill { display: inline-block; font-family: var(--mono); font-size: 0.75rem; background: var(--panel); border-radius: 9px; padding: 0.15em 0.6em; color: var(--muted); text-decoration: none; }
.pill:hover { color: var(--ink); }
.skills { list-style: none; padding: 0; margin: 0; }
.skills li { display: grid; grid-template-columns: 15rem 1fr; gap: 0 1.25rem; padding: 0.65rem 0; border-bottom: 1px solid var(--rule); }
.skills li a { font-family: var(--mono); font-size: 0.85rem; text-decoration: none; color: var(--ink); }
.skills li a:hover { text-decoration: underline; }
.skills li span { color: var(--muted); font-size: 0.95rem; }
@media (max-width: 600px) { .skills li { grid-template-columns: 1fr; } }
.entry { margin: 2.5rem 0; }
.entry h2 { border: 0; padding: 0; margin: 0 0 0.25rem; font-family: var(--mono); font-size: 1rem; font-weight: 600; }
.entry h2 a { color: var(--ink); text-decoration: none; }
.entry h2 a:hover { text-decoration: underline; }
.entry .description { margin: 0.25rem 0 0.75rem; }
.output { font-family: var(--mono); font-size: 0.78rem; }
footer { margin-top: 4rem; padding-top: 1rem; border-top: 1px solid var(--rule); font-family: var(--mono); font-size: 0.75rem; color: var(--muted); display: flex; flex-wrap: wrap; gap: 0.5rem 1.5rem; }
footer a { color: var(--muted); }
.hl-keyword, .hl-keyword-storage, .hl-keyword-control, .hl-keyword-operator { color: var(--hl-keyword); }
.hl-type, .hl-type-builtin, .hl-namespace { color: var(--hl-type); }
.hl-string { color: var(--hl-string); }
.hl-number, .hl-constant-builtin, .hl-variable-builtin { color: var(--hl-number); }
.hl-comment { color: var(--hl-comment); font-style: italic; }
.hl-attribute { color: var(--hl-attr); }
.hl-function, .hl-function-method, .hl-function-call { color: var(--hl-fn); }

/* The reference: sidebar, search, inline markers */
main.wide { max-width: 1180px; }
.ref { display: grid; grid-template-columns: 240px minmax(0, 1fr); gap: 3rem; align-items: start; }
@media (max-width: 860px) { .ref { grid-template-columns: 1fr; } .ref nav.side { position: static; max-height: none; } }
nav.side { position: sticky; top: 1rem; max-height: calc(100vh - 2rem); overflow-y: auto; font-family: var(--mono); font-size: 0.78rem; scrollbar-width: thin; }
nav.side input { width: 100%; font: inherit; padding: 0.4rem 0.55rem; border: 1px solid var(--rule); border-radius: 4px; background: var(--paper); color: var(--ink); margin-bottom: 0.9rem; }
nav.side input:focus-visible { outline: 2px solid var(--link); outline-offset: 1px; }
nav.side details { margin: 0 0 0.35rem; }
nav.side summary { cursor: pointer; text-transform: uppercase; letter-spacing: 0.06em; font-size: 0.7rem; color: var(--muted); padding: 0.25rem 0; list-style: none; }
nav.side summary::-webkit-details-marker { display: none; }
nav.side summary::before { content: "▸ "; display: inline-block; width: 1em; }
nav.side details[open] summary::before { content: "▾ "; }
nav.side a { display: block; color: var(--ink); text-decoration: none; padding: 0.12rem 0 0.12rem 1em; border-left: 2px solid transparent; }
nav.side a:hover { text-decoration: underline; }
nav.side a.active { border-left-color: var(--link); color: var(--link); }
nav.side .hidden, .ref .hidden { display: none; }
.category-heading { font-family: var(--serif); font-weight: 500; font-size: 1.5rem; margin: 3rem 0 0.5rem; padding-bottom: 0.35rem; border-bottom: 1px solid var(--rule); }
.category-heading:first-child { margin-top: 0; }
.ref .entry { margin: 2.25rem 0; }
mark.diag { background: none; color: inherit; text-decoration: underline wavy var(--red); text-decoration-skip-ink: none; text-underline-offset: 3px; }
mark.diag.warning { text-decoration-color: var(--yellow); }
mark.diag.info { text-decoration-color: var(--blue); }
/* Placed by the script from the mark's rectangle, fixed to the viewport, so
   the example's scroll box neither clips it nor scrolls to fit it. */
.diag-popup { display: none; position: fixed; z-index: 10; min-width: 22rem; max-width: min(40rem, calc(100vw - 2rem)); white-space: pre-wrap; font-family: var(--mono); font-size: 0.78rem; line-height: 1.45; color: var(--ink); background: var(--paper); border: 1px solid var(--rule); border-radius: 4px; padding: 0.6rem 0.75rem; box-shadow: 0 6px 24px rgba(0,0,0,0.18); text-decoration: none; }
.diag-popup.open { display: block; }
.diag-popup .source { color: var(--muted); }
.diag-popup .code { color: var(--link); }
.diag-popup .note, .diag-popup .related, .diag-popup .fix { display: block; margin-top: 0.4rem; }
.diag-popup .loc { color: var(--link); }
#top { position: fixed; right: 1.25rem; bottom: 1.25rem; font-family: var(--mono); font-size: 0.75rem; padding: 0.35rem 0.6rem; border: 1px solid var(--rule); border-radius: 4px; background: var(--paper); color: var(--muted); text-decoration: none; opacity: 0; transition: opacity 0.15s; }
#top.visible { opacity: 1; }
"#;

/// The reference page's behaviour: search that filters the sidebar and the
/// entries, `/` to focus it, the active entry tracked on scroll, back to top.
/// Without it the page is still complete: every entry visible, every
/// category open.
pub const REFERENCE_JS: &str = r#"
(() => {
const search = document.getElementById('search');
const links = [...document.querySelectorAll('nav.side a[data-code]')];
const entries = [...document.querySelectorAll('section.entry')];
const groups = [...document.querySelectorAll('nav.side details')];
const headings = [...document.querySelectorAll('.category-heading')];

search.addEventListener('input', () => {
  const q = search.value.trim().toLowerCase();
  const shown = new Set();
  for (const e of entries) {
    const hit = !q || e.dataset.text.includes(q);
    e.classList.toggle('hidden', !hit);
    if (hit) shown.add(e.dataset.cat);
  }
  for (const a of links) a.classList.toggle('hidden', q && !document.getElementById(a.dataset.code)?.matches(':not(.hidden)'));
  for (const g of groups) { const on = !q || shown.has(g.dataset.cat); g.classList.toggle('hidden', !on); if (q && on) g.open = true; }
  for (const h of headings) h.classList.toggle('hidden', q && !shown.has(h.dataset.cat));
});
document.addEventListener('keydown', e => {
  if (e.key === '/' && document.activeElement !== search) { e.preventDefault(); search.focus(); }
});

// Collapsed until the reader is on a section; the section they are in opens.
for (const g of groups) g.open = false;
let active = null;
const byCode = Object.fromEntries(links.map(a => [a.dataset.code, a]));
const seen = new IntersectionObserver(items => {
  for (const it of items) {
    if (!it.isIntersecting) continue;
    const a = byCode[it.target.id];
    if (!a) continue;
    active?.classList.remove('active');
    a.classList.add('active');
    active = a;
    if (!search.value) { for (const g of groups) g.open = g.contains(a); }
    a.scrollIntoView({ block: 'nearest' });
  }
}, { rootMargin: '-10% 0px -70% 0px' });
entries.forEach(e => seen.observe(e));

for (const m of document.querySelectorAll('mark.diag')) {
  const pop = m.querySelector('.diag-popup');
  if (!pop) continue;
  m.removeAttribute('title');
  m.addEventListener('mouseenter', () => {
    const r = m.getBoundingClientRect();
    pop.classList.add('open');
    const w = pop.offsetWidth, h = pop.offsetHeight;
    pop.style.left = Math.max(8, Math.min(r.left, innerWidth - w - 8)) + 'px';
    pop.style.top = (r.bottom + h + 8 <= innerHeight ? r.bottom + 4 : r.top - h - 4) + 'px';
  });
  m.addEventListener('mouseleave', () => pop.classList.remove('open'));
}
addEventListener('scroll', () => { for (const p of document.querySelectorAll('.diag-popup.open')) p.classList.remove('open'); }, { passive: true });

const topButton = document.getElementById('top');
addEventListener('scroll', () => topButton.classList.toggle('visible', scrollY > 400), { passive: true });
})();
"#;

pub fn shell(page: &Page, base_url: &str) -> String {
    shell_with(page, base_url, "")
}

/// [`shell`] with a script appended before `</main>`; the reference page's.
pub fn shell_with(page: &Page, base_url: &str, script: &str) -> String {
    let script = if script.is_empty() {
        String::new()
    } else {
        format!("<script>{script}</script>\n")
    };
    let wide = if page.wide { " class=\"wide\"" } else { "" };
    let alternate = page
        .md
        .map(|md| format!("\n  <link rel=\"alternate\" type=\"text/markdown\" href=\"{md}\">"))
        .unwrap_or_default();
    let md_link = page
        .md
        .map(|md| format!("<a href=\"{md}\">Markdown</a>"))
        .unwrap_or_default();
    let canonical = format!("{base_url}{}", page.path);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  <meta name="description" content="{description}">
  <link rel="canonical" href="{canonical}">{alternate}
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:wght@400;500;600&family=IBM+Plex+Serif:ital,wght@0,400;0,500;1,400&display=swap">
  <style>{css}</style>
</head>
<body>
<main{wide}>
<nav class="top">
  <a class="brand" href="/">{site}</a>
  <a href="/skills/">skills</a>
  <a href="/diagnostics/">diagnostics</a>
  <a href="/llms.txt">llms.txt</a>
  <a href="/mcp">mcp</a>
</nav>
<p class="eyebrow">{eyebrow}</p>
{body}
<footer>
  {md_link}
  <a href="/diagnostics.json">diagnostics.json</a>
  <a href="/skills.tar.gz">skills.tar.gz</a>
</footer>
{script}</main>
</body>
</html>
"#,
        title = escape(page.title),
        description = escape(page.description),
        css = CSS,
        site = SITE_NAME,
        eyebrow = escape(page.eyebrow),
        body = page.body,
    )
}

// ── Agent-facing files ───────────────────────────────────────────────────

pub struct DiagCategory {
    pub name: String,
    pub slug: String,
    pub count: usize,
}

pub fn llms_txt(base: &str, skills: &[Skill], categories: &[DiagCategory]) -> String {
    let mut s = String::new();
    s.push_str("# rk\n\n");
    s.push_str("> rk is an agent-first platform for industrial automation: a compiler from IEC 61131-3 Structured Text to WebAssembly, a runtime, deployment to controllers, a debugger, Modbus and MQTT, all driven from one CLI. The documentation is a set of Agent Skills whose examples the compiler verifies before publishing.\n\n");
    s.push_str("Every HTML page on this site has a Markdown twin at the URL linked here, and answers `Accept: text/markdown` with it.\n\n");
    for (group, heading) in [
        ("getting", "Start here"),
        ("cli", "Toolchain skills"),
        ("programming", "Language skills"),
        ("tool", "Tool skills"),
    ] {
        s.push_str(&format!("## {heading}\n\n"));
        for skill in skills.iter().filter(|k| k.group() == group) {
            s.push_str(&format!(
                "- [{}]({base}/skills/{}/SKILL.md): {}\n",
                skill.name,
                skill.name,
                one_line(&skill.description)
            ));
            for r in &skill.references {
                s.push_str(&format!(
                    "- [{} · {}]({base}/skills/{}/{}): reference loaded on demand by the skill\n",
                    skill.name, r.title, skill.name, r.rel
                ));
            }
        }
        s.push('\n');
    }
    s.push_str("## Diagnostics\n\n");
    for c in categories {
        s.push_str(&format!(
            "- [{}]({base}/diagnostics/{}.md): {} codes, each with a compiler-verified example\n",
            c.name, c.slug, c.count
        ));
    }
    s.push_str("\n## Optional\n\n");
    s.push_str(&format!(
        "- [llms-full.txt]({base}/llms-full.txt): every skill in one file\n"
    ));
    s.push_str(&format!("- [diagnostics.json]({base}/diagnostics.json): every code, title, description and example as JSON; the same file `rk explain` embeds\n"));
    s.push_str(&format!(
        "- [MCP]({base}/mcp): a read-only MCP server with explain, search and skill tools\n"
    ));
    s
}

pub fn llms_full_txt(skills: &[Skill]) -> String {
    let mut body = String::new();
    for skill in skills {
        body.push_str(&format!("\n\n<!-- skills/{}/SKILL.md -->\n\n", skill.name));
        body.push_str(&skill.text);
        for r in &skill.references {
            body.push_str(&format!("\n\n<!-- skills/{}/{} -->\n\n", skill.name, r.rel));
            body.push_str(&r.text);
        }
    }
    let tokens = estimate_tokens(&body);
    format!(
        "# rk: every skill\n\n> About {tokens} tokens. Prefer /llms.txt and load one skill at a time when context is tight.{body}\n"
    )
}

pub fn robots_txt(base: &str) -> String {
    format!(
        r#"# This site is documentation written for agents. Everything is open to read,
# to ground on, and to train on. Content Signals (https://contentsignals.org):
Content-Signal: search=yes, ai-input=yes, ai-train=yes

User-agent: *
Allow: /

# Named so a reader knows the policy was deliberate for each role.
User-agent: ClaudeBot
User-agent: Claude-SearchBot
User-agent: Claude-User
User-agent: GPTBot
User-agent: OAI-SearchBot
User-agent: ChatGPT-User
User-agent: Google-Extended
User-agent: PerplexityBot
User-agent: CCBot
Allow: /

Sitemap: {base}/sitemap.xml
"#
    )
}

pub fn sitemap_xml(base: &str, paths: &[String]) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    for p in paths {
        s.push_str(&format!("  <url><loc>{base}{}</loc></url>\n", escape(p)));
    }
    s.push_str("</urlset>\n");
    s
}

/// Headers for asset responses. The Worker sets the same ones on the
/// responses it generates, since `_headers` never applies to those.
pub fn headers_file(md_twins: &[(String, String)]) -> String {
    let mut s = String::from(
        "/*\n  Content-Signal: search=yes, ai-input=yes, ai-train=yes\n  X-Content-Type-Options: nosniff\n  Link: </.well-known/api-catalog>; rel=\"api-catalog\"\n\n/*.md\n  Content-Type: text/markdown; charset=utf-8\n\n/*.txt\n  Content-Type: text/plain; charset=utf-8\n\n/.well-known/api-catalog\n  Content-Type: application/linkset+json\n\n",
    );
    for (html_path, md_path) in md_twins {
        s.push_str(&format!(
            "{html_path}\n  Link: <{md_path}>; rel=\"alternate\"; type=\"text/markdown\"\n\n"
        ));
    }
    s
}

pub fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Strip ANSI escape sequences from a compiler report, for the Markdown twin.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

pub fn write(out: &Path, rel: &str, content: &str) {
    let path = out.join(rel.trim_start_matches('/'));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

// ── Well-known discovery files ───────────────────────────────────────────
//
// The paths Cloudflare's Agent Readiness scanner looks for, in the shapes
// its own site publishes. Each is a plain JSON file the generator derives
// from what the site already knows.

/// `/.well-known/agent-skills/index.json`: every skill with a digest of its
/// SKILL.md, so a client can verify what it fetched.
pub fn agent_skills_index(skills: &[Skill]) -> String {
    let items: Vec<String> = skills
        .iter()
        .map(|k| {
            let digest = Sha256::digest(k.text.as_bytes());
            format!(
                "    {{\"name\": \"{}\", \"type\": \"skill-md\", \"description\": \"{}\", \"url\": \"/skills/{}/SKILL.md\", \"digest\": \"sha256:{:x}\"}}",
                json_escape(&k.name),
                json_escape(&one_line(&k.description)),
                k.name,
                digest
            )
        })
        .collect();
    format!(
        "{{\n  \"$schema\": \"https://schemas.agentskills.io/discovery/0.2.0/schema.json\",\n  \"skills\": [\n{}\n  ]\n}}\n",
        items.join(",\n")
    )
}

/// `/.well-known/mcp/server-card.json`: where the MCP server is and what it
/// speaks. Carries both the fields the scanner reads (`serverInfo`, `url`,
/// `transport`) and the SEP-2127 ones (`remotes`, protocol versions).
pub fn mcp_server_card(base: &str) -> String {
    format!(
        r#"{{
  "name": "{name}",
  "serverInfo": {{ "name": "rk", "version": "1.0.0" }},
  "title": "rk",
  "description": "Read-only tools over the rk documentation: explain a diagnostic code, search the diagnostics, list and read the Agent Skills. No authentication.",
  "version": "1.0.0",
  "url": "{base}/mcp",
  "transport": {{ "type": "streamable-http" }},
  "capabilities": {{ "tools": true }},
  "supportedProtocolVersions": ["2026-07-28", "2025-11-25", "2025-06-18"],
  "remotes": [{{ "transport": "streamable-http", "url": "{base}/mcp" }}],
  "websiteUrl": "{base}/",
  "authentication": {{ "type": "none" }}
}}
"#,
        name = reverse_dns(base),
    )
}

/// `/.well-known/api-catalog` (RFC 9727): a linkset naming the machine
/// endpoints and the documents that describe them.
pub fn api_catalog(base: &str) -> String {
    format!(
        r#"{{
  "linkset": [
    {{
      "anchor": "{base}/mcp",
      "service-desc": [{{ "href": "{base}/.well-known/mcp/server-card.json", "type": "application/json" }}],
      "service-doc": [{{ "href": "{base}/llms.txt", "type": "text/markdown" }}],
      "status": [{{ "href": "{base}/mcp" }}]
    }},
    {{
      "anchor": "{base}/diagnostics.json",
      "service-desc": [{{ "href": "{base}/diagnostics.json", "type": "application/json" }}],
      "service-doc": [{{ "href": "{base}/diagnostics/index.md", "type": "text/markdown" }}]
    }},
    {{
      "anchor": "{base}/skills.tar.gz",
      "service-desc": [{{ "href": "{base}/.well-known/agent-skills/index.json", "type": "application/json" }}],
      "service-doc": [{{ "href": "{base}/skills/index.md", "type": "text/markdown" }}]
    }}
  ]
}}
"#
    )
}

/// `rk.example` becomes `example.rk`, the namespace an MCP card carries.
fn reverse_dns(base: &str) -> String {
    let host = base.split("://").nth(1).unwrap_or(base);
    let host = host.split([':', '/']).next().unwrap_or(host);
    let mut parts: Vec<&str> = host.split('.').collect();
    parts.reverse();
    format!("{}/docs", parts.join("."))
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
