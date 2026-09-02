// The site's one moving part. Everything readable is a static asset built by
// `cargo run --release -p doc`; this Worker adds what static hosting cannot:
//
// - `Accept: text/markdown` on any page answers with its Markdown twin,
// - the `Link: rel="alternate"` and `Content-Signal` headers on every
//   response (a `_headers` file never applies to Worker-served responses),
// - `/mcp`, a read-only MCP server over the same JSON the pages are built from.

import { McpServer } from "@modelcontextprotocol/server";
import { createMcpHandler } from "agents/mcp/server";
import { z } from "zod";

interface Env {
  ASSETS: Fetcher;
}

const CONTENT_SIGNAL = "search=yes, ai-input=yes, ai-train=yes";
// RFC 9727: every response points at the catalog of machine endpoints.
const API_CATALOG_LINK = '</.well-known/api-catalog>; rel="api-catalog"';

/** The Markdown twin of an HTML page, by the site's naming convention. */
function markdownTwin(pathname: string): string | null {
  const p = pathname.endsWith("/") || /\.[a-z]+$/.test(pathname) ? pathname : pathname + "/";
  if (p === "/") return "/index.md";
  if (p === "/skills/") return "/skills/index.md";
  if (p === "/diagnostics/") return "/diagnostics/index.md";
  let m = p.match(/^\/skills\/([^/]+)\/references\/([^/]+)\/$/);
  if (m) return `/skills/${m[1]}/references/${m[2]}.md`;
  m = p.match(/^\/skills\/([^/]+)\/$/);
  if (m) return `/skills/${m[1]}/SKILL.md`;
  m = p.match(/^\/diagnostics\/([^/]+)\/$/);
  if (m) return `/diagnostics/${m[1]}.md`;
  return null;
}

/** About four characters per token; a budget hint, never a promise. */
const tokens = (text: string) => Math.ceil(text.length / 4);

function wantsMarkdown(request: Request): boolean {
  const accept = request.headers.get("accept") ?? "";
  return accept.includes("text/markdown");
}

// ── Data the MCP tools answer from ──────────────────────────────────────

interface Diagnostic {
  code: string;
  category: string;
  title: string;
  description: string;
  sources: string[];
}
interface SkillEntry {
  name: string;
  group: string;
  description: string;
  files: string[];
}

const text = (t: string) => ({ content: [{ type: "text" as const, text: t }] });

const cache = new Map<string, Promise<unknown>>();
function asset<T>(env: Env, path: string, origin: string): Promise<T> {
  let hit = cache.get(path);
  if (!hit) {
    hit = env.ASSETS.fetch(new Request(origin + path)).then((r) => {
      if (!r.ok) throw new Error(`${path}: ${r.status}`);
      return r.json();
    });
    cache.set(path, hit);
  }
  return hit as Promise<T>;
}

async function assetText(env: Env, path: string, origin: string): Promise<string | null> {
  const r = await env.ASSETS.fetch(new Request(origin + path));
  return r.ok ? r.text() : null;
}

