//! The skills are the documentation. This module reads `skills/`, renders
//! each `SKILL.md` and its references, and runs every `iecst` fence through
//! the compiler so the site cannot publish an example that no longer holds.
//!
//! The fence info string says how a block is checked:
//!
//! - `iecst`: a whole file; must check with no error.
//! - `iecst expect=E0101,E0402`: a whole file that must report exactly those
//!   errors and no other.
//! - `iecst continues`: appended to the previous whole-file fence of the same
//!   document. The chain is checked as one file where it ends, so a fence may
//!   use a POU a later fence of the chain declares.
//! - `iecst fragment`: statements, wrapped in a function body and parsed
//!   only, since a fragment names things the surrounding text declared.
//! - `iecst decl`: declarations, wrapped in a VAR block and parsed only.
//! - `iecst syntax`: a whole file, parsed only.
//! - `iecst sketch`: a shape with elisions (`…`), shown but not checked. The
//!   build reports how many there are; keep that number small.
//!
//! Lints never block: a skill may show a lint firing on purpose.

use std::path::{Path, PathBuf};

use auto_lsp::{
    default::db::{FileManager, file::File},
    lsp_types::{DiagnosticSeverity, NumberOrString, Url},
};
use db::RootDatabase;
use hir::check::diagnostics_for_file;

use crate::highlight::StHighlighter;
use crate::markdown::{self, Fence};

pub struct RefFile {
    /// Path relative to the skill directory, e.g. `references/errors.md`.
    pub rel: String,
    pub text: String,
    pub html: String,
    pub title: String,
    pub fences: Vec<Fence>,
}

pub struct Skill {
    pub name: String,
    pub description: String,
    pub dir: PathBuf,
    /// The full `SKILL.md`, frontmatter included: what an agent installs.
    pub text: String,
    pub html: String,
    pub fences: Vec<Fence>,
    pub references: Vec<RefFile>,
}

impl Skill {
    /// `cli`, `programming` or `tool`: the prefix the README groups by.
    pub fn group(&self) -> &str {
        self.name.split('-').next().unwrap_or("")
    }
}

pub fn discover(root: &Path, highlighter: &StHighlighter) -> Vec<Skill> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
        .expect("skills directory must exist")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("SKILL.md").is_file())
        .collect();
    dirs.sort();

    dirs.into_iter()
        .map(|dir| {
            let text = std::fs::read_to_string(dir.join("SKILL.md")).unwrap();
            let (frontmatter, body) = markdown::split_frontmatter(&text);
            let name = frontmatter.get("name").unwrap_or_default().to_string();
            let dir_name = dir.file_name().unwrap().to_string_lossy();
            assert_eq!(
                name, dir_name,
                "a skill's name must match its directory (Agent Skills spec)"
            );
            let rendered = markdown::render(body, highlighter);
            let fm_lines = text[..text.len() - body.len()].matches('\n').count();
            let fences = shift(rendered.fences, fm_lines);

            let mut references = Vec::new();
            let refs_dir = dir.join("references");
            if refs_dir.is_dir() {
                let mut files: Vec<PathBuf> = std::fs::read_dir(&refs_dir)
                    .unwrap()
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.extension().is_some_and(|e| e == "md"))
                    .collect();
                files.sort();
                for path in files {
                    let text = std::fs::read_to_string(&path).unwrap();
                    let (_, body) = markdown::split_frontmatter(&text);
                    let rendered = markdown::render(body, highlighter);
                    let fm_lines = text[..text.len() - body.len()].matches('\n').count();
                    let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                    references.push(RefFile {
                        rel: format!("references/{}", path.file_name().unwrap().to_string_lossy()),
                        title: rendered.title.clone().unwrap_or(stem),
                        fences: shift(rendered.fences, fm_lines),
                        html: rendered.html,
                        text,
                    });
                }
            }

            Skill {
                description: frontmatter
                    .get("description")
                    .unwrap_or_default()
                    .to_string(),
                name,
                dir,
                html: rendered.html,
                fences,
                references,
                text,
            }
        })
        .collect()
}

fn shift(mut fences: Vec<Fence>, by: usize) -> Vec<Fence> {
    for f in &mut fences {
        f.line += by;
    }
    fences
}

// ── The fence gate ───────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
enum Mode {
    Whole,
    Continues,
    Fragment,
    Decl,
    Syntax,
    Sketch,
}

struct Directive {
    mode: Mode,
    expect: Vec<String>,
}

fn parse_info(info: &str) -> Result<Directive, String> {
    let mut mode = Mode::Whole;
    let mut expect = Vec::new();
    for word in info.split_whitespace().skip(1) {
        match word {
            "continues" => mode = Mode::Continues,
            "fragment" => mode = Mode::Fragment,
            "decl" => mode = Mode::Decl,
            "syntax" => mode = Mode::Syntax,
            "sketch" => mode = Mode::Sketch,
            w if w.starts_with("expect=") => {
                expect.extend(w["expect=".len()..].split(',').map(str::to_string));
            }
            w => return Err(format!("unknown fence marker `{w}`")),
        }
    }
    Ok(Directive { mode, expect })
}

/// Where the compiler finds `Std.*`: `RK_STDLIB_PATH` if set, else the
/// checkout's own `stdlib/`. An empty variable loads nothing, as for `rk`.
fn stdlib_dir() -> Option<PathBuf> {
    match std::env::var("RK_STDLIB_PATH") {
        Ok(v) if v.is_empty() => None,
        Ok(v) => Some(PathBuf::from(v)),
        Err(_) => Some(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../stdlib")
                .canonicalize()
                .expect("stdlib/ must exist beside crates/"),
        ),
    }
}

