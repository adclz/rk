// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Do the debug tables survive their reader?
//!
//! Two oracles over `debug_format`. [`check`] is `fuzz_debug_format`'s:
//! its input is a wasm module, a seed's custom sections with a mutation,
//! and every entry point of the reader is called on whatever they say.
//! None may panic. The module's own bytes stand in for its memory and for
//! a frame's wasm locals. [`audit`] is the semantics oracle's, on a module
//! the compiler built and ran: what the tables say must hold.

use debug_format::{DebugInfo, Live, Place, VarValue};

use crate::Finding;

/// How much of a table one iteration visits: a mutated table can claim
/// more entries than an iteration should spend on.
const LIMIT: usize = 256;

/// A frame's start, when the bytes give none.
const BASE: u32 = 4096;

/// The module's bytes, read as its memory, its frames' wasm locals and its
/// live frames.
struct Bytes<'a>(&'a [u8]);

impl Live for Bytes<'_> {
    fn read(&mut self, address: u32, size: u32) -> Option<Vec<u8>> {
        let start = address as usize;
        self.0
            .get(start..start.checked_add(size as usize)?)
            .map(<[u8]>::to_vec)
    }

    fn local(&mut self, index: u32) -> Option<u32> {
        let at = (index as usize).checked_mul(4)?;
        let bytes = self.0.get(at..at.checked_add(4)?)?;
        Some(u32::from_le_bytes(bytes.try_into().ok()?))
    }

    fn frames(&mut self) -> Vec<(u32, u32)> {
        (0..4)
            .filter_map(|i| Some((i, self.local(i + 8)?)))
            .collect()
    }
}

/// A path through [`DebugInfo::follow`], its value read and decoded when
/// it has one.
fn touch(info: &DebugInfo, path: &str, live: &mut Bytes) {
    if let Some(Place::Value(loc)) = info.follow(path, live)
        && let Some(bytes) = live.read(loc.address, loc.size)
    {
        let _ = loc.decode(&bytes);
        let _ = loc.encode(&bytes, VarValue::I32(1));
    }
}

/// Every entry point of the reader, on `module`'s tables as they are:
/// nothing may panic, whatever they say.
pub fn check(module: &[u8]) -> Result<(), Finding> {
    let info = DebugInfo::from_wasm(module);
    let _ = info.problems();
    let _ = info.source_files();
    let _ = info.stack();
    let live = &mut Bytes(module);

    for sym in info.list_symbols().iter().take(LIMIT) {
        let _ = info.resolve(&sym.path);
        for tail in ["", "^", "[0]", ".x"] {
            touch(&info, &format!("{}{tail}", sym.path), live);
        }
        let _ = info.variable_at(sym.address);
        let _ = info.container_at(sym.address, &sym.path);
        if let Some(bytes) = live.read(sym.address, sym.size) {
            let _ = info.decode_symbol(sym, &bytes);
            let _ = info.decode_local(sym, &bytes);
        }
        let _ = info.encode_var(&sym.path, VarValue::I64(-1));
    }
    for p in info.list_pointers().iter().take(LIMIT) {
        let _ = info.pointer(&p.path);
        for tail in ["", "^", "^.x", "^[0]", "^^"] {
            touch(&info, &format!("{}{tail}", p.path), live);
        }
        let _ = info.variable_at(p.address);
    }
    for arr in info.arrays().iter().take(LIMIT) {
        for flat in [0, arr.total_elements.saturating_sub(1), arr.total_elements] {
            let path = debug_format::element_path(&arr.path, flat, &arr.dimensions);
            let _ = info.resolve(&path);
            for tail in ["", "^", ".x"] {
                touch(&info, &format!("{path}{tail}"), live);
            }
        }
        let _ = info.variable_at(arr.address);
    }

    // Each frame's start from the bytes, as a debugger reads it from the
    // slot.
    let frames: Vec<(u32, u32)> = info
        .frame_tables()
        .take(LIMIT)
        .map(|(index, table)| {
            let base = table.frame.as_ref().and_then(|f| live.local(f.slot));
            (index, base.unwrap_or(BASE))
        })
        .collect();
    for (index, locals) in info.frame_tables().take(LIMIT) {
        let base = frames
            .iter()
            .find(|(i, _)| *i == index)
            .map_or(BASE, |(_, base)| *base);
        for l in locals.locals.iter().take(LIMIT) {
            let _ = info.follow_local(index, &l.name, None, live);
        }
        for p in locals.local_pointers.iter().take(LIMIT) {
            for tail in ["", "^", "^.x"] {
                let _ = info.follow_local(index, &format!("{}{tail}", p.name), Some(base), live);
            }
        }
        for sym in locals.memory.iter().take(LIMIT) {
            let _ = info.resolve_local(index, &sym.path);
            let _ = info.follow_local(index, &sym.path, None, live);
            let _ = info.frame_variable_at(sym.address, &frames, None);
        }
        for p in locals.pointers.iter().take(LIMIT) {
            let _ = info.follow_local(index, &format!("{}^", p.path), Some(base), live);
        }
        for arr in locals.arrays.iter().take(LIMIT) {
            let first = debug_format::element_path(&arr.path, 0, &arr.dimensions);
            let _ = info.resolve_local(index, &first);
            let _ = info.follow_local(index, &format!("{first}^"), Some(base), live);
        }
        if let Some(frame) = &locals.frame {
            for sym in frame.memory.iter().take(LIMIT) {
                let _ = info.resolve_in_frame(index, &sym.path, base);
                let _ = info.follow_local(index, &sym.path, Some(base), live);
                let _ = info.frame_variable_at(base.wrapping_add(sym.address), &frames, Some("x"));
            }
            for p in frame.pointers.iter().take(LIMIT) {
                let _ = info.follow_local(index, &format!("{}^", p.path), Some(base), live);
            }
            for arr in frame.arrays.iter().take(LIMIT) {
                let first = debug_format::element_path(&arr.path, 0, &arr.dimensions);
                let _ = info.resolve_in_frame(index, &first, base);
                let _ = info.follow_local(index, &first, Some(base), live);
            }
            let _ = info.frame_variable_at(base, &frames, None);
        }
        let _ = info.function_name(index);
        let _ = info.resolve_frame(index, Some(BASE));
    }
    for (file, line) in info.breakable_lines().into_iter().take(LIMIT) {
        if let Some((defined, pc)) = info.line_to_pc(file, line) {
            let _ = info.function_at(pc);
            let _ = info.source_position(defined, pc);
            let _ = info.resolve_frame(defined, Some(pc));
            let _ = info.defined_index(pc);
        }
    }
    let _ = info.function_at(u32::MAX);
    let _ = info.variable_at(u32::MAX);
    let _ = info.read_all_bytes(module);
    Ok(())
}

