// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A generated workspace, for what the standard library does not show:
//! work that grows faster than the workspace. The standard library is 13
//! files whose POUs sit in namespaces; a lookup that walked every POU of the
//! workspace cost nothing there, and 21 s on 10,000 POUs.
//!
//! Each file holds 25 POUs: an enum, a STRUCT, 12 FUNCTIONs, 10
//! FUNCTION_BLOCKs and a PROGRAM. The FUNCTIONs come in three layers, so a
//! chain of calls stays three deep however large the workspace is: leaves,
//! FUNCTIONs that call two leaves of other files, and FUNCTIONs that call two
//! of those. Half the blocks hold an instance of another file's block, call
//! it and its METHOD; a CONFIGURATION runs every PROGRAM. The text is the
//! same on every run: a measurement must not depend on a random draw.

use crate::Corpus;

/// POUs in one generated file.
pub const POUS_PER_FILE: usize = 25;

/// The file whose names file `f` calls or instantiates, picked by `salt`:
/// spread over the workspace, and never `f` itself when there is another.
/// A multiplicative hash picks it, so that it spreads at any size: `7f +
/// 13·salt` modulo the 39 other files of 40 gave each file three others.
fn other(f: usize, salt: usize, files: usize) -> usize {
    if files == 1 {
        return 0;
    }
    let mixed = (((f as u64) << 32) | salt as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32;
    (f + 1 + mixed as usize % (files - 1)) % files
}

fn file_source(f: usize, files: usize) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "TYPE Mode{f} : (Idle, Run, Stop); END_TYPE\n\n\
         TYPE Sample{f} :\nSTRUCT\n    value : INT;\n    valid : BOOL;\nEND_STRUCT;\nEND_TYPE\n\n"
    ));

    // Leaves: arithmetic and an IF.
    for i in 0..4 {
        out.push_str(&format!(
            "FUNCTION Scale{f}_{i} : INT\nVAR_INPUT\n    x : INT;\nEND_VAR\nVAR\n    y : INT;\nEND_VAR\n    \
             y := x * {factor};\n    IF y > 100 THEN\n        y := y - 100;\n    END_IF;\n    \
             Scale{f}_{i} := y;\nEND_FUNCTION\n\n",
            factor = i + 2
        ));
    }
    // Two leaves of other files.
    for i in 4..8 {
        let (g1, g2) = (other(f, i, files), other(f, i + 20, files));
        let (j1, j2) = ((f + i) % 4, (f + i + 1) % 4);
        out.push_str(&format!(
            "FUNCTION Scale{f}_{i} : INT\nVAR_INPUT\n    x : INT;\nEND_VAR\n    \
             Scale{f}_{i} := Scale{g1}_{j1}(x := x) + Scale{g2}_{j2}(x := x + 1);\nEND_FUNCTION\n\n"
        ));
    }
    // Two of the middle layer, one of them in a FOR.
    for i in 8..12 {
        let (g1, g2) = (other(f, i, files), other(f, i + 40, files));
        let (m1, m2) = (4 + (f + i) % 4, 4 + (f + i + 3) % 4);
        out.push_str(&format!(
            "FUNCTION Scale{f}_{i} : INT\nVAR_INPUT\n    x : INT;\nEND_VAR\nVAR\n    k : INT;\n    sum : INT;\nEND_VAR\n    \
             sum := 0;\n    FOR k := 1 TO 3 DO\n        sum := sum + Scale{g1}_{m1}(x := x + k);\n    END_FOR;\n    \
             Scale{f}_{i} := sum + Scale{g2}_{m2}(x := x);\nEND_FUNCTION\n\n"
        ));
    }

    // Blocks with a state machine on the file's enum, and a METHOD.
    for j in 0..5 {
        let g = other(f, j + 60, files);
        let t = 8 + (f + j) % 4;
        out.push_str(&format!(
            "FUNCTION_BLOCK Unit{f}_{j}\nVAR_INPUT\n    enable : BOOL;\n    setpoint : INT;\nEND_VAR\n\
             VAR_OUTPUT\n    out : INT;\n    done : BOOL;\nEND_VAR\n\
             VAR\n    mode : Mode{f};\n    sample : Sample{f};\nEND_VAR\n\
             METHOD Reset\n    out := 0;\n    mode := Mode{f}#Idle;\nEND_METHOD\n    \
             IF NOT enable THEN\n        mode := Mode{f}#Idle;\n    END_IF;\n    \
             CASE mode OF\n        \
             Mode{f}#Idle:\n            IF enable THEN\n                mode := Mode{f}#Run;\n            END_IF;\n        \
             Mode{f}#Run:\n            sample.value := Scale{g}_{t}(x := setpoint);\n            sample.valid := TRUE;\n            mode := Mode{f}#Stop;\n        \
             Mode{f}#Stop:\n            done := sample.valid;\n    END_CASE;\n    \
             out := sample.value;\nEND_FUNCTION_BLOCK\n\n"
        ));
    }
    // Blocks holding another file's block.
    for j in 5..10 {
        let g = other(f, j + 80, files);
        let inner = j - 5;
        out.push_str(&format!(
            "FUNCTION_BLOCK Unit{f}_{j}\nVAR_INPUT\n    enable : BOOL;\n    setpoint : INT;\nEND_VAR\n\
             VAR_OUTPUT\n    out : INT;\n    done : BOOL;\nEND_VAR\n\
             VAR\n    inner : Unit{g}_{inner};\nEND_VAR\n    \
             inner(enable := enable, setpoint := setpoint * 2);\n    out := inner.out;\n    done := inner.done;\n    \
             IF inner.done THEN\n        inner.Reset();\n    END_IF;\nEND_FUNCTION_BLOCK\n\n"
        ));
    }

    out.push_str(&format!(
        "PROGRAM Line{f}\nVAR\n    a : Unit{f}_5;\n    b : Unit{f}_6;\n    start : BOOL;\n    level : INT;\nEND_VAR\n    \
         a(enable := start, setpoint := level);\n    b(enable := a.done, setpoint := a.out);\n    \
         level := Scale{f}_8(x := b.out);\nEND_PROGRAM\n"
    ));
    out
}

/// A workspace of `files` generated files, `POUS_PER_FILE` POUs each, and
/// the CONFIGURATION that runs their PROGRAMs.
pub fn generated_corpus(name: &'static str, files: usize) -> Corpus {
    let mut sources: Vec<(String, String)> = (0..files)
        .map(|f| (format!("generated/f{f:04}.st"), file_source(f, files)))
        .collect();
    let mut plant = String::from(
        "CONFIGURATION Plant\n    RESOURCE Res ON CPU\n        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);\n",
    );
    for f in 0..files {
        plant.push_str(&format!("        PROGRAM L{f} WITH Fast : Line{f};\n"));
    }
    plant.push_str("    END_RESOURCE\nEND_CONFIGURATION\n");
    sources.push(("generated/plant.st".to_string(), plant));
    Corpus {
        name,
        files: sources,
    }
}
