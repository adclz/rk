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
  --hl-keyword: #0000ff; --hl-control: #af00db; --hl-type: #267f99; --hl-string: #a31515;
  --hl-number: #098658; --hl-comment: #008000; --hl-attr: #0451a5; --hl-fn: #795e26;
  --hl-var: #001080;
  --bracket-1: #0431fa; --bracket-2: #319331; --bracket-3: #7b3814;
  --hl-enum: #0070c1;
  --red: #e51400; --yellow: #bf8803; --blue: #1a85ff; --bright-blue: #0050bd; --green: #1c6b3a;
  --pass: #007100; --fail: #a1260d;
  --serif: 'IBM Plex Serif', Georgia, 'Times New Roman', serif;
  --mono: 'JetBrains Mono', 'IBM Plex Mono', 'SF Mono', Consolas, monospace;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --paper: #131312; --ink: #e8e6df; --muted: #979489; --rule: #2b2a27; --panel: #1c1b19;
    --link: #8ab4f8; --code: #e8e6df;
    --hl-keyword: #569cd6; --hl-control: #c586c0; --hl-type: #4ec9b0; --hl-string: #ce9178;
    --hl-number: #b5cea8; --hl-comment: #6a9955; --hl-attr: #9cdcfe; --hl-fn: #dcdcaa;
    --hl-var: #9cdcfe;
    --bracket-1: #ffd700; --bracket-2: #da70d6; --bracket-3: #179fff;
    --hl-enum: #4fc1ff;
    --red: #f14c4c; --yellow: #cca700; --blue: #3794ff; --bright-blue: #8ab4f8; --green: #a6d189;
    --pass: #73c991; --fail: #f14c4c;
  }
}
:root[data-theme="dark"] {
  --paper: #131312; --ink: #e8e6df; --muted: #979489; --rule: #2b2a27; --panel: #1c1b19;
  --link: #8ab4f8; --code: #e8e6df;
    --hl-keyword: #569cd6; --hl-control: #c586c0; --hl-type: #4ec9b0; --hl-string: #ce9178;
  --hl-number: #b5cea8; --hl-comment: #6a9955; --hl-attr: #9cdcfe; --hl-fn: #dcdcaa;
  --hl-var: #9cdcfe;
  --bracket-1: #ffd700; --bracket-2: #da70d6; --bracket-3: #179fff;
  --hl-enum: #4fc1ff;
  --red: #f14c4c; --yellow: #cca700; --blue: #3794ff; --bright-blue: #8ab4f8; --green: #a6d189;
  --pass: #73c991; --fail: #f14c4c;
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
blockquote { margin: 1.25rem 0; padding: 0.75rem 1rem; border-left: 3px solid var(--link); background: var(--panel); color: var(--ink); font-size: 0.95rem; }
blockquote p { margin: 0; }
blockquote p + p { margin-top: 0.5rem; }
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
h2.group { display: flex; align-items: center; gap: 0.55rem; }
h2.group svg { flex: none; width: 1.05em; height: 1.05em; color: var(--muted); }
.entry { margin: 2.5rem 0; }
.entry h2 { border: 0; padding: 0; margin: 0 0 0.25rem; font-family: var(--mono); font-size: 1rem; font-weight: 600; }
.entry h2 a { color: var(--ink); text-decoration: none; }
.entry h2 a:hover { text-decoration: underline; }
.entry .description { margin: 0.25rem 0 0.75rem; }
.output { font-family: var(--mono); font-size: 0.78rem; }
footer { margin-top: 4rem; padding-top: 1rem; border-top: 1px solid var(--rule); font-family: var(--mono); font-size: 0.75rem; color: var(--muted); display: flex; flex-wrap: wrap; gap: 0.5rem 1.5rem; }
footer a { color: var(--muted); }
/* What the VS Code extension shows for a .st file, scope for scope: an
   elementary type is `support.type.primitives` (teal), a storage word and a
   language constant are one blue, control flow is magenta, and operators and
   separators keep the plain foreground. */
.hl-keyword, .hl-keyword-storage, .hl-keyword-operator,
.hl-constant-builtin, .hl-variable-builtin { color: var(--hl-keyword); }
.hl-keyword-control { color: var(--hl-control); }
.hl-type, .hl-type-builtin, .hl-namespace { color: var(--hl-type); }
.hl-string { color: var(--hl-string); }
.hl-number { color: var(--hl-number); }
.hl-comment { color: var(--hl-comment); }
.hl-attribute { color: var(--hl-attr); }
.hl-function, .hl-function-method, .hl-function-call { color: var(--hl-fn); }
.hl-variable { color: var(--hl-var); }
.hl-bracket-1 { color: var(--bracket-1); }
.hl-bracket-2 { color: var(--bracket-2); }
.hl-bracket-3 { color: var(--bracket-3); }
.hl-constant { color: var(--hl-enum); }

.hl-pass { color: var(--pass); }
.hl-fail { color: var(--fail); }

/* What sets it apart: full-width rows, diagram beside the claim, alternating */
.why { margin: 2.5rem 0 3rem; }
.feat { display: grid; grid-template-columns: 1fr 1fr; gap: 3rem; align-items: center; margin: 0 0 5.5rem; }
.feat:last-of-type { margin-bottom: 3rem; }
.feat:nth-of-type(even) .feat-art { order: 2; }
@media (max-width: 720px) { .feat { grid-template-columns: 1fr; gap: 1.5rem; margin-bottom: 4rem; } .feat:nth-of-type(even) .feat-art { order: 0; } }
.feat svg { width: 100%; height: auto; display: block; color: var(--muted); }
.feat svg text { font-family: var(--mono); }
.feat h3 { font-family: var(--serif); font-weight: 500; font-size: 1.55rem; line-height: 1.2; color: var(--ink); margin: 0 0 0.7rem; letter-spacing: -0.01em; text-transform: none; text-wrap: balance; }
.feat p { margin: 0; font-size: 1.05rem; line-height: 1.6; color: var(--muted); }
.feat p .lead-in { color: var(--ink); font-weight: 500; }
.feat p .quote { font-size: 1.2em; color: var(--ink); }
.why-foot { font-size: 0.9rem; color: var(--muted); margin: 0 0 2.5rem; }

/* The reference: sidebar, search, inline markers */
main.wide { max-width: 1180px; }
.ref { display: grid; grid-template-columns: 240px minmax(0, 1fr); gap: 3rem; align-items: start; }
/* On a phone a 242-code tree is not navigation, the search is: the tree is
   dropped so the diagnostics start at the top, and the field stays in reach. */
@media (max-width: 860px) {
  .ref { grid-template-columns: 1fr; gap: 1rem; }
  .ref nav.side { position: sticky; top: 0; z-index: 6; max-height: none; overflow: visible; background: var(--paper); padding: 0.6rem 0 0.4rem; }
  .ref nav.side details { display: none; }
  .ref nav.side input { margin-bottom: 0; }
}
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
.diag-popup { display: none; position: fixed; z-index: 10; min-width: min(22rem, calc(100vw - 2rem)); max-width: min(40rem, calc(100vw - 2rem)); white-space: pre-wrap; font-family: var(--mono); font-size: 0.78rem; line-height: 1.45; color: var(--ink); background: var(--paper); border: 1px solid var(--rule); border-radius: 4px; padding: 0.6rem 0.75rem; box-shadow: 0 6px 24px rgba(0,0,0,0.18); text-decoration: none; }
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
    // Only when the sidebar is on screen and scrolls itself; where it is
    // dropped (a phone) this would drag the page back up to the index.
    if (a.offsetParent) a.scrollIntoView({ block: 'nearest' });
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
    // Below the mark when it fits, above it otherwise, and never off a short
    // screen: a tall popup on a phone used to land past the top edge.
    const top = r.bottom + h + 8 <= innerHeight ? r.bottom + 4 : r.top - h - 4;
    pop.style.top = Math.max(8, Math.min(top, innerHeight - h - 8)) + 'px';
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
  <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500;600&family=IBM+Plex+Serif:ital,wght@0,400;0,500;1,400&display=swap">
  <style>{css}</style>
</head>
<body>
<main{wide}>
<nav class="top">
  <a class="brand" href="/">{site}</a>
  <a href="/skills/">skills</a>
  <a href="/diagnostics/">diagnostics</a>
  <a href="/linter/">linter</a>
  <a href="/formatter/">formatter</a>
</nav>
{eyebrow}{body}
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
        eyebrow = if page.eyebrow.is_empty() {
            String::new()
        } else {
            format!("<p class=\"eyebrow\">{}</p>\n", escape(page.eyebrow))
        },
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

/// `/.well-known/ai-catalog.json` (Agentic Resource Discovery): the
/// capabilities an agent can reach here, each with the questions it answers.
pub fn ard_manifest(base: &str) -> String {
    let host = host_of(base);
    format!(
        r#"{{
  "specVersion": "1.0",
  "host": {{ "displayName": "rk", "identifier": "did:web:{host}" }},
  "entries": [
    {{
      "identifier": "urn:air:{host}:server:docs",
      "displayName": "rk documentation",
      "type": "application/mcp-server-card+json",
      "url": "{base}/.well-known/mcp/server-card.json",
      "representativeQueries": [
        "what does diagnostic E0301 mean",
        "which rk diagnostics are about duplicates",
        "show the skill for writing Structured Text tests",
        "how do I bind a PROGRAM to a task in rk"
      ]
    }}
  ]
}}
"#
    )
}