/// The memory of a module that ran.
struct Memory<'a>(&'a [u8]);

impl Live for Memory<'_> {
    fn read(&mut self, address: u32, size: u32) -> Option<Vec<u8>> {
        let start = address as usize;
        self.0
            .get(start..start.checked_add(size as usize)?)
            .map(<[u8]>::to_vec)
    }
}

/// What the tables of a module the compiler built must say, against its
/// memory after a run: a leaf resolves to where it is listed and names
/// itself by address, a pointer follows, a descriptor's elements resolve
/// and nothing past them, a frame's locals resolve, and a breakable line's
/// offset is in its function.
pub(crate) fn audit(info: &DebugInfo, memory: &[u8]) -> Result<(), Finding> {
    let bad = |what: String| Finding::new("debug-tables", what);
    let live = &mut Memory(memory);
    for sym in info.list_symbols() {
        let Some(loc) = info.resolve(&sym.path) else {
            return Err(bad(format!("`{}` does not resolve", sym.path)));
        };
        if loc.address != sym.address {
            return Err(bad(format!(
                "`{}` resolves to {}, listed at {}",
                sym.path, loc.address, sym.address
            )));
        }
        match info.follow(&sym.path, live) {
            Some(Place::Value(at)) if at.address == sym.address => {}
            other => return Err(bad(format!("`{}` follows to {other:?}", sym.path))),
        }
        let Some(named) = info.variable_at(sym.address) else {
            return Err(bad(format!(
                "nothing is named at {}, where `{}` is",
                sym.address, sym.path
            )));
        };
        if info.resolve(&named).map(|l| l.address) != Some(sym.address) {
            return Err(bad(format!(
                "`{named}` is named at {}, where `{}` is, and resolves elsewhere",
                sym.address, sym.path
            )));
        }
    }
    for p in info.list_pointers() {
        // A VAR_IN_OUT reads as the variable it is bound to; a REF_TO is
        // the pointer itself, until `^`.
        match (p.by_ref, info.follow(&p.path, live)) {
            (false, Some(Place::Pointer(_) | Place::Null { .. })) => {}
            (true, Some(Place::Value(_) | Place::Aggregate { .. } | Place::Null { .. })) => {}
            (_, other) => return Err(bad(format!("pointer `{}` follows to {other:?}", p.path))),
        }
        if p.target.is_some() && info.follow(&format!("{}^", p.path), live).is_none() {
            return Err(bad(format!(
                "`{}^` resolves to nothing, with a target",
                p.path
            )));
        }
    }
    for arr in info.arrays() {
        if arr.total_elements == 0 {
            continue;
        }
        for flat in [0, arr.total_elements - 1] {
            let path = debug_format::element_path(&arr.path, flat, &arr.dimensions);
            if info.follow(&path, live).is_none() {
                return Err(bad(format!("element `{path}` resolves to nothing")));
            }
        }
        if let [(_, hi)] = arr.dimensions.as_slice() {
            let past = format!("{}[{}]", arr.path, hi + 1);
            if info.follow(&past, live).is_some() {
                return Err(bad(format!("`{past}` is past the array and resolves")));
            }
        }
    }
    for (index, locals) in info.frame_tables() {
        for sym in &locals.memory {
            if info.resolve_local(index, &sym.path).map(|l| l.address) != Some(sym.address) {
                return Err(bad(format!(
                    "`{}` of function {index} does not resolve to {}",
                    sym.path, sym.address
                )));
            }
        }
        let Some(frame) = &locals.frame else {
            continue;
        };
        for sym in &frame.memory {
            if info
                .resolve_in_frame(index, &sym.path, 0)
                .map(|l| l.address)
                != Some(sym.address)
            {
                return Err(bad(format!(
                    "`{}` in the frame of function {index} does not resolve to {}",
                    sym.path, sym.address
                )));
            }
            if sym
                .address
                .checked_add(sym.size)
                .is_none_or(|end| end > frame.size)
            {
                return Err(bad(format!(
                    "`{}` at {} is past its frame of {} bytes",
                    sym.path, sym.address, frame.size
                )));
            }
        }
    }
    for (file, line) in info.breakable_lines() {
        let Some((defined, pc)) = info.line_to_pc(file, line) else {
            return Err(bad(format!(
                "line {line} of file {file} is breakable and has no offset"
            )));
        };
        if info.function_at(pc) != Some(defined) {
            return Err(bad(format!(
                "offset {pc} of function {defined} is placed in {:?}",
                info.function_at(pc)
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    /// Pointers of every kind, an array of structs, an enum, a STRING and
    /// a recursive function, under a configuration.
    const SOURCE: &str = r#"
        TYPE Node : STRUCT value : INT; next : REF_TO Node; END_STRUCT; END_TYPE
        TYPE Pt : STRUCT x : REAL; y : REAL; END_STRUCT; END_TYPE
        TYPE Lamp : (Off, On); END_TYPE

        FUNCTION_BLOCK Motor
        VAR_IN_OUT speed : INT; END_VAR
        VAR target : REF_TO Pt; count : INT; END_VAR
            speed := speed + 1;
            count := count + 1;
        END_FUNCTION_BLOCK

        FUNCTION Depth : INT
        VAR_INPUT n : INT; END_VAR
        VAR trail : ARRAY[0..3] OF INT; pt : Pt; link : REF_TO Pt; END_VAR
            trail[0] := n;
            pt.x := n;
            link := REF(pt);
            IF n > 0 THEN Depth := Depth(n - 1) + 1; ELSE Depth := 0; END_IF;
        END_FUNCTION

        PROGRAM Main
        VAR
            a : Node; b : Node; head : REF_TO Node;
            pt : Pt; rp : REF_TO Pt; ri : REF_TO INT;
            refs : ARRAY[0..2] OF REF_TO INT;
            pts : ARRAY[0..9] OF Pt;
            lamp : Lamp;
            name : STRING[8];
            n : INT;
            m : Motor;
        END_VAR
            a.value := 1; b.value := 2;
            a.next := REF(b); head := REF(a);
            rp := REF(pt); ri := REF(n); refs[1] := REF(n);
            pts[3].y := 1.5;
            lamp := Lamp#On;
            name := 'abc';
            m(speed := n);
            n := n + Depth(2);
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL hold : Pt; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;

    /// A compiled module's tables as they are, and under a few hundred
    /// single-byte mutations: no panic. What `fuzz_debug_format` does,
    /// without libFuzzer.
    #[test]
    fn mutated_tables_do_not_panic() {
        let module = crate::corpus::debug_module(SOURCE).expect("the program compiles");
        super::check(&module).expect("the tables as they are");
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        for _ in 0..400 {
            let mut mutated = module.clone();
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let at = (seed as usize) % mutated.len();
            mutated[at] ^= ((seed >> 32) as u8) | 1;
            let _ = super::check(&mutated);
        }
    }

    /// The audit holds on a module the compiler built and ran.
    #[test]
    fn a_built_module_passes_the_audit() {
        let (db, file) = crate::session::load(SOURCE).expect("parses");
        assert!(crate::session::compiles(&db, file));
        let index = hir::hir_def::semantic_index::semantic_index(&db, file);
        let mir = mir::lower::lower_module::lower_module(&db, index).expect("lowers");
        let wasm = wasm_codegen::generate_wasm(&db, &mir).finish();
        let sections = crate::wasm::decode_sections(&wasm).expect("sections");
        let memory = crate::wasm::memory_after(&wasm, &sections, 2)
            .expect("runs")
            .expect("does not trap");
        super::audit(sections.info(), &memory).expect("the tables hold");
        assert!(!sections.info().list_pointers().is_empty());
        assert!(
            sections
                .info()
                .frame_tables()
                .any(|(_, t)| t.frame.is_some())
        );
    }
}