function createServer(env: Env, origin: string): McpServer {
  const server = new McpServer({ name: "rk", version: "1.0.0" });

  server.registerTool(
    "explain_diagnostic",
    {
      description:
        "What an rk diagnostic code means: its title, explanation, and an example that produces it. Codes look like E0301 or L0207.",
      inputSchema: { code: z.string().describe("A diagnostic code such as E0301") },
    },
    async ({ code }) => {
      const all = await asset<Diagnostic[]>(env, "/diagnostics.json", origin);
      const wanted = code.trim().toUpperCase();
      const d = all.find((x) => x.code === wanted);
      if (!d) {
        return text(`No diagnostic ${wanted}. Codes are E0001..E1xxx and L0xxx; use search_diagnostics to find one by words.`);
      }
      const example = d.sources.map((s) => "```iecst\n" + s + "\n```").join("\n\n");
      return text(`${d.code} ${d.title} (${d.category})\n\n${d.description}\n\nExample that produces it:\n\n${example}`);
    },
  );

  server.registerTool(
    "search_diagnostics",
    {
      description: "Find rk diagnostic codes whose code, title, or description contains the words given.",
      inputSchema: {
        query: z.string().describe("Words to look for, e.g. 'duplicate' or 'VAR_IN_OUT'"),
        limit: z.number().int().min(1).max(50).optional(),
      },
    },
    async ({ query, limit }) => {
      const all = await asset<Diagnostic[]>(env, "/diagnostics.json", origin);
      const words = query.toLowerCase().split(/\s+/).filter(Boolean);
      const hits = all
        .filter((d) => {
          const hay = `${d.code} ${d.title} ${d.description}`.toLowerCase();
          return words.every((w) => hay.includes(w));
        })
        .slice(0, limit ?? 10);
      return text(
        hits.length
          ? hits.map((d) => `${d.code}  ${d.title}  [${d.category}]`).join("\n")
          : "No diagnostic matches every word; try fewer.",
      );
    },
  );

  server.registerTool(
    "list_skills",
    {
      description: "The rk documentation as Agent Skills: each skill's name and when to use it. Read one with read_skill.",
    },
    async () => {
      const skills = await asset<SkillEntry[]>(env, "/skills.json", origin);
      return text(skills.map((s) => `${s.name}: ${s.description}`).join("\n"));
    },
  );

  server.registerTool(
    "read_skill",
    {
      description:
        "The full text of one skill (its SKILL.md), or one of its reference files. Skills are the language and toolchain documentation of rk.",
      inputSchema: {
        name: z.string().describe("A skill name from list_skills, e.g. programming-oop"),
        file: z.string().optional().describe("A file inside the skill, e.g. references/errors.md; default SKILL.md"),
      },
    },
    async ({ name, file }) => {
      const skills = await asset<SkillEntry[]>(env, "/skills.json", origin);
      const skill = skills.find((s) => s.name === name);
      if (!skill) {
        return text(`No skill named ${name}. Known: ${skills.map((s) => s.name).join(", ")}`);
      }
      const wanted = file ?? "SKILL.md";
      if (!skill.files.includes(wanted)) {
        return text(`${name} has no file ${wanted}. Files: ${skill.files.join(", ")}`);
      }
      const body = await assetText(env, `/skills/${name}/${wanted}`, origin);
      return text(body ?? `${wanted} could not be read`);
    },
  );

  return server;
}

// ── The request path ────────────────────────────────────────────────────

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    const url = new URL(request.url);

    if (url.pathname === "/mcp" || url.pathname.startsWith("/mcp/")) {
      const handler = createMcpHandler(() => createServer(env, url.origin));
      const response = await handler(request, env, ctx);
      const headers = new Headers(response.headers);
      headers.set("Content-Signal", CONTENT_SIGNAL);
      headers.append("Link", API_CATALOG_LINK);
      return new Response(response.body, { status: response.status, headers });
    }

    const twin = markdownTwin(url.pathname);

    if (twin && wantsMarkdown(request)) {
      const md = await env.ASSETS.fetch(new Request(url.origin + twin));
      if (md.ok) {
        const body = await md.text();
        const html = await env.ASSETS.fetch(request.clone());
        const headers = new Headers({
          "Content-Type": "text/markdown; charset=utf-8",
          Vary: "Accept",
          "Content-Signal": CONTENT_SIGNAL,
          "x-markdown-tokens": String(tokens(body)),
          "Cache-Control": md.headers.get("Cache-Control") ?? "public, max-age=300",
        });
        if (html.ok) {
          headers.set("x-original-tokens", String(tokens(await html.text())));
        }
        headers.append("Link", API_CATALOG_LINK);
        return new Response(body, { status: 200, headers });
      }
    }

    const response = await env.ASSETS.fetch(request);
    const headers = new Headers(response.headers);
    headers.set("Content-Signal", CONTENT_SIGNAL);
    if (url.pathname === "/.well-known/api-catalog") {
      headers.set("Content-Type", "application/linkset+json");
    }
    if (twin) {
      // `_headers` may already have set these on an asset response.
      headers.set("Link", `<${twin}>; rel="alternate"; type="text/markdown"`);
      const vary = headers.get("Vary");
      if (!vary) headers.set("Vary", "Accept");
      else if (!/\bAccept\b/.test(vary)) headers.set("Vary", `${vary}, Accept`);
    }
    if (url.pathname.endsWith(".md")) {
      headers.set("Content-Type", "text/markdown; charset=utf-8");
    } else if (url.pathname.endsWith(".txt")) {
      headers.set("Content-Type", "text/plain; charset=utf-8");
    }
    if (!(headers.get("Link") ?? "").includes('rel="api-catalog"')) {
      headers.append("Link", API_CATALOG_LINK);
    }
    return new Response(response.body, { status: response.status, headers });
  },
} satisfies ExportedHandler<Env>;
