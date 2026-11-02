// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

pub mod call_hierarchy;
pub mod code_lens;
pub mod comments;
pub mod completion_snippets;
pub mod definition;
pub mod denormalize;
pub mod document_highlight;
pub mod document_links;
pub mod document_symbols;
pub mod formatter;
pub mod formatter_preserves_meaning;
pub mod formatter_stdlib;
pub mod hover;
pub mod implementations;
pub mod inlay_hints;
pub mod inline_value;
pub mod references;
pub mod rename;
pub mod semantic_tokens;
pub mod signature_help;
pub mod type_definition;
pub mod workspace_symbols;

/// A program configuration that names something of every kind: inputs fed by
/// an address and a global, an output copied to a global, a function block
/// run by a task of its own, and a VAR_CONFIG path through an instance.
pub(crate) const CONNECTED: &str = r#"
FUNCTION_BLOCK Counter
VAR_OUTPUT n : INT; END_VAR
    n := n + 1;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Drive
VAR out AT %Q* : INT; END_VAR
END_FUNCTION_BLOCK

PROGRAM F
VAR_INPUT x1 : BOOL; x2 : UINT; END_VAR
VAR_OUTPUT y1 : UINT; END_VAR
VAR fb1 : Counter; d : Drive; END_VAR
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL w : UINT := 5; total : UINT; END_VAR
VAR_CONFIG
    Res.P1.d.out AT %QW0 : INT;
END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        TASK FAST(INTERVAL := T#5ms, PRIORITY := 0);
        PROGRAM P1 WITH T : F(x1 := %IX0.0, x2 := w, y1 => total, fb1 WITH FAST);
    END_RESOURCE
END_CONFIGURATION
"#;

/// A configuration whose tasks read everything a SINGLE can name.
pub(crate) const TRIGGERED: &str = r#"
TYPE Flags : STRUCT go : BOOL; END_STRUCT; END_TYPE

FUNCTION_BLOCK Fb
VAR_OUTPUT q : BOOL; END_VAR
    q := NOT q;
END_FUNCTION_BLOCK

PROGRAM Prog
VAR_OUTPUT done : BOOL; bits : ARRAY[1..3] OF BOOL; END_VAR
    done := NOT done;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL flags : Flags; start : BOOL; timer : Fb; END_VAR
VAR_GLOBAL CONSTANT which : INT := 2; END_VAR
    RESOURCE Res ON CPU
        TASK A(SINGLE := start, INTERVAL := T#10ms, PRIORITY := 1);
        TASK B(SINGLE := Res.flags.go, PRIORITY := 2);
        TASK C(SINGLE := inst.done, PRIORITY := 3);
        TASK D(SINGLE := inst.bits[which], PRIORITY := 4);
        TASK E(SINGLE := %IX0.0, PRIORITY := 5);
        TASK F(SINGLE := timer.q, PRIORITY := 6);
        PROGRAM inst WITH A : Prog;
        PROGRAM other WITH B : Prog;
    END_RESOURCE
END_CONFIGURATION
"#;

/// Where `needle` is written in [`TRIGGERED`], searching from the
/// CONFIGURATION.
pub(crate) fn triggered_at(needle: &str) -> usize {
    let from = TRIGGERED.find("CONFIGURATION").unwrap();
    from + TRIGGERED[from..].find(needle).expect(needle)
}

/// Where `needle` is written in [`CONNECTED`], searching from the
/// CONFIGURATION when `in_config`.
pub(crate) fn connected_at(needle: &str, in_config: bool) -> usize {
    let from = if in_config {
        CONNECTED.find("CONFIGURATION").unwrap()
    } else {
        0
    };
    from + CONNECTED[from..].find(needle).expect(needle)
}
