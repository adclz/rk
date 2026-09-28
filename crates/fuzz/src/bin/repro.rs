//! Run the fuzzing oracles on files and say which one fails.
//!
//! `cargo run -p rk-fuzz --bin repro -- [--only pipeline|semantics|format|incremental|ide] <path>...`
//!
//! With `--generated`, each input is a `fuzz_semantics` crash file: the
//! bytes the program generator ran on. The program is written again and
//! checked, and printed when it fails; `--print` only prints it.
//!
//! With `--isolate`, each input runs in a child process, so an input that
//! kills it (a stack overflow cannot be caught) is reported as a crash, and
//! the other inputs still run. The nightly triage uses it.
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
    let mut isolate = false;
    let mut print = false;
    let mut paths = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--only" => only = args.next(),
            "--generated" => generated = true,
            "--isolate" => isolate = true,
            "--print" => print = true,
            "-h" | "--help" => {
                eprintln!(
                    "usage: repro [--only pipeline|semantics|format|incremental|ide] [--generated [--print]] [--isolate] <file-or-dir>..."
                );
                return ExitCode::SUCCESS;
            }
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    if paths.is_empty() {
        eprintln!(
            "usage: repro [--only pipeline|semantics|format|incremental|ide] [--generated [--print]] [--isolate] <file-or-dir>..."
        );
        return ExitCode::FAILURE;
    }
    // The pipeline's verdict, when it ran: whether an input reached the
    // code generator is the first thing to know about a hypothesis.
    let check: fn(&str) -> Result<Option<Verdict>, Finding> = match only.as_deref() {
        // A generated program gets what `fuzz_semantics` gives it.
        None if generated => |s| rk_fuzz::check_generated(s).map(Some),
        None => |s| rk_fuzz::check_all(s).map(Some),
        Some("pipeline") => |s| rk_fuzz::pipeline::check(s).map(Some),
        Some("format") => |s| rk_fuzz::format::check(s).map(|()| None),
        Some("incremental") => |s| rk_fuzz::incremental::check(s).map(|()| None),
        Some("ide") => |s| rk_fuzz::ide::check(s).map(|()| None),
        Some("semantics") => |s| rk_fuzz::semantics::check(s).map(|()| None),
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
    // What a child gets: the same checks, one input.
    let mut flags: Vec<String> = Vec::new();
    if let Some(only) = &only {
        flags.extend(["--only".to_string(), only.clone()]);
    }
    if generated {
        flags.push("--generated".to_string());
    }
    for file in &files {
        if isolate {
            match isolated(file, &flags) {
                Ok(how) => *verdicts.entry(how).or_default() += 1,
                Err(cause) => failures.entry(cause).or_default().push(file.clone()),
            }
            continue;
        }
        let Ok(bytes) = std::fs::read(file) else {
            println!("skip   {} (unreadable)", file.display());
            continue;
        };
        let program;
        let source = match generated {
            true => {
                program = rk_fuzz::generate::program(&bytes);
                if print {
                    println!("{program}");
                    continue;
                }
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

    // The programs, and nothing else: the output is meant to be saved.
    if print && generated {
        return ExitCode::SUCCESS;
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

/// Check `file` in a child process: `Ok` holds how it passed (`(compiled
/// and ran)`), `Err` the cause it failed with. The child's lines about the
/// input are passed through.
fn isolated(file: &Path, flags: &[String]) -> Result<String, String> {
    let child = std::env::current_exe().and_then(|exe| {
        std::process::Command::new(exe)
            .args(flags)
            .arg(file)
            .env("RK_FUZZ_TRACE", "1")
            .output()
    });
    let out = match child {
        Ok(out) => out,
        Err(e) => return Err(format!("could not run a child: {e}")),
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Its summary starts with a blank line, `passed:` or the input count.
    let about_input = stdout.lines().take_while(|l| {
        !l.is_empty() && !l.starts_with("passed:") && !l.starts_with(|c: char| c.is_ascii_digit())
    });
    for line in about_input {
        println!("{line}");
    }
    match out.status.code() {
        Some(0) => Ok(stdout
            .lines()
            .next()
            .and_then(|l| l.rsplit_once(" ("))
            .map(|(_, how)| format!("({how}"))
            .unwrap_or_default()),
        Some(1) => Err(stdout
            .lines()
            .filter_map(|l| l.strip_prefix("  "))
            .find_map(|l| l.split_once(" — ").map(|(cause, _)| cause.to_string()))
            .unwrap_or_else(|| "a failure the child did not name".to_string())),
        _ => {
            // Killed: the runtime's last words, and the request it was on
            // (the IDE oracle names each one under RK_FUZZ_TRACE).
            let stderr = String::from_utf8_lossy(&out.stderr);
            let why = stderr
                .lines()
                .rev()
                .find(|l| l.contains("fatal") || l.contains("overflowed"))
                .or_else(|| stderr.lines().rev().find(|l| !l.trim().is_empty()))
                .unwrap_or("killed by a signal")
                .trim();
            let during = stderr
                .lines()
                .rev()
                .find_map(|l| l.strip_prefix("request: "))
                .map(|r| format!(", in {}", masked(r)))
                .unwrap_or_default();
            println!("CRASH  {} ({why}{during})", file.display());
            Err(format!("crash: {}{during}", masked(why)))
        }
    }
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