/// Every diagnostic code the compiler reports for `source`, with whether it
/// is an error. A fresh database per call: the workspace file set is read
/// untracked, so one database cannot be reused across sources.
fn codes(source: &str, stdlib: &[(Url, String)]) -> Vec<(String, bool)> {
    let mut db = RootDatabase::default();
    for (url, text) in stdlib {
        let file = File::from_string()
            .db(&db)
            .parsers(&ast::RK_PARSER)
            .url(url)
            .source(text.clone())
            .call()
            .unwrap();
        db.insert_library_file(url.clone(), file);
    }
    let url = Url::parse("file:///fence.st").unwrap();
    let file = File::from_string()
        .db(&db)
        .parsers(&ast::RK_PARSER)
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();
    db.add_file(file).unwrap();

    diagnostics_for_file(&db, file)
        .iter()
        .filter_map(|d| {
            let code = match d.diagnostic.code.as_ref()? {
                NumberOrString::Number(n) => n.to_string(),
                NumberOrString::String(s) => s.clone(),
            };
            let is_error = d.diagnostic.severity == Some(DiagnosticSeverity::ERROR);
            Some((code, is_error))
        })
        .collect()
}

fn load_stdlib() -> Vec<(Url, String)> {
    let Some(dir) = stdlib_dir() else {
        return Vec::new();
    };
    let mut files = db::loader::find_st_files(&dir);
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let url = Url::from_file_path(&p).unwrap();
            (url, std::fs::read_to_string(&p).unwrap())
        })
        .collect()
}

fn wrap(mode: &Mode, code: &str) -> String {
    match mode {
        Mode::Fragment => format!("FUNCTION __Fragment : INT\n{code}\nEND_FUNCTION\n"),
        Mode::Decl => format!("FUNCTION_BLOCK __Decl\nVAR\n{code}\nEND_VAR\nEND_FUNCTION_BLOCK\n"),
        _ => code.to_string(),
    }
}

/// Compare what the compiler reported with what the fence promised.
fn report(
    at: &str,
    reported: &[(String, bool)],
    parse_only: bool,
    expect: &[String],
    problems: &mut Vec<String>,
) {
    let errors: Vec<&str> = reported
        .iter()
        .filter(|(c, is_error)| *is_error && (!parse_only || c.starts_with("E00")))
        .map(|(c, _)| c.as_str())
        .collect();
    let mut unexpected: Vec<&str> = errors
        .iter()
        .copied()
        .filter(|c| !expect.iter().any(|e| e == c))
        .collect();
    unexpected.sort();
    unexpected.dedup();
    let missing: Vec<&str> = expect
        .iter()
        .map(String::as_str)
        .filter(|e| !errors.contains(e))
        .collect();
    if unexpected.is_empty() && missing.is_empty() {
        eprintln!(" ok");
        return;
    }
    eprintln!(" FAIL");
    if !unexpected.is_empty() {
        problems.push(format!(
            "{at}: the example reports {} (mark it `expect=` if that is the point, or fix it)",
            unexpected.join(", ")
        ));
    }
    if !missing.is_empty() {
        problems.push(format!(
            "{at}: expected {} but the compiler did not report it",
            missing.join(", ")
        ));
    }
}

/// Check every fence of every skill. Returns one line per problem, naming
/// the file and line; empty means the site may be published.
pub fn verify(skills: &[Skill]) -> Vec<String> {
    let stdlib = load_stdlib();
    let mut problems = Vec::new();
    let mut checked = 0usize;
    let mut sketches = 0usize;

    for skill in skills {
        let mut docs: Vec<(PathBuf, &[Fence])> = vec![(skill.dir.join("SKILL.md"), &skill.fences)];
        for r in &skill.references {
            docs.push((skill.dir.join(&r.rel), &r.fences));
        }
        for (path, fences) in docs {
            let shown = path
                .strip_prefix(skill.dir.parent().unwrap().parent().unwrap())
                .unwrap_or(&path);
            // A `continues` chain accumulates here and is checked where it
            // ends: at the next non-chained fence, or the end of the document.
            let mut previous = String::new();
            let mut pending: Option<(String, String)> = None; // (location, source)

            let flush = |pending: &mut Option<(String, String)>,
                         problems: &mut Vec<String>,
                         checked: &mut usize| {
                if let Some((at, source)) = pending.take() {
                    eprint!("  {at} (chain)...");
                    *checked += 1;
                    report(&at, &codes(&source, &stdlib), false, &[], problems);
                }
            };

            for fence in fences {
                let at = format!("{}:{}", shown.display(), fence.line);
                let directive = match parse_info(&fence.info) {
                    Ok(d) => d,
                    Err(e) => {
                        problems.push(format!("{at}: {e}"));
                        continue;
                    }
                };
                if directive.mode == Mode::Sketch {
                    sketches += 1;
                    continue;
                }
                if directive.mode == Mode::Continues {
                    let source = format!("{previous}\n{}", fence.code);
                    previous = source.clone();
                    pending = Some((at, source));
                    continue;
                }
                flush(&mut pending, &mut problems, &mut checked);

                let source = wrap(&directive.mode, &fence.code);
                let parse_only =
                    matches!(directive.mode, Mode::Fragment | Mode::Decl | Mode::Syntax);
                if directive.mode == Mode::Whole {
                    previous = source.clone();
                }

                eprint!("  {at}...");
                checked += 1;
                report(
                    &at,
                    &codes(&source, &stdlib),
                    parse_only,
                    &directive.expect,
                    &mut problems,
                );
            }
            flush(&mut pending, &mut problems, &mut checked);
        }
    }
    eprintln!("  {checked} fences checked, {sketches} sketches shown unchecked");
    problems
}
