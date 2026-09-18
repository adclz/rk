//! The agent-facing surfaces Zola does not produce: `llms.txt`, `_headers`,
//! the well-known discovery files, and the small helpers the generator
//! shares. The page frame, stylesheet and sitemap are Zola's, in site/.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::markdown::estimate_tokens;
use crate::skills::Skill;

// ── Agent-facing files ───────────────────────────────────────────────────

pub struct DiagCategory {
    pub name: String,
    pub slug: String,
    pub count: usize,
}

pub fn llms_txt(base: &str, skills: &[Skill], categories: &[DiagCategory]) -> String {
    let mut s = String::new();
    s.push_str("# rk\n\n");
    s.push_str("> rk is an agent-first compiler from IEC 61131-3 Structured Text to WebAssembly: check, test, compile, in one CLI. The documentation is a set of Agent Skills whose examples the compiler verifies before publishing.\n\n");
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

/// A line drawing in the one weight every icon on the site uses: a 24-unit
/// box, stroked in the text's colour, round caps.
pub fn icon(inner: &str) -> String {
    format!(
        r#"<svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">{inner}</svg>"#
    )
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

pub fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