fn host_of(base: &str) -> &str {
    let host = base.split("://").nth(1).unwrap_or(base);
    host.split([':', '/']).next().unwrap_or(host)
}

/// `rk.example` becomes `example.rk`, the namespace an MCP card carries.
fn reverse_dns(base: &str) -> String {
    let host = host_of(base);
    let mut parts: Vec<&str> = host.split('.').collect();
    parts.reverse();
    format!("{}/docs", parts.join("."))
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// What sets the toolchain apart: four rows, each led by the capability as a
/// sentence, with a diagram of the mechanism beside it.
pub fn why_html() -> String {
    format!(
        r##"<div class="why">

<svg width="0" height="0" aria-hidden="true" focusable="false" style="position:absolute">
<symbol id="wasm-mark" viewBox="97.43 0 107.62 107.62">
  <path fill="#654ff0" transform="translate(-0.21)" d="M163.76,0c0,.19,0,.38,0,.58a12.34,12.34,0,0,1-24.68,0c0-.2,0-.39,0-.58H97.64V107.62H205.26V0ZM149,96.1l-5.24-25.93h-.09L138,96.1h-7.22L122.6,58h7.13l4.88,25.93h.09L140.58,58h6.67l5.28,26.25h.09L158.19,58h7L156.1,96.1Zm39.26,0-2.43-8.48H173l-1.87,8.48H164L173.22,58h11.25l11.21,38.1Z"/>
  <polygon fill="#654ff0" transform="translate(-0.21)" points="177.3 67.39 174.19 81.37 183.87 81.37 180.3 67.39 177.3 67.39"/>
</symbol>
</svg>

<div class="feat">
<div class="feat-art">
<svg viewBox="0 0 240 82" role="img" aria-label="Source files with lines added and removed, as in a review">
  <g fill="none" stroke="currentColor" stroke-width="1.5">
    <path d="M14 8h34l10 10v50H14z"/><path d="M48 8v10h10"/>
    <path d="M86 8h34l10 10v50H86z"/><path d="M120 8v10h10"/>
    <path d="M158 8h34l10 10v50h-44z"/><path d="M192 8v10h10"/>
  </g>
  <g stroke="currentColor" stroke-width="1.5" opacity="0.4">
    <path d="M22 32h28M22 40h20M22 56h16"/>
    <path d="M94 32h28M94 48h25M94 56h16"/>
    <path d="M166 32h28M166 40h20M166 48h25"/>
  </g>
  <path d="M22 48h25" stroke="var(--green)" stroke-width="1.5"/>
  <path d="M94 40h20" stroke="var(--red)" stroke-width="1.5"/>
  <g fill="currentColor" font-size="9" text-anchor="middle" opacity="0.8">
    <text x="41" y="76">.st</text><text x="113" y="76">.st</text><text x="185" y="76">.st</text>
  </g>
</svg>
</div>
<div>
<h3>Everything is code.</h3>
<p><span class="lead-in">From PLC logic to library configuration.</span>
<br>Namespaces organise your workspace, without restrictions.
<br><span class="quote">&#8220;If you need something, write the code for it!&#8221;</span></p>
</div>
</div>

<div class="feat">
<div class="feat-art">
<svg viewBox="0 0 240 102" role="img" aria-label="The compiler refusing an unknown struct field and listing the fields with similar names">
  <text x="6" y="14" font-size="7.6" fill="currentColor" xml:space="preserve" textLength="145.92" lengthAdjust="spacing">Base : Engine := (power := 100, </text>
  <text x="151.92" y="14" font-size="7.6" fill="currentColor" textLength="18.24" lengthAdjust="spacing">fuel</text>
  <text x="170.16" y="14" font-size="7.6" fill="currentColor" xml:space="preserve" textLength="45.6" lengthAdjust="spacing"> := 10.0);</text>
  <path d="M151.9 19q2.3-3.3 4.6 0t4.6 0t4.6 0t4.6 0" fill="none" stroke="var(--red)" stroke-width="1.5"/>
  <path d="M156 22v5h-8" fill="none" stroke="var(--red)" stroke-width="1.1" opacity="0.6"/>
  <text x="6" y="34" font-size="7.6" fill="var(--red)">'Engine' has no field named 'fuel'</text>
  <text x="6" y="50" font-size="7.6" fill="currentColor" opacity="0.75">Note: 'Engine' has fields with similar name:</text>
  <g font-size="7.6" fill="var(--green)">
    <text x="34" y="62">- fuel1</text>
    <text x="34" y="73">- fuel2</text>
    <text x="34" y="84">- fuel3</text>
    <text x="34" y="95">- fuel4</text>
  </g>
</svg>
</div>
<div>
<h3>Catch mistakes at your desk, not on site.</h3>
<p><span class="lead-in">A strict compiler, on purpose.</span>
<br>Hundreds of diagnostics, each explained, each with an example.
<br>Plus a linter you can tune.</p>
</div>
</div>

<div class="feat">
<div class="feat-art">
<svg viewBox="0 0 240 76" role="img" aria-label="A new WebAssembly module swapping into a controller that keeps scanning">
  <rect x="8" y="26" width="34" height="26" rx="3" fill="none" stroke="currentColor" stroke-width="1.5" opacity="0.35"/>
  <use href="#wasm-mark" x="16" y="30" width="18" height="18" opacity="0.3"/>
  <rect x="54" y="26" width="34" height="26" rx="3" fill="none" stroke="var(--green)" stroke-width="1.6"/>
  <use href="#wasm-mark" x="62" y="30" width="18" height="18"/>
  <path d="M94 39h24" fill="none" stroke="var(--green)" stroke-width="1.6" stroke-dasharray="4 3"/>
  <path d="M116 35l6 4-6 4z" fill="var(--green)"/>
  <rect x="128" y="14" width="104" height="50" rx="4" fill="none" stroke="currentColor" stroke-width="1.5"/>
  <path d="M136 40q7-13 14 0t14 0t14 0t14 0t14 0" fill="none" stroke="var(--green)" stroke-width="1.6"/>
  <text x="180" y="59" font-size="7.5" fill="currentColor" text-anchor="middle" opacity="0.75">still scanning</text>
  <text x="180" y="9" font-size="8" fill="currentColor" text-anchor="middle">controller</text>
</svg>
</div>
<div>
<h3>Deploy as fast as you edit.</h3>
<p><span class="lead-in">A workspace compiles in half a second.</span>
<br>WebAssembly instantiation does the swap.
<br>The machine never stops scanning.</p>
</div>
</div>

</div>
"##
    )
}

/// The formatter page. Everything here was checked by running `rk fmt`:
/// it indents with TABS, and it never reflows to a line width.
pub fn formatter_html(hl: &crate::highlight::StHighlighter) -> String {
    // Statements, declarations and whole var sections each need the POU they
    // were written for, or the snippet's first token is lost to error recovery.
    let (body_open, body_close) = crate::highlight::FRAGMENT_WRAP;
    let (pou_open, pou_close) = ("FUNCTION_BLOCK __Fmt\n", "\nEND_FUNCTION_BLOCK\n");
    let (decl_open, decl_close) = crate::highlight::DECL_WRAP;
    let indent = hl.html_in(
        body_open,
        "IF condition THEN\n\tx := 1;\n\tIF nested THEN\n\t\ty := 2;\n\tEND_IF;\nEND_IF;",
        body_close,
    );
    let decls = hl.html_in(
        pou_open,
        "// as written\nVAR a : INT; b : INT; c : INT; END_VAR\n\n// as formatted\nVAR\n\ta: INT;\n\tb: INT;\n\tc: INT;\nEND_VAR",
        pou_close,
    );
    let spacing = hl.html_in(
        body_open,
        "// as written\nm_iCurrentValue:= m_iCurrentValue+1;\nx :=a>b AND c<>d;\nok := Color # Red;\nv := arr [ 0 ];\n\n// as formatted\nm_iCurrentValue := m_iCurrentValue + 1;\nx := a > b AND c <> d;\nok := Color#Red;\nv := arr[0];",
        body_close,
    );
    let lists = hl.html_in(
        body_open,
        "// written on one line, so it stays on one line\nn := Sum(a := 1, b := 2, c := 3);\n\n// written with a break after the first argument…\nn := Sum(a := 1,\n\tb := 2, c := 3);\n\n// …so all of them get their own line\nn := Sum(\n\ta := 1,\n\tb := 2,\n\tc := 3\n);",
        body_close,
    );
    let inits = hl.html_in(
        decl_open,
        "p: Pt := (x := 1, y := 2);\nq: Pt := (\n\tx := 1,\n\ty := 2\n);",
        decl_close,
    );
    let cmds = crate::highlight::shell_html(
        "rk fmt              # rewrite every .st file in the workspace\naa fmt --check      # report what would change; exit 1 if anything would",
    );
    let comments = hl.html_in(
        pou_open,
        "VAR_INPUT\n\t(** this stays on top **)\n\tIN: BOOL; (* and this stays here *)\nEND_VAR\n\ts := '  spacing   inside   a   string  ';",
        pou_close,
    );
    format!(
        r##"<h1>Formatter</h1>
<p class="lede">One way to write it, so a diff shows what changed rather than who typed it.</p>

<pre><code class="language-sh">{cmds}</code></pre>

<p>In an editor it is the language server's <em>Format Document</em>, so the same rules apply whether a person or a script asks.</p>

<h2>What it guarantees</h2>
<p>The formatter works on the parsed syntax tree, not on the text, so it cannot produce a file that no longer parses. It needs the file to parse going in, and refuses the whole file if it does not; semantic errors like a type mismatch or an unresolved name do not stop it. Its own suite formats twice and requires the second pass to change nothing, and continuous integration reformats two corpora on every change, the standard library and the grammar's own test fixtures, checking that no program's meaning moved.</p>

<h2>What it does not do</h2>
<p>It has no line-width target and never reflows your expressions. A 300-character condition stays on one line if that is how you wrote it, and a call you split across lines stays split. What it normalises is spacing, indentation and the placement of declarations, which is the part people argue about in review.</p>

<h2>Indentation</h2>
<p>One tab per level, and every block indents: variable sections, bodies, methods, namespaces, classes and <code>TYPE</code> blocks.</p>
<pre><code class="language-iecst">{indent}</code></pre>

<h2>Declarations</h2>
<p>One declaration per line, so a name is never hidden behind a semicolon halfway across the line, and the type is separated by a single space after the colon.</p>
<pre><code class="language-iecst">{decls}</code></pre>

<h2>Spacing</h2>
<p>Binary operators, assignments and comparisons get one space either side; the accessors get none.</p>
<pre><code class="language-iecst">{spacing}</code></pre>

<h2>Lists: you choose the shape</h2>
<p>A parameter list or an initialiser is written on one line or spread over several, and <strong>a line break anywhere inside it is the instruction</strong>. Keep it on one line and the formatter leaves it there; put a newline in it and every element gets its own line, with the closing bracket back at the statement's indent. Nothing depends on how long the line is, so the shape is yours to decide and the formatter only makes it consistent.</p>
<pre><code class="language-iecst">{lists}</code></pre>
<p>Initialisers follow the same rule, so a small struct stays inline and a large one opens up.</p>
<pre><code class="language-iecst">{inits}</code></pre>

<h2>Semicolons</h2>
<p>The parser accepts a missing <code>;</code> at the end of a declaration or a statement, so a file that omits one still checks and still compiles. The formatter writes it in: every declaration, statement and directive comes back terminated, and one already there is left alone. A <code>USING</code> naming several namespaces takes one terminator at the end, not one per name.</p>

<h2>What it never touches</h2>
<p>Comments and string literals are left exactly as written. A comment keeps its place, above the thing it describes or at the end of its line, and the spacing inside a string is yours.</p>
<pre><code class="language-iecst">{comments}</code></pre>

<h2>Blank lines</h2>
<p>A blank line between declarations, POUs or variable sections is a paragraph break, so it is kept. Several in a row collapse to one, which is the only part of your vertical spacing the formatter has an opinion about.</p>
"##
    )
}
