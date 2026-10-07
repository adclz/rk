// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A module addresses 4 GiB. `rk check` refuses what it can count (E0322: a
//! declaration, a frame, a configuration's globals and programs), and a
//! build refuses the rest, which only lowering counts: the static storage of
//! the POUs, the strings, a global's slot beside its copy in the band.

use rk::workspace::init_db;

use super::{disable_stdlib, temp_workspace};

/// A FUNCTION's array is static storage, which `rk check` does not count,
/// and with the global it passes 4 GiB: the build stops and names the
/// largest parts, where it emitted a module whose addresses wrapped.
#[test]
fn a_module_past_4_gib_is_refused_by_the_build() {
    let source = "FUNCTION Fill : DINT
VAR cells : ARRAY[0..799999999] OF DINT; END_VAR
    cells[0] := 1;
    Fill := cells[0];
END_FUNCTION

PROGRAM P
VAR_EXTERNAL buffer : ARRAY[0..299999999] OF DINT; END_VAR
VAR got : DINT; END_VAR
    got := Fill() + buffer[0];
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL buffer : ARRAY[0..299999999] OF DINT; END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
";
    let (_ws, root) = temp_workspace(&[("main.st", source)]);
    disable_stdlib();
    let db = init_db(&root, false, true).expect("init db");
    let error = rk::compiler::build_core_quiet(&db, &root, false).expect_err("past 4 GiB");
    assert_eq!(
        error,
        "compilation failed: the module's memory takes 4400016388 bytes, most of it 'cells' \
         (3200000000 bytes), 'buffer' (1200000000 bytes) and 'P1' (4 bytes): a module \
         addresses at most 4 GiB\n"
    );
}

/// A global is laid out once, in its band: 2.5 GB of them build, where the
/// slot first allocated and the copy in the band took 5 GB together.
#[test]
fn a_global_takes_its_size_once() {
    let source = "PROGRAM P
VAR_EXTERNAL samples : ARRAY[0..624999999] OF DINT; END_VAR
    samples[0] := 1;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL samples : ARRAY[0..624999999] OF DINT; END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P;
    END_RESOURCE
END_CONFIGURATION
";
    let (_ws, root) = temp_workspace(&[("main.st", source)]);
    disable_stdlib();
    let db = init_db(&root, false, true).expect("init db");
    let (_, mir) = rk::compiler::build_core_quiet(&db, &root, false).expect("2.5 GB fit");
    assert_eq!(mir.globals_size, 2_500_000_000);
    assert!(
        u64::from(mir.globals_base) + u64::from(mir.globals_size) < 2_600_000_000,
        "one copy, at {}",
        mir.globals_base
    );
}
