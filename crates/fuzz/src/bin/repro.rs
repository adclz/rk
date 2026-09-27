//! Run the fuzzing oracles on files and say which one fails.
//!
//! `cargo run -p rk-fuzz --bin repro -- [--only pipeline|format|incremental] <path>...`
//!
//! With `--generated`, each input is a `fuzz_semantics` crash file: the
//! bytes the program generator ran on. The program is written again and
//! checked, and printed when it fails.
//!
//! A path is a `.st` file, a libFuzzer crash file, or a directory of either.
//! Each input gets one line, `ok`, `FAIL [oracle]` or `PANIC at <site>`,
//! and the run ends with the failures grouped by cause, so a night's crash
//! files read as a handful of bugs. The exit status is 1 when anything
//! failed.
//!
//! This is the harness's front door for a person or an agent with a
//! hypothesis ("CASE over a subrange mis-lowers"): write the program that
//! should break it, run it here, and the verdict is reproducible on the
//! stable toolchain, with no libFuzzer build.

use std::collections::BTreeMap;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;

use rk_fuzz::Finding;
use rk_fuzz::pipeline::Verdict;

/// Where the last panic happened: the grouping key for crashes.
static PANIC_SITE: Mutex<Option<String>> = Mutex::new(None);

fn main() -> ExitCode {
    let mut only = None;
    let mut generated = false;
    let mut paths = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--only" => only = args.next(),
            "--generated" => generated = true,
            "-h" | "--help" => {
                eprintln!(
                    "usage: repro [--only pipeline|format|incremental] [--generated] <file-or-dir>..."
                );
                return ExitCode::SUCCESS;
            }
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    if paths.is_empty() {
        eprintln!(
            "usage: repro [--only pipeline|format|incremental] [--generated] <file-or-dir>..."
        );
        return ExitCode::FAILURE;
    }
    // The pipeline's verdict, when it ran: whether an input reached the
    // code generator is the first thing to know about a hypothesis.
    let check: fn(&str) -> Result<Option<Verdict>, Finding> = match only.as_deref() {
        None => |s| rk_fuzz::check_all(s).map(Some),
        Some("pipeline") => |s| rk_fuzz::pipeline::check(s).map(Some),
        Some("format") => |s| rk_fuzz::format::check(s).map(|()| None),
        Some("incremental") => |s| rk_fuzz::incremental::check(s).map(|()| None),
        Some(other) => {
            eprintln!("unknown oracle `{other}`");
            return ExitCode::FAILURE;
        }
    };

    panic::set_hook(Box::new(|info| {
        let site = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "an unknown site".to_string());
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        *PANIC_SITE.lock().unwrap() = Some(format!("{site}: {message}"));
    }));

    let mut files = Vec::new();
    for path in &paths {
        collect(path, &mut files);
    }
    let mut failures: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    let mut verdicts: BTreeMap<String, usize> = BTreeMap::new();
    for file in &files {
        let Ok(bytes) = std::fs::read(file) else {
            println!("skip   {} (unreadable)", file.display());
            continue;
        };
        let program;
        let source = match generated {
            true => {
                program = rk_fuzz::generate::program(&bytes);
                program.as_str()
            }
            false => match std::str::from_utf8(&bytes) {
                Ok(source) => source,
                Err(_) => {
                    // The fuzz targets reject these before any code runs.
                    println!("skip   {} (not UTF-8)", file.display());
                    continue;
                }
            },
        };
        match panic::catch_unwind(AssertUnwindSafe(|| check(source))) {
            Ok(Ok(verdict)) => {
                let how = match verdict {
                    Some(Verdict::Unparsed) => " (not parsed)",
                    Some(Verdict::Rejected) => " (rejected by check)",
                    Some(Verdict::Compiled) => " (compiled and ran)",
                    None => "",
                };
                *verdicts.entry(how.trim().to_string()).or_default() += 1;
                println!("ok     {}{how}", file.display());
            }
            Ok(Err(finding)) => {
                println!("FAIL   {} [{}]", file.display(), finding.oracle);
                for line in finding.detail.lines() {
                    println!("         {line}");
                }
                if generated {
                    println!("         --- the program ---");
                    for line in source.lines() {
                        println!("         {line}");
                    }
                }
                // The oracle and what it says first: two lowering errors
                // are two bugs, one lowering error on two inputs is one.
                let first = finding.detail.lines().next().unwrap_or_default();
                failures
                    .entry(format!("[{}] {}", finding.oracle, masked(first)))
                    .or_default()
                    .push(file.clone());
            }
            Err(_) => {
                let site = PANIC_SITE.lock().unwrap().take().unwrap_or_default();
                println!("PANIC  {} at {site}", file.display());
                // The site groups them; the message can hold the input.
                let key = site.split(": ").next().unwrap_or_default().to_string();
                failures
                    .entry(format!("panic at {key}"))
                    .or_default()
                    .push(file.clone());
            }
        }
    }

    let passed: Vec<String> = verdicts
        .iter()
        .filter(|(how, _)| !how.is_empty())
        .map(|(how, n)| format!("{n} {}", how.trim_matches(['(', ')'])))
        .collect();
    if !passed.is_empty() {
        println!("\npassed: {}", passed.join(", "));
    }
    if failures.is_empty() {
        println!("{} input(s), no failure", files.len());
        return ExitCode::SUCCESS;
    }
    println!(
        "{} input(s), {} distinct failure(s):",
        files.len(),
        failures.len()
    );
    for (cause, inputs) in &failures {
        // The smallest input first: the one to read.
        let smallest = inputs
            .iter()
            .min_by_key(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(u64::MAX))
            .expect("at least one input");
        println!(
            "  {cause} — {} input(s), smallest {}",
            inputs.len(),
            smallest.display()
        );
    }
    ExitCode::FAILURE
}

/// `text` with every number masked, decimal or hex (a salsa id, a line), so
/// two inputs that trip the same bug share a group.
fn masked(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        match run.chars().any(|c| c.is_ascii_digit()) {
            true => out.push('#'),
            false => out.push_str(run),
        }
        run.clear();
    };
    for c in text.chars() {
        if c.is_ascii_hexdigit() {
            run.push(c);
        } else {
            flush(&mut run, &mut out);
            out.push(c);
        }
    }
    flush(&mut run, &mut out);
    out
}

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(path)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();
        entries.sort();
        for entry in entries {
            collect(&entry, out);
        }
    } else {
        out.push(path.to_path_buf());
    }
}
