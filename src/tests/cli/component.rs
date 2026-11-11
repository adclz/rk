// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A built module wrapped into a component, run as a component host runs
//! it: through the control interface, with nothing but its typed functions
//! to call.

use debug_format::test_manifest::{TEST_MANIFEST_SECTION, TestManifest};
use rstest::rstest;
use wasmtime::component::{Component, Instance, Linker, TypedFunc};

use rk::control::{MemoryError, Range, ScanError, TestError};

use crate::tests::codegen::{
    compile_to_mir_and_wasm_as_built, compile_to_wasm_as_built, compile_with_libraries,
    expect_section, with_db,
};

const CONTROL: &str = debug_format::wit::CONTROL_INTERFACE;

/// The interface the `{export}` FUNCTIONs are in: the project's name, and
/// `plc` for these tests, which have no project.
const EXPORTS: &str = "plc";

/// A task that divides by its input.
const DIVIDER: &str = r#"
    PROGRAM P
    VAR_EXTERNAL input : DINT; output : DINT; END_VAR
        output := 1000 / input;
    END_PROGRAM

    CONFIGURATION Cfg
    VAR_GLOBAL
        input AT %ID0 : DINT;
        output AT %QD0 : DINT;
    END_VAR
        RESOURCE Res ON CPU
            TASK T(INTERVAL := T#10ms, PRIORITY := 1);
            PROGRAM Main WITH T : P;
        END_RESOURCE
    END_CONFIGURATION
"#;

fn engine() -> wasmtime::Engine {
    let mut config = wasmtime::Config::new();
    config.wasm_exceptions(true);
    wasmtime::Engine::new(&config).expect("an engine")
}

/// A component instantiated, its imports trapping.
struct Plc {
    store: wasmtime::Store<()>,
    instance: Instance,
}

impl Plc {
    fn load(bytes: &[u8]) -> Self {
        Self::load_with(bytes, |_| {})
    }

    /// [`Plc::load`] with `wire` defining host functions first: the
    /// imports it leaves undefined trap.
    fn load_with(bytes: &[u8], wire: impl FnOnce(&mut Linker<()>)) -> Self {
        let engine = engine();
        let component = Component::new(&engine, bytes).expect("a valid component");
        let mut linker = Linker::<()>::new(&engine);
        wire(&mut linker);
        linker
            .define_unknown_imports_as_traps(&component)
            .expect("its imports stubbed");
        let mut store = wasmtime::Store::new(&engine, ());
        let instance = linker
            .instantiate(&mut store, &component)
            .expect("the component instantiates");
        Self { store, instance }
    }

    fn func<P, R>(&mut self, name: &str) -> TypedFunc<P, R>
    where
        P: wasmtime::component::ComponentNamedList + wasmtime::component::Lower,
        R: wasmtime::component::ComponentNamedList + wasmtime::component::Lift,
    {
        self.function_of(CONTROL, name)
    }

    /// The function `name` of the interface the component exports as
    /// `interface`, typed as the caller says: a type the WIT does not give
    /// it is refused here.
    fn function_of<P, R>(&mut self, interface: &str, name: &str) -> TypedFunc<P, R>
    where
        P: wasmtime::component::ComponentNamedList + wasmtime::component::Lower,
        R: wasmtime::component::ComponentNamedList + wasmtime::component::Lift,
    {
        let exported = self
            .instance
            .get_export_index(&mut self.store, None, interface)
            .unwrap_or_else(|| panic!("the component exports `{interface}`"));
        let func = self
            .instance
            .get_export_index(&mut self.store, Some(&exported), name)
            .unwrap_or_else(|| panic!("the interface has `{name}`"));
        self.instance
            .get_typed_func::<P, R>(&mut self.store, &func)
            .unwrap_or_else(|e| panic!("`{name}`: {e}"))
    }

    /// Calls the `{export}` FUNCTION a host knows as `name`.
    fn call<P, T>(&mut self, name: &str, params: P) -> Result<T, String>
    where
        P: wasmtime::component::ComponentNamedList + wasmtime::component::Lower,
        (Result<T, String>,): wasmtime::component::ComponentNamedList + wasmtime::component::Lift,
    {
        let function = self.function_of::<P, (Result<T, String>,)>(EXPORTS, name);
        function
            .call(&mut self.store, params)
            .unwrap_or_else(|e| panic!("`{name}` trapped: {e:#}"))
            .0
    }

    fn init(&mut self) -> Result<(), String> {
        let init = self.func::<(), (Result<(), String>,)>("init");
        init.call(&mut self.store, ()).expect("init returns").0
    }

    fn run_task(&mut self, task: u32, inputs: &[u8]) -> Result<Vec<u8>, ScanError> {
        let run = self.func::<(u32, &[u8]), (Result<Vec<u8>, ScanError>,)>("run-task");
        run.call(&mut self.store, (task, inputs))
            .expect("run-task returns")
            .0
    }

    fn run_test(&mut self, test: u32) -> Result<(), TestError> {
        let run = self.func::<(u32,), (Result<(), TestError>,)>("run-test");
        run.call(&mut self.store, (test,))
            .expect("run-test returns")
            .0
    }

    fn save_retain(&mut self) -> Vec<u8> {
        let save = self.func::<(), (Vec<u8>,)>("save-retain");
        save.call(&mut self.store, ())
            .expect("save-retain returns")
            .0
    }

    fn restore_retain(&mut self, retained: &[u8]) {
        let restore = self.func::<(&[u8],), ()>("restore-retain");
        restore
            .call(&mut self.store, (retained,))
            .expect("restore-retain returns");
    }

    fn read_memory(&mut self, ranges: &[Range]) -> Result<Vec<u8>, MemoryError> {
        let read = self.func::<(&[Range],), (Result<Vec<u8>, MemoryError>,)>("read-memory");
        read.call(&mut self.store, (ranges,))
            .expect("read-memory returns")
            .0
    }

    fn write_memory(&mut self, address: u32, bytes: &[u8]) -> Result<(), MemoryError> {
        let write = self.func::<(u32, &[u8]), (Result<(), MemoryError>,)>("write-memory");
        write
            .call(&mut self.store, (address, bytes))
            .expect("write-memory returns")
            .0
    }
}

/// The range a variable takes, as `debug-symbols` gives it.
fn range_of(info: &debug_format::DebugInfo, path: &str) -> Range {
    let symbol = info
        .symbol(path)
        .unwrap_or_else(|| panic!("the symbols have {path}"));
    Range {
        address: symbol.address,
        size: symbol.size,
    }
}

/// A program that counts its scans, says when it passed a limit, and keeps
/// a note of 64 bytes. Its sixteen bytes of inputs give the scratch area a
/// size.
const COUNTER: &str = r#"
    PROGRAM P
    VAR_EXTERNAL over : DINT; END_VAR
    VAR count : DINT; limit : DINT := 3; note : STRING[60]; END_VAR
        count := count + 1;
        note := 'counting';
        IF count > limit THEN over := 1; ELSE over := 0; END_IF;
    END_PROGRAM

    CONFIGURATION Cfg
    VAR_GLOBAL
        a AT %ID0 : DINT;
        b AT %ID4 : DINT;
        c AT %ID8 : DINT;
        d AT %ID12 : DINT;
        over AT %QD0 : DINT;
    END_VAR
        RESOURCE Res ON CPU
            TASK T(INTERVAL := T#10ms, PRIORITY := 1);
            PROGRAM Main WITH T : P;
        END_RESOURCE
    END_CONFIGURATION
"#;

fn wrap(wasm: &[u8]) -> rk::component::Component {
    rk::component::wrap(wasm, "plant", &Default::default()).expect("the module wraps")
}

/// `source` built and wrapped, its own imports typed from its MIR.
fn build(db: &mut db::RootDatabase, source: &str) -> Result<rk::component::Component, String> {
    let (mir, wasm) = compile_to_mir_and_wasm_as_built(db, source);
    let interfaces = rk::component::interfaces(db, &mir)?;
    rk::component::wrap(&wasm, "plant", &interfaces)
}

/// A scan through the component: the inputs in, the outputs out.
#[rstest]
fn a_component_runs_its_tasks(mut with_db: db::RootDatabase) {
    let component = wrap(&compile_to_wasm_as_built(&mut with_db, DIVIDER));
    let mut plc = Plc::load(&component.bytes);
    assert_eq!(plc.init(), Ok(()));
    let outputs = plc.run_task(0, &4i32.to_le_bytes()).expect("the scan runs");
    assert_eq!(outputs, 250i32.to_le_bytes());
}

/// A host that has only the component finds each address in the lists
/// `run-task` takes and returns by the located map, which the component
/// carries: a value written at an input's offset is the one the program
/// reads, the one at an output's offset is the one it wrote, and the map
/// says how long the inputs are. The addresses' numbers are not offsets.
#[rstest]
fn a_host_finds_each_address_in_the_lists_by_the_located_map(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR_EXTERNAL a : DINT; b : DINT; total : DINT; big : BOOL; END_VAR
            total := a + b;
            big := total > 10;
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_GLOBAL
            a     AT %ID8    : DINT;
            b     AT %ID20   : DINT;
            total AT %QD4    : DINT;
            big   AT %QX12.0 : BOOL;
        END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let component = wrap(&compile_to_wasm_as_built(&mut with_db, source));
    let map = debug_format::LocatedMap::from_msgpack(expect_section(
        &component.bytes,
        debug_format::LOCATED_MAP_SECTION,
    ))
    .expect("the map decodes");
    let at = |address: &str| {
        let entry = map
            .entries
            .iter()
            .find(|e| e.address == address)
            .unwrap_or_else(|| panic!("the map has {address}"));
        entry.offset.expect("an offset") as usize..(entry.offset.unwrap() + entry.size) as usize
    };
    let size = |area| map.area_size(area).expect("the area's size") as usize;

    let mut inputs = vec![0; size(debug_format::LocatedArea::Input)];
    inputs[at("%ID8")].copy_from_slice(&7i32.to_le_bytes());
    inputs[at("%ID20")].copy_from_slice(&5i32.to_le_bytes());
    let mut plc = Plc::load(&component.bytes);
    plc.init().unwrap();
    let outputs = plc.run_task(0, &inputs).expect("the scan runs");
    assert_eq!(outputs.len(), size(debug_format::LocatedArea::Output));
    assert_eq!(outputs[at("%QD4")], 12i32.to_le_bytes());
    assert_eq!(outputs[at("%QX12.0")][0], 1, "TRUE");
}

/// A host finds each retained value in the list `save-retain` returns by
/// the retain map's offsets, and gives the list back whole to a new
/// instance.
#[rstest]
fn a_host_finds_each_retained_value_by_the_retain_map(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR RETAIN hours : DINT; starts : DINT; END_VAR
            starts := starts + 1;
            hours := 40;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let component = wrap(&compile_to_wasm_as_built(&mut with_db, source));
    let map = debug_format::RetainMap::from_msgpack(expect_section(
        &component.bytes,
        debug_format::RETAIN_MAP_SECTION,
    ))
    .expect("the map decodes");
    let value = |saved: &[u8], path: &str| {
        let range = map
            .ranges
            .iter()
            .find(|r| r.path == path)
            .unwrap_or_else(|| panic!("the map has {path}"));
        let at = range.offset.expect("an offset") as usize;
        i32::from_le_bytes(saved[at..at + 4].try_into().unwrap())
    };

    let mut plc = Plc::load(&component.bytes);
    plc.init().unwrap();
    plc.run_task(0, &[]).unwrap();
    plc.run_task(0, &[]).unwrap();
    let saved = plc.save_retain();
    assert_eq!(Some(saved.len() as u32), map.size);
    assert_eq!(value(&saved, "P1.starts"), 2);
    assert_eq!(value(&saved, "P1.hours"), 40);

    // A power cycle: a new instance, the list given back, one more scan.
    let mut warm = Plc::load(&component.bytes);
    warm.init().unwrap();
    warm.restore_retain(&saved);
    warm.run_task(0, &[]).unwrap();
    assert_eq!(value(&warm.save_retain(), "P1.starts"), 3);
}

/// A host reads any variable between two scans, at the address and size
/// `debug-symbols` gives it, and forces one: the next scan finds the value
/// it wrote. Several ranges come back one after the other.
#[rstest]
fn a_host_reads_and_forces_a_variable_between_scans(mut with_db: db::RootDatabase) {
    let component = wrap(&compile_to_wasm_as_built(&mut with_db, COUNTER));
    let info = debug_format::DebugInfo::from_wasm(&component.bytes);
    let (count, limit) = (range_of(&info, "Main.count"), range_of(&info, "Main.limit"));
    let mut plc = Plc::load(&component.bytes);
    plc.init().unwrap();
    let inputs = [0; 16];
    plc.run_task(0, &inputs).unwrap();
    assert_eq!(plc.run_task(0, &inputs).unwrap(), 0i32.to_le_bytes());

    // One range fits beside itself in the scratch area; two do not, and come
    // back from past it.
    assert_eq!(plc.read_memory(&[count]), Ok(2i32.to_le_bytes().to_vec()));
    assert_eq!(
        plc.read_memory(&[count, limit]),
        Ok([2i32.to_le_bytes(), 3i32.to_le_bytes()].concat())
    );
    assert_eq!(plc.read_memory(&[]), Ok(Vec::new()));

    plc.write_memory(limit.address, &1i32.to_le_bytes())
        .unwrap();
    assert_eq!(
        plc.run_task(0, &inputs).unwrap(),
        1i32.to_le_bytes(),
        "the third scan passed the limit the host wrote"
    );
    assert_eq!(plc.read_memory(&[limit]), Ok(1i32.to_le_bytes().to_vec()));
}

/// A read past the end of the memory and a write outside the variables are
/// errors, which leave the component running. The variables end where the
/// string literals start: their last byte can be written, the next cannot,
/// and neither can the builtins' memory before them.
#[rstest]
fn what_a_host_may_not_read_or_write_is_an_error(mut with_db: db::RootDatabase) {
    let (mir, wasm) = compile_to_mir_and_wasm_as_built(&mut with_db, COUNTER);
    assert!(!mir.string_data.is_empty(), "the program has a literal");
    let component = wrap(&wasm);
    let info = debug_format::DebugInfo::from_wasm(&component.bytes);
    let count = range_of(&info, "Main.count");
    let mut plc = Plc::load(&component.bytes);
    plc.init().unwrap();

    let past_the_end = Range {
        address: u32::MAX - 3,
        size: 8,
    };
    assert_eq!(
        plc.read_memory(&[count, past_the_end]),
        Err(MemoryError::OutsideMemory(1)),
        "the second range, by its place in the list"
    );

    let refused = Err(MemoryError::OutsideVariables);
    let end = mir.memory_layout.total_size();
    assert_eq!(plc.write_memory(end - 1, &[0]), Ok(()), "the last byte");
    assert_eq!(plc.write_memory(end - 1, &[0, 0]), refused, "one past it");
    assert_eq!(plc.write_memory(end, &[0]), refused, "a string literal");
    assert_eq!(
        plc.write_memory(mir::memory::BUILTIN_RESERVED_FLOOR - 1, &[0]),
        refused,
        "the builtins' memory"
    );
    assert_eq!(plc.write_memory(u32::MAX, &[0]), refused);

    assert!(
        plc.run_task(0, &[0; 16]).is_ok(),
        "the component still runs"
    );
}

/// A read too large for the scratch area lands past everything the module
/// laid out, after its ranges when they landed there themselves, in memory
/// grown to hold it. Each range of 4 and 64 bytes takes 8 of the list, so a
/// list written over its own ranges would read garbage. The component goes
/// on, and reads again.
#[rstest]
fn a_read_larger_than_the_scratch_area_lands_past_it(mut with_db: db::RootDatabase) {
    let component = wrap(&compile_to_wasm_as_built(&mut with_db, COUNTER));
    let info = debug_format::DebugInfo::from_wasm(&component.bytes);
    let (count, note) = (range_of(&info, "Main.count"), range_of(&info, "Main.note"));
    assert_eq!(note.size, 64, "a length of 4 bytes and 60 characters");
    let mut plc = Plc::load(&component.bytes);
    plc.init().unwrap();
    plc.run_task(0, &[0; 16]).unwrap();
    let alone = plc.read_memory(&[note]).expect("the note alone");
    assert_eq!(&alone[4..12], b"counting");

    let ranges: Vec<Range> = (0..5_000).flat_map(|_| [count, note]).collect();
    let bytes = plc.read_memory(&ranges).expect("the read");
    assert_eq!(bytes.len(), 5_000 * (4 + 64));
    for pair in bytes.chunks(4 + 64) {
        assert_eq!(&pair[..4], 1i32.to_le_bytes());
        assert_eq!(&pair[4..], alone.as_slice());
    }

    plc.run_task(0, &[0; 16]).unwrap();
    assert_eq!(plc.read_memory(&[count]), Ok(2i32.to_le_bytes().to_vec()));
}

/// A release build reads and writes its memory too: a production host
/// watches its variables. Its addresses are the debug build's.
#[rstest]
fn a_release_build_reads_and_writes_its_memory(mut with_db: db::RootDatabase) {
    let (mir, debug) = compile_to_mir_and_wasm_as_built(&mut with_db, COUNTER);
    let release =
        wasm_codegen::generate_wasm_profile(&with_db, &mir, wasm_codegen::Profile::Release)
            .finish();
    let info = debug_format::DebugInfo::from_wasm(&debug);
    let limit = range_of(&info, "Main.limit");
    let mut plc = Plc::load(&wrap(&release).bytes);
    plc.init().unwrap();
    plc.write_memory(limit.address, &0i32.to_le_bytes())
        .unwrap();
    assert_eq!(plc.run_task(0, &[0; 16]).unwrap(), 1i32.to_le_bytes());
    assert_eq!(plc.read_memory(&[limit]), Ok(0i32.to_le_bytes().to_vec()));
}

/// A division by zero in a scan is an error the host reads, and the next
/// scan runs: the exception stopped at the interface, nothing trapped, and
/// the component's store is not locked.
#[rstest]
fn a_raise_leaves_the_component_running(mut with_db: db::RootDatabase) {
    let component = wrap(&compile_to_wasm_as_built(&mut with_db, DIVIDER));
    let mut plc = Plc::load(&component.bytes);
    plc.init().unwrap();
    assert_eq!(
        plc.run_task(0, &0i32.to_le_bytes()),
        Err(ScanError::Raised("division by zero".to_string()))
    );
    assert_eq!(
        plc.run_task(0, &8i32.to_le_bytes()),
        Ok(125i32.to_le_bytes().to_vec())
    );
    assert_eq!(plc.run_task(1, &[]), Err(ScanError::NoSuchTask));
}

/// A module that cannot raise has no exception to catch, and runs the same.
#[rstest]
fn a_module_that_cannot_raise_wraps_too(mut with_db: db::RootDatabase) {
    let source = DIVIDER.replace("1000 / input", "input + 1");
    let component = wrap(&compile_to_wasm_as_built(&mut with_db, &source));
    let mut plc = Plc::load(&component.bytes);
    plc.init().unwrap();
    assert_eq!(
        plc.run_task(0, &41i32.to_le_bytes()),
        Ok(42i32.to_le_bytes().to_vec())
    );
}

/// A test runs through the component in a debug build.
#[rstest]
fn a_component_runs_its_tests(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION test_fails
            __RAISE('pump stuck');
        END_FUNCTION
    "#;
    let component = wrap(&compile_to_wasm_as_built(&mut with_db, source));
    let mut plc = Plc::load(&component.bytes);
    assert_eq!(
        plc.run_test(0),
        Err(TestError::Failed("pump stuck".to_string()))
    );
    assert_eq!(plc.run_test(1), Err(TestError::NoSuchTest));
}

/// The component is named after the project, as a file name: a character
/// a file name cannot hold becomes `_`, and a name with nothing left is
/// `plc`.
#[test]
fn the_component_is_named_after_the_project() {
    let ws = std::path::Path::new("ws");
    let debug = |project| rk::compiler::component_path(ws, wasm_codegen::Profile::Debug, project);
    let built = ws.join("rk_build");
    assert_eq!(debug("Plant_A"), built.join("debug").join("Plant_A.wasm"));
    assert_eq!(
        debug("line 2/east"),
        built.join("debug").join("line_2_east.wasm")
    );
    assert_eq!(debug(".."), built.join("debug").join("plc.wasm"));
    assert_eq!(
        rk::compiler::component_path(ws, wasm_codegen::Profile::Release, "Plant_A"),
        built.join("release").join("Plant_A.wasm")
    );
    // `rk test` builds a module of its own, which lands beside them.
    assert_eq!(
        rk::compiler::test_component_path(ws, None, "Plant_A"),
        built.join("test").join("Plant_A.wasm")
    );
    assert_eq!(
        rk::compiler::test_component_path(ws, Some("z"), "Plant_A"),
        built.join("test-Oz").join("Plant_A.wasm")
    );
}

/// The world imports each WASI interface the module calls, as WASI
/// declares it, and exports the control interface; the WIT folder holds
/// each package the world names, and the ones those use.
#[rstest]
fn the_world_imports_what_the_module_calls(mut with_db: db::RootDatabase) {
    let source = r#"
        {extern 'wasi:clocks/monotonic-clock@0.3.0' 'now'}
        FUNCTION now : LINT
        END_FUNCTION

        {export}
        FUNCTION Stamp : LINT
            Stamp := now();
        END_FUNCTION
    "#;
    let component = build(&mut with_db, source).expect("the module wraps");
    assert!(
        component
            .world
            .contains("import wasi:clocks/monotonic-clock@0.3.0;"),
        "{}",
        component.world
    );
    assert!(component.world.contains(&format!("export {CONTROL};")));
    let files: Vec<&str> = component
        .wit
        .iter()
        .map(|(path, _)| path.as_str())
        .collect();
    assert_eq!(
        files,
        [
            "deps/rk-plc/control.wit",
            "deps/wasi-clocks/monotonic-clock.wit",
            "deps/wasi-clocks/system-clock.wit",
            "deps/wasi-clocks/timezone.wit",
            "deps/wasi-clocks/types.wit",
            "deps/wasi-clocks/world.wit",
            "deps/wasi-clocks/LICENSE.md",
        ]
    );

    let engine = engine();
    let loaded = Component::new(&engine, &component.bytes).expect("a valid component");
    let ty = loaded.component_type();
    let imports: Vec<&str> = ty.imports(&engine).map(|(name, _)| name).collect();
    assert_eq!(imports, ["wasi:clocks/monotonic-clock@0.3.0"]);
    let exports: Vec<&str> = ty.exports(&engine).map(|(name, _)| name).collect();
    assert_eq!(exports, [CONTROL, EXPORTS]);
}

/// A module whose import names an interface at a version rk carries no WIT
/// for is refused, the interface named.
#[rstest]
fn an_import_at_another_version_is_refused(mut with_db: db::RootDatabase) {
    let source = r#"
        {extern 'wasi:clocks/monotonic-clock@0.2.6' 'now'}
        FUNCTION now : LINT
        END_FUNCTION

        {export}
        FUNCTION Stamp : LINT
            Stamp := now();
        END_FUNCTION
    "#;
    let refused = build(&mut with_db, source)
        .err()
        .expect("the module is refused");
    assert_eq!(
        refused,
        "the module imports `wasi:clocks/monotonic-clock@0.2.6`, \
         which no WIT package rk carries describes"
    );
}

/// A task that calls the host's own interfaces: one named by a plain name,
/// one by an interface's name. Its input picks the text it shows.
const HOST_CALLS: &str = r#"
    {extern 'modbus' 'read-coils'}
    FUNCTION ReadCoils : UDINT
    VAR_INPUT start_address : UINT; count : UINT; END_VAR
    END_FUNCTION

    {extern 'acme:display/text@1.0.0' 'show'}
    FUNCTION Show : BOOL
    VAR_INPUT line : STRING; marker : CHAR; delay : TIME; END_VAR
    END_FUNCTION

    PROGRAM P
    VAR_EXTERNAL input : DINT; output : UDINT; END_VAR
    VAR shown : BOOL; END_VAR
        output := ReadCoils(start_address := 3, count := 8);
        IF input = 1 THEN
            shown := Show(line := 'café', marker := 'x', delay := T#1500ms);
        ELSIF input = 2 THEN
            shown := Show(line := 'caf$E9', marker := 'x', delay := T#0s);
        END_IF;
    END_PROGRAM

    CONFIGURATION Cfg
    VAR_GLOBAL
        input AT %ID0 : DINT;
        output AT %QD0 : UDINT;
    END_VAR
        RESOURCE Res ON CPU
            TASK T(INTERVAL := T#10ms, PRIORITY := 1);
            PROGRAM Main WITH T : P;
        END_RESOURCE
    END_CONFIGURATION
"#;

/// The world imports the host's own interfaces as their `{extern}`
/// FUNCTIONs type them: one named by a plain name is declared in the world,
/// one named by an interface's name in a package rk writes under `deps/`.
#[rstest]
fn the_host_s_own_interfaces_are_typed_from_their_declarations(mut with_db: db::RootDatabase) {
    let component = build(&mut with_db, HOST_CALLS).expect("the module wraps");
    assert_eq!(
        component.world,
        "// The world of `plant`, written by rk at each build.
package local:plant;

world plc {
    import acme:display/text@1.0.0;
    import modbus: interface {
        read-coils: func(start-address: u16, count: u16) -> u32;
    }
    export rk:plc/control@0.1.0;
}
"
    );
    let files: Vec<(&str, &str)> = component
        .wit
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_ref()))
        .collect();
    assert_eq!(files[0].0, "deps/rk-plc/control.wit");
    assert_eq!(
        files[1..],
        [(
            "deps/acme-display/text.wit",
            "// What the module imports from `acme:display/text@1.0.0`, written by rk at each build.
package acme:display@1.0.0;

interface text {
    show: func(line: string, marker: char, delay: s32) -> bool;
}
"
        )]
    );
    let engine = engine();
    let loaded = Component::new(&engine, &component.bytes).expect("a valid component");
    let ty = loaded.component_type();
    let imports: Vec<&str> = ty.imports(&engine).map(|(name, _)| name).collect();
    assert_eq!(imports, ["acme:display/text@1.0.0", "modbus"]);
}

/// A host built on those types runs the component: what a call passes
/// arrives as WIT values, and what the host returns comes back. A STRING
/// that is not UTF-8 is an error of the scan, raised before the call, and
/// the next scan runs: nothing trapped, the store is not locked.
#[rstest]
fn a_host_runs_the_component_on_its_own_interfaces(mut with_db: db::RootDatabase) {
    let component = build(&mut with_db, HOST_CALLS).expect("the module wraps");
    type Shown = std::sync::Arc<std::sync::Mutex<Vec<(String, char, i32)>>>;
    let shown: Shown = Default::default();
    let host = shown.clone();
    let mut plc = Plc::load_with(&component.bytes, move |linker| {
        let mut modbus = linker.instance("modbus").unwrap();
        modbus
            .func_wrap("read-coils", |_, (start, count): (u16, u16)| {
                Ok((u32::from(start) * 100 + u32::from(count),))
            })
            .unwrap();
        let mut display = linker.instance("acme:display/text@1.0.0").unwrap();
        display
            .func_wrap(
                "show",
                move |_, (line, marker, delay): (String, char, i32)| {
                    host.lock().unwrap().push((line, marker, delay));
                    Ok((true,))
                },
            )
            .unwrap();
    });
    plc.init().unwrap();
    assert_eq!(
        plc.run_task(0, &1i32.to_le_bytes()),
        Ok(308u32.to_le_bytes().to_vec())
    );
    assert_eq!(
        plc.run_task(0, &2i32.to_le_bytes()),
        Err(ScanError::Raised(
            "a STRING passed to the host is not UTF-8".to_string()
        ))
    );
    assert_eq!(
        plc.run_task(0, &1i32.to_le_bytes()),
        Ok(308u32.to_le_bytes().to_vec())
    );
    assert_eq!(
        *shown.lock().unwrap(),
        [
            ("café".to_string(), 'x', 1500),
            ("café".to_string(), 'x', 1500)
        ]
    );
}

/// Two FUNCTIONs may import one function of the host when they type it
/// alike; typed two ways, the host could define it only one, and the build
/// is refused with both named.
#[rstest]
fn one_function_imported_with_two_signatures_is_refused(mut with_db: db::RootDatabase) {
    let source = r#"
        {extern 'modbus' 'read'}
        FUNCTION ReadInt : INT
        VAR_INPUT address : UINT; END_VAR
        END_FUNCTION

        {extern 'modbus' 'read'}
        FUNCTION ReadWord : WORD
        VAR_INPUT address : UINT; END_VAR
        END_FUNCTION

        {extern 'modbus' 'write'}
        FUNCTION WriteOne : BOOL
        VAR_INPUT address : UINT; END_VAR
        END_FUNCTION

        {extern 'modbus' 'write'}
        FUNCTION WriteAgain : BOOL
        VAR_INPUT register : UINT; END_VAR
        END_FUNCTION
    "#;
    assert_eq!(
        build(&mut with_db, source).err(),
        Some(
            "`ReadInt` and `ReadWord` import `read` from `modbus` with two signatures, \
             and the host has one function under that name"
                .to_string()
        )
    );
}

/// A name spelled like a WIT keyword is written after a `%` in the WIT rk
/// writes, wherever it stands, and the host finds it under its own name.
#[rstest]
fn a_name_spelled_like_a_wit_keyword_is_escaped(mut with_db: db::RootDatabase) {
    let source = r#"
        {extern 'acme:list/stream@1.0.0' 'type'}
        FUNCTION Kind : INT
        VAR_INPUT list : INT; END_VAR
        END_FUNCTION

        {extern 'record' 'use'}
        FUNCTION UseIt : INT
        END_FUNCTION
    "#;
    let component = build(&mut with_db, source).expect("the module wraps");
    assert!(
        component.world.contains("import acme:%list/%stream@1.0.0;")
            && component.world.contains("import %record: interface {")
            && component.world.contains("%use: func() -> s16;"),
        "{}",
        component.world
    );
    let engine = engine();
    let loaded = Component::new(&engine, &component.bytes).expect("a valid component");
    let ty = loaded.component_type();
    let imports: Vec<&str> = ty.imports(&engine).map(|(name, _)| name).collect();
    assert_eq!(imports, ["acme:list/stream@1.0.0", "record"]);
}

/// FUNCTIONs a host calls.
const EXPORTED: &str = r#"
    TYPE Mode : (Off, Slow := 5, Fast); END_TYPE
    TYPE Percent : INT (0..100); END_TYPE

    {export}
    FUNCTION Scale : REAL
    VAR_INPUT raw : INT; gain : REAL; END_VAR
        Scale := raw * gain;
    END_FUNCTION

    {export}
    FUNCTION Divide : LINT
    VAR_INPUT a : LINT; b : LINT; END_VAR
        Divide := a / b;
    END_FUNCTION

    {export}
    FUNCTION Greet : STRING
    VAR_INPUT who : STRING[10]; times : USINT; END_VAR
        IF times = 0 THEN
            Greet := 'nobody';
        ELSE
            Greet := who;
        END_IF;
    END_FUNCTION

    {export}
    FUNCTION Pick : STRING
    VAR_INPUT left : STRING[4]; right : STRING[4]; END_VAR
        IF left = right THEN
            Pick := 'same';
        ELSE
            Pick := right;
        END_IF;
    END_FUNCTION

    {export}
    FUNCTION Latin : STRING
        Latin := 'caf$E9';
    END_FUNCTION

    {export}
    FUNCTION Half : Percent
    VAR_INPUT level : Percent; END_VAR
        Half := level / 2;
    END_FUNCTION

    {export 'next-mode'}
    FUNCTION Next : Mode
    VAR_INPUT mode : Mode; END_VAR
        CASE mode OF
            Mode#Off: Next := Mode#Slow;
            Mode#Slow: Next := Mode#Fast;
        ELSE
            Next := Mode#Off;
        END_CASE;
    END_FUNCTION

    {export}
    FUNCTION Is_Even : BOOL
    VAR_INPUT n : UDINT; END_VAR
        Is_Even := n MOD 2 = 0;
    END_FUNCTION

    {export}
    FUNCTION Upper : CHAR
    VAR_INPUT c : CHAR; END_VAR
        Upper := c;
        IF c = 'a' THEN
            Upper := 'A';
        END_IF;
    END_FUNCTION

    {export}
    FUNCTION Wide : LREAL
    VAR_INPUT x : LREAL; small : SINT; count : UINT; END_VAR
        Wide := x + small + count;
    END_FUNCTION

    {export}
    FUNCTION Mask : WORD
    VAR_INPUT bits : WORD; END_VAR
        Mask := bits AND 16#00FF;
    END_FUNCTION

    {export}
    FUNCTION Bump : INT
    VAR_INPUT start_value : INT; END_VAR
    VAR p : REF_TO INT; END_VAR
        p := REF(start_value);
        p^ := p^ + 1;
        Bump := start_value;
    END_FUNCTION

    NAMESPACE Motion.Axis
        {export}
        FUNCTION Stop
        END_FUNCTION

        {export}
        FUNCTION Halt
        VAR_INPUT why : STRING; END_VAR
            __RAISE(why);
        END_FUNCTION
    END_NAMESPACE
"#;

/// The world exports the `{export}` FUNCTIONs in one interface named after
/// the project, each under its WIT name, typed from its declaration, and
/// each a `result` of what it returns or of the message that stopped it.
#[rstest]
fn the_world_exports_the_functions_a_host_calls(mut with_db: db::RootDatabase) {
    let component = build(&mut with_db, EXPORTED).expect("the module wraps");
    assert_eq!(
        component.world,
        "// The world of `plant`, written by rk at each build.
package local:plant;

world plc {
    export rk:plc/control@0.1.0;
    export plc: interface {
        scale: func(raw: s16, gain: f32) -> result<f32, string>;
        divide: func(a: s64, b: s64) -> result<s64, string>;
        greet: func(who: string, times: u8) -> result<string, string>;
        pick: func(left: string, right: string) -> result<string, string>;
        latin: func() -> result<string, string>;
        half: func(level: s16) -> result<s16, string>;
        next-mode: func(mode: s32) -> result<s32, string>;
        is-even: func(n: u32) -> result<bool, string>;
        upper: func(c: char) -> result<char, string>;
        wide: func(x: f64, small: s8, count: u16) -> result<f64, string>;
        mask: func(bits: u16) -> result<u16, string>;
        bump: func(start-value: s16) -> result<s16, string>;
        motion-axis-stop: func() -> result<_, string>;
        motion-axis-halt: func(why: string) -> result<_, string>;
    }
}
"
    );
    let engine = engine();
    let loaded = Component::new(&engine, &component.bytes).expect("a valid component");
    let ty = loaded.component_type();
    let exports: Vec<&str> = ty.exports(&engine).map(|(name, _)| name).collect();
    assert_eq!(exports, [CONTROL, EXPORTS]);
}

/// A host calls each of them through the types the world gives: what it
/// passes arrives, and what the FUNCTION returns comes back, whatever its
/// width.
#[rstest]
fn a_host_calls_the_exported_functions(mut with_db: db::RootDatabase) {
    let component = build(&mut with_db, EXPORTED).expect("the module wraps");
    let mut plc = Plc::load(&component.bytes);
    plc.init().unwrap();
    assert_eq!(plc.call("scale", (-12i16, 0.5f32)), Ok(-6.0f32));
    assert_eq!(plc.call("divide", (1i64 << 40, 2i64)), Ok(1i64 << 39));
    assert_eq!(plc.call("greet", ("café", 2u8)), Ok("café".to_string()));
    assert_eq!(plc.call("greet", ("café", 0u8)), Ok("nobody".to_string()));
    assert_eq!(plc.call("pick", ("ab", "cdef")), Ok("cdef".to_string()));
    assert_eq!(plc.call("pick", ("ab", "ab")), Ok("same".to_string()));
    assert_eq!(plc.call("half", (90i16,)), Ok(45i16));
    assert_eq!(plc.call("next-mode", (5i32,)), Ok(6i32));
    assert_eq!(plc.call("next-mode", (77i32,)), Ok(0i32));
    assert_eq!(plc.call("is-even", (10u32,)), Ok(true));
    assert_eq!(plc.call("is-even", (u32::MAX,)), Ok(false));
    assert_eq!(plc.call("upper", ('a',)), Ok('A'));
    assert_eq!(plc.call("upper", ('é',)), Ok('é'));
    assert_eq!(plc.call("wide", (0.5f64, -3i8, 65535u16)), Ok(65532.5f64));
    assert_eq!(plc.call("mask", (0xABCDu16,)), Ok(0x00CDu16));
    assert_eq!(plc.call("bump", (-1i16,)), Ok(0i16));
    assert_eq!(plc.call::<_, ()>("motion-axis-stop", ()), Ok(()));
}

/// What stops a FUNCTION is the error of its result, and the next call
/// runs: an exception, a STRING longer than its parameter, however long, a
/// value outside its subrange, and a STRING returned that is not UTF-8.
/// Nothing traps, so the component's store is never locked.
#[rstest]
fn what_stops_an_exported_function_is_an_error(mut with_db: db::RootDatabase) {
    let component = build(&mut with_db, EXPORTED).expect("the module wraps");
    let mut plc = Plc::load(&component.bytes);
    plc.init().unwrap();
    let too_long = Err("a STRING passed by the host is longer than its parameter".to_string());

    assert_eq!(
        plc.call::<_, i64>("divide", (1i64, 0i64)),
        Err("division by zero".to_string())
    );
    assert_eq!(plc.call("divide", (9i64, 3i64)), Ok(3i64));
    assert_eq!(
        plc.call::<_, ()>("motion-axis-halt", ("pump stuck",)),
        Err("pump stuck".to_string())
    );

    // 10 bytes fit `who`, 11 do not: `é` is two.
    assert_eq!(
        plc.call("greet", ("abcdefghé", 1u8)),
        Ok("abcdefghé".to_string())
    );
    assert_eq!(
        plc.call::<_, String>("greet", ("abcdefghié", 1u8)),
        too_long
    );
    let megabyte = "x".repeat(1 << 20);
    assert_eq!(
        plc.call::<_, String>("greet", (megabyte.as_str(), 1u8)),
        too_long
    );
    assert_eq!(
        plc.call::<_, String>("pick", ("ab", megabyte.as_str())),
        too_long
    );
    assert_eq!(
        plc.call::<_, String>("pick", (megabyte.as_str(), megabyte.as_str())),
        too_long
    );
    assert_eq!(plc.call("pick", ("ab", "cd")), Ok("cd".to_string()));

    assert_eq!(
        plc.call::<_, i16>("half", (101i16,)),
        Err("value out of subrange bounds".to_string())
    );
    assert_eq!(
        plc.call::<_, String>("latin", ()),
        Err("a STRING passed to the host is not UTF-8".to_string())
    );
    assert_eq!(plc.call("half", (100i16,)), Ok(50i16));
}

/// Only the host calls an exported function, so the stack of recursive
/// calls starts empty at each call: the frames an exception left behind do
/// not add up until the stack is full.
#[rstest]
fn an_exported_function_starts_the_stack_over(mut with_db: db::RootDatabase) {
    let source = r#"
        {export}
        FUNCTION Dive : DINT
        VAR_INPUT depth : DINT; END_VAR
        VAR trail : ARRAY[0..15] OF DINT; END_VAR
            trail[0] := depth;
            IF depth = 0 THEN
                __RAISE('bottom');
            END_IF;
            Dive := Dive(depth - 1) + trail[0];
        END_FUNCTION
    "#;
    let component = build(&mut with_db, source).expect("the module wraps");
    let mut plc = Plc::load(&component.bytes);
    plc.init().unwrap();
    for call in 0..2_000 {
        assert_eq!(
            plc.call::<_, i32>("dive", (40i32,)),
            Err("bottom".to_string()),
            "call {call}"
        );
    }
}

/// A project named like a module it imports from has one name for two
/// interfaces of its world: the build says so.
#[rstest]
fn an_export_interface_named_like_an_import_is_refused(mut with_db: db::RootDatabase) {
    let source = r#"
        {extern 'plc' 'read'}
        FUNCTION Read : INT
        END_FUNCTION

        {export}
        FUNCTION Twice : INT
            Twice := Read() * 2;
        END_FUNCTION
    "#;
    assert_eq!(
        build(&mut with_db, source).err(),
        Some(
            "the module imports from `plc`, and exports its FUNCTIONs in the interface `plc`, \
             named after the project"
                .to_string()
        )
    );
}

/// What a host returns for `read-register`: its VAR_OUTPUTs, and its
/// return value under the import's name.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(record)]
struct ReadRegisterOutputs {
    quality: i8,
    value: i64,
    #[component(name = "read-register")]
    read_register: bool,
}

/// What a host returns for `position`: one VAR_OUTPUT, no return value.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(record)]
struct PositionOutputs {
    counts: i32,
}

/// What `measure` returns: its VAR_OUTPUTs, a STRING among them, and its
/// return value under its own name.
#[derive(
    Clone,
    Debug,
    PartialEq,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(record)]
struct MeasureOutputs {
    unit: String,
    clamped: bool,
    peak: i64,
    mark: char,
    measure: f32,
}

/// What `twice` returns: one VAR_OUTPUT, no return value.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(record)]
struct TwiceOutputs {
    doubled: u16,
}

/// What `echo` returns.
#[derive(
    Clone,
    Debug,
    PartialEq,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(record)]
struct EchoOutputs {
    copy: String,
}

/// What `label` returns.
#[derive(
    Clone,
    Debug,
    PartialEq,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(record)]
struct LabelOutputs {
    text: String,
}

/// A task that reads the VAR_OUTPUTs of two imports, and FUNCTIONs a host
/// reads theirs from.
const OUTPUTS: &str = r#"
    {extern 'modbus' 'read-register'}
    FUNCTION ReadRegister : BOOL
    VAR_INPUT address : UINT; END_VAR
    VAR_OUTPUT quality : SINT; value : LINT; END_VAR
    END_FUNCTION

    {extern 'acme:drive/axis@1.0.0' 'position'}
    FUNCTION Position
    VAR_OUTPUT counts : DINT; END_VAR
    END_FUNCTION

    {export}
    FUNCTION Measure : REAL
    VAR_INPUT raw : INT; END_VAR
    VAR_OUTPUT unit : STRING[8]; clamped : BOOL; peak : LINT; mark : CHAR; END_VAR
        IF raw < 0 THEN
            __RAISE('negative');
        END_IF;
        unit := 'm³/h';
        clamped := raw > 100;
        peak := raw;
        peak := peak * 1000000000;
        mark := 'K';
        IF clamped THEN
            Measure := 100.0;
        ELSE
            Measure := raw;
        END_IF;
    END_FUNCTION

    {export}
    FUNCTION Twice
    VAR_INPUT n : USINT; END_VAR
    VAR_OUTPUT doubled : UINT; END_VAR
        doubled := n;
        doubled := doubled * 2;
    END_FUNCTION

    {export}
    FUNCTION Label
    VAR_OUTPUT text : STRING; END_VAR
        text := 'caf$E9';
    END_FUNCTION

    {export}
    FUNCTION Echo
    VAR_INPUT text : STRING[4]; END_VAR
    VAR_OUTPUT copy : STRING[4]; END_VAR
        copy := text;
    END_FUNCTION

    PROGRAM P
    VAR_EXTERNAL output : LINT; END_VAR
    VAR ok : BOOL; v : LINT; q : SINT; c : DINT; END_VAR
        ok := ReadRegister(address := 40, value => v, quality => q);
        Position(counts => c);
        output := v + q + c;
        IF ok THEN
            output := output + 1000;
        END_IF;
    END_PROGRAM

    CONFIGURATION Cfg
    VAR_GLOBAL
        output AT %QL0 : LINT;
    END_VAR
        RESOURCE Res ON CPU
            TASK T(INTERVAL := T#10ms, PRIORITY := 1);
            PROGRAM Main WITH T : P;
        END_RESOURCE
    END_CONFIGURATION
"#;

/// A function with VAR_OUTPUTs returns the record of them, its return value
/// last under its own name, typed in the interface the function is in: an
/// import's in the host's, an export's in the project's.
#[rstest]
fn a_function_with_outputs_returns_a_record(mut with_db: db::RootDatabase) {
    let component = build(&mut with_db, OUTPUTS).expect("the module wraps");
    assert_eq!(
        component.world,
        "// The world of `plant`, written by rk at each build.
package local:plant;

world plc {
    import acme:drive/axis@1.0.0;
    import modbus: interface {
        record read-register-outputs {
            quality: s8,
            value: s64,
            read-register: bool,
        }
        read-register: func(address: u16) -> read-register-outputs;
    }
    export rk:plc/control@0.1.0;
    export plc: interface {
        record measure-outputs {
            unit: string,
            clamped: bool,
            peak: s64,
            mark: char,
            measure: f32,
        }
        measure: func(raw: s16) -> result<measure-outputs, string>;
        record twice-outputs {
            doubled: u16,
        }
        twice: func(n: u8) -> result<twice-outputs, string>;
        record label-outputs {
            text: string,
        }
        label: func() -> result<label-outputs, string>;
        record echo-outputs {
            copy: string,
        }
        echo: func(text: string) -> result<echo-outputs, string>;
    }
}
"
    );
    let files: Vec<(&str, &str)> = component
        .wit
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_ref()))
        .collect();
    assert_eq!(
        files[1..],
        [(
            "deps/acme-drive/axis.wit",
            "// What the module imports from `acme:drive/axis@1.0.0`, written by rk at each build.
package acme:drive@1.0.0;

interface axis {
    record position-outputs {
        counts: s32,
    }
    position: func() -> position-outputs;
}
"
        )]
    );
}

/// A host built on those types runs the component: the record it returns
/// for an import lands in the VAR_OUTPUTs, whatever each field's width and
/// place, `value` 8 bytes in past the padding after `quality`, and the
/// record an export returns holds the VAR_OUTPUTs it wrote,
/// a STRING checked for UTF-8 as a returned one is. What stops the
/// FUNCTION is the error of its result, and a record as large as
/// `measure`'s leaves the message of a STRING too long whole.
#[rstest]
fn a_host_exchanges_records_with_the_component(mut with_db: db::RootDatabase) {
    let component = build(&mut with_db, OUTPUTS).expect("the module wraps");
    let mut plc = Plc::load_with(&component.bytes, |linker| {
        let mut modbus = linker.instance("modbus").unwrap();
        modbus
            .func_wrap("read-register", |_, (address,): (u16,)| {
                Ok((ReadRegisterOutputs {
                    quality: -3,
                    value: i64::from(address) << 33,
                    read_register: true,
                },))
            })
            .unwrap();
        let mut axis = linker.instance("acme:drive/axis@1.0.0").unwrap();
        axis.func_wrap("position", |_, ()| Ok((PositionOutputs { counts: -7 },)))
            .unwrap();
    });
    plc.init().unwrap();
    let scanned = (40i64 << 33) - 3 - 7 + 1000;
    assert_eq!(plc.run_task(0, &[]), Ok(scanned.to_le_bytes().to_vec()));

    let measured = |clamped: bool, raw: i64, measure: f32| MeasureOutputs {
        unit: "m³/h".to_string(),
        clamped,
        peak: raw * 1_000_000_000,
        mark: 'K',
        measure,
    };
    assert_eq!(
        plc.call("measure", (150i16,)),
        Ok(measured(true, 150, 100.0))
    );
    assert_eq!(plc.call("measure", (42i16,)), Ok(measured(false, 42, 42.0)));
    assert_eq!(
        plc.call::<_, MeasureOutputs>("measure", (-1i16,)),
        Err("negative".to_string())
    );
    assert_eq!(
        plc.call("twice", (200u8,)),
        Ok(TwiceOutputs { doubled: 400 })
    );
    assert_eq!(
        plc.call::<_, LabelOutputs>("label", ()),
        Err("a STRING passed to the host is not UTF-8".to_string())
    );
    assert_eq!(plc.call("measure", (7i16,)), Ok(measured(false, 7, 7.0)));
    assert_eq!(
        plc.call("echo", ("ab³",)),
        Ok(EchoOutputs {
            copy: "ab³".to_string()
        })
    );
    assert_eq!(
        plc.call::<_, EchoOutputs>("echo", ("abcde",)),
        Err("a STRING passed by the host is longer than its parameter".to_string())
    );
}

/// A record and a function of one interface are two of its names, which
/// the component model compares without their `-`: a function whose record
/// is named as another function of the interface is refused at the build,
/// an export as an import.
#[rstest]
fn a_record_named_like_a_function_is_refused(mut with_db: db::RootDatabase) {
    let exported = r#"
        {export}
        FUNCTION Read : INT
        VAR_OUTPUT status : INT; END_VAR
            Read := 1;
        END_FUNCTION

        {export}
        FUNCTION ReadOutputs : INT
            ReadOutputs := 2;
        END_FUNCTION
    "#;
    assert_eq!(
        build(&mut with_db, exported).err(),
        Some(
            "`read` returns its VAR_OUTPUTs as the record `read-outputs`, which cannot be \
             told from the function `readoutputs` of `plc`"
                .to_string()
        )
    );

    let imported = r#"
        {extern 'modbus' 'read'}
        FUNCTION Read : INT
        VAR_OUTPUT status : INT; END_VAR
        END_FUNCTION

        {extern 'modbus' 'read-outputs'}
        FUNCTION ReadOutputs : INT
        END_FUNCTION

        {export}
        FUNCTION Both : INT
        VAR status : INT; END_VAR
            Both := Read(status => status) + ReadOutputs();
        END_FUNCTION
    "#;
    let mut db = db::RootDatabase::default();
    assert_eq!(
        build(&mut db, imported).err(),
        Some(
            "`read` returns its VAR_OUTPUTs as the record `read-outputs`, which cannot be \
             told from the function `read-outputs` of `modbus`"
                .to_string()
        )
    );
}

/// WASI's `instant`, as the system clock gives it and the zone takes it.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(record)]
struct Instant {
    seconds: i64,
    nanoseconds: u32,
}

/// The instant the system clock gives below: 2026-01-01 00:00:00.25 UTC.
const WALL: Instant = Instant {
    seconds: 1_767_225_600,
    nanoseconds: 250_000_000,
};

/// `Std.Timers` reading WASI's clocks through a component host: the
/// canonical ABI lifts and lowers them as WASI's WIT types them, the
/// records the system clock and the zone write where the module points
/// included.
const CLOCKS: &str = r#"
    USING Std.Timers;

    {test}
    FUNCTION test_monotonic
        IF PLC_LTIME() <> LT#42ns THEN
            __RAISE('the monotonic clock');
        END_IF;
    END_FUNCTION

    {test}
    FUNCTION test_wall
        IF WALL_LDT() <> LDT#2026-01-01-00:00:00.25 THEN
            __RAISE('the wall clock');
        END_IF;
    END_FUNCTION

    {test}
    FUNCTION test_offset
        IF LOCAL_OFFSET(IN := WALL_LDT()) <> LT#1h THEN
            __RAISE('the offset');
        END_IF;
    END_FUNCTION
"#;

#[rstest]
fn the_stdlib_reads_wasi_s_clocks_through_a_component(mut with_db: db::RootDatabase) {
    let wasm = compile_with_libraries(
        &mut with_db,
        &[
            include_str!("../../../stdlib/Timers.st"),
            include_str!("../../../stdlib/Convert.st"),
            include_str!("../../../stdlib/Strings.st"),
        ],
        CLOCKS,
    );
    let manifest =
        TestManifest::from_msgpack(expect_section(&wasm, TEST_MANIFEST_SECTION)).unwrap();
    let test = |name: &str| {
        manifest
            .tests
            .iter()
            .position(|entry| entry.path == name)
            .unwrap_or_else(|| panic!("{name} in the manifest")) as u32
    };
    let component = wrap(&wasm);
    for clock in ["monotonic-clock", "system-clock", "timezone"] {
        let import = format!("import wasi:clocks/{clock}@0.3.0;");
        assert!(component.world.contains(&import), "{}", component.world);
    }

    // A zone an hour east of UTC at the instant the wall clock gave, and
    // none anywhere else.
    fn an_hour_east(when: Instant) -> Option<i64> {
        (when == WALL).then_some(3_600 * 1_000_000_000)
    }
    fn no_zone(_: Instant) -> Option<i64> {
        None
    }
    let wire = |zone: fn(Instant) -> Option<i64>| {
        move |linker: &mut Linker<()>| {
            let mut clock = linker
                .instance("wasi:clocks/monotonic-clock@0.3.0")
                .unwrap();
            clock.func_wrap("now", |_, ()| Ok((42u64,))).unwrap();
            let mut clock = linker.instance("wasi:clocks/system-clock@0.3.0").unwrap();
            clock.func_wrap("now", |_, ()| Ok((WALL,))).unwrap();
            let mut timezone = linker.instance("wasi:clocks/timezone@0.3.0").unwrap();
            timezone
                .func_wrap("utc-offset", move |_, (when,): (Instant,)| {
                    Ok((zone(when),))
                })
                .unwrap();
        }
    };

    let mut plc = Plc::load_with(&component.bytes, wire(an_hour_east));
    plc.init().unwrap();
    for name in ["test_monotonic", "test_wall", "test_offset"] {
        assert_eq!(plc.run_test(test(name)), Ok(()), "{name}");
    }

    let mut plc = Plc::load_with(&component.bytes, wire(no_zone));
    plc.init().unwrap();
    assert_eq!(
        plc.run_test(test("test_offset")),
        Err(TestError::Failed("the host has no time zone".to_string()))
    );
    assert_eq!(plc.run_test(test("test_wall")), Ok(()));
}

/// What `stamp` returns: its outputs, then the seconds under its own name.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(record)]
struct StampOutputs {
    calls: i32,
    nanoseconds: u32,
    stamp: i64,
}

/// A record of scalars a WASI function returns comes back in VAR_OUTPUT,
/// one for each of its fields in order: the system clock's `instant`, its
/// seconds then its nanoseconds, typed as WASI's WIT types `now`. What an
/// export wrote before it called the import is still there after.
#[rstest]
fn a_wasi_record_comes_back_in_outputs(mut with_db: db::RootDatabase) {
    let source = r#"
        {extern 'wasi:clocks/system-clock@0.3.0' 'now'}
        FUNCTION Now
        VAR_OUTPUT seconds : LINT; nanoseconds : UDINT; END_VAR
        END_FUNCTION

        {export}
        FUNCTION Stamp : LINT
        VAR_OUTPUT calls : DINT; nanoseconds : UDINT; END_VAR
        VAR seconds : LINT; END_VAR
            calls := 1;
            Now(seconds => seconds, nanoseconds => nanoseconds);
            Stamp := seconds;
        END_FUNCTION
    "#;
    let component = build(&mut with_db, source).expect("the module wraps");
    assert!(
        component
            .world
            .contains("import wasi:clocks/system-clock@0.3.0;"),
        "{}",
        component.world
    );
    let mut plc = Plc::load_with(&component.bytes, |linker| {
        let mut clock = linker.instance("wasi:clocks/system-clock@0.3.0").unwrap();
        clock.func_wrap("now", |_, ()| Ok((WALL,))).unwrap();
    });
    plc.init().unwrap();
    assert_eq!(
        plc.call("stamp", ()),
        Ok(StampOutputs {
            calls: 1,
            nanoseconds: WALL.nanoseconds,
            stamp: WALL.seconds,
        })
    );
}

/// A component whose host defines async functions, which wasmtime runs
/// through its async API only: each call driven to its end here, the host's
/// functions finishing when their test opens a [`Gate`].
struct AsyncPlc {
    store: wasmtime::Store<()>,
    instance: Instance,
}

/// Polls `future` to its end on this thread: the futures these tests' hosts
/// make wait on a [`Gate`], never on a clock or a socket.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
        std::thread::yield_now();
    }
}

impl AsyncPlc {
    fn load_with(bytes: &[u8], wire: impl FnOnce(&mut Linker<()>)) -> Self {
        let mut config = wasmtime::Config::new();
        config.wasm_exceptions(true);
        config.wasm_component_model_async(true);
        let engine = wasmtime::Engine::new(&config).expect("an engine");
        let component = Component::new(&engine, bytes).expect("a valid component");
        let mut linker = Linker::<()>::new(&engine);
        wire(&mut linker);
        linker
            .define_unknown_imports_as_traps(&component)
            .expect("its imports stubbed");
        let mut store = wasmtime::Store::new(&engine, ());
        let instance = block_on(linker.instantiate_async(&mut store, &component))
            .expect("the component instantiates");
        Self { store, instance }
    }

    fn func<P, R>(&mut self, name: &str) -> TypedFunc<P, R>
    where
        P: wasmtime::component::ComponentNamedList + wasmtime::component::Lower,
        R: wasmtime::component::ComponentNamedList + wasmtime::component::Lift,
    {
        let control = self
            .instance
            .get_export_index(&mut self.store, None, CONTROL)
            .expect("the component exports its control interface");
        let func = self
            .instance
            .get_export_index(&mut self.store, Some(&control), name)
            .unwrap_or_else(|| panic!("the interface has `{name}`"));
        self.instance
            .get_typed_func::<P, R>(&mut self.store, &func)
            .unwrap_or_else(|e| panic!("`{name}`: {e}"))
    }

    fn init(&mut self) -> Result<(), String> {
        let init = self.func::<(), (Result<(), String>,)>("init");
        block_on(init.call_async(&mut self.store, ()))
            .expect("init returns")
            .0
    }

    /// One scan of the task at `task`, its outputs as words.
    fn scan(&mut self, task: u32) -> Result<Vec<i32>, ScanError> {
        let run = self.func::<(u32, &[u8]), (Result<Vec<u8>, ScanError>,)>("run-task");
        let outputs = block_on(run.call_async(&mut self.store, (task, &[] as &[u8])))
            .expect("run-task returns")
            .0?;
        Ok(outputs
            .chunks(4)
            .map(|word| i32::from_le_bytes(word.try_into().unwrap()))
            .collect())
    }
}

/// What lets a host's async functions finish: opening it wakes each call
/// waiting on it, as finished I/O wakes the call it was for.
#[derive(Default)]
struct Gate {
    open: std::sync::atomic::AtomicBool,
    wakers: std::sync::Mutex<Vec<std::task::Waker>>,
}

impl Gate {
    fn open(&self) {
        self.open.store(true, std::sync::atomic::Ordering::SeqCst);
        for waker in self.wakers.lock().unwrap().drain(..) {
            waker.wake();
        }
    }

    /// A future that is ready once the gate is open, with `value`.
    fn wait<T: Send + 'static>(
        gate: std::sync::Arc<Gate>,
        value: T,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = wasmtime::Result<T>> + Send>> {
        let mut value = Some(value);
        Box::pin(std::future::poll_fn(move |cx| {
            if gate.open.load(std::sync::atomic::Ordering::SeqCst) {
                std::task::Poll::Ready(Ok(value.take().expect("polled once ready")))
            } else {
                gate.wakers.lock().unwrap().push(cx.waker().clone());
                std::task::Poll::Pending
            }
        }))
    }
}

/// A task that calls WASI's `wait-for` through a block at every scan, and
/// counts the scans and the calls that finished.
const WAITING: &str = r#"
    {extern async 'wasi:clocks/monotonic-clock@0.3.0' 'wait-for'}
    FUNCTION_BLOCK WAIT_FOR
    VAR_INPUT how_long : LTIME; END_VAR
    VAR_OUTPUT DONE : BOOL; END_VAR
    END_FUNCTION_BLOCK

    PROGRAM P
    VAR_EXTERNAL finished : DINT; scans : DINT; END_VAR
    VAR wait : WAIT_FOR; END_VAR
        wait(how_long := LT#5ms);
        scans := scans + 1;
        IF wait.DONE THEN
            finished := finished + 1;
        END_IF;
    END_PROGRAM

    CONFIGURATION Cfg
    VAR_GLOBAL
        finished AT %QD0 : DINT;
        scans AT %QD4 : DINT;
    END_VAR
        RESOURCE Res ON CPU
            TASK T(INTERVAL := T#10ms, PRIORITY := 1);
            PROGRAM Main WITH T : P;
        END_RESOURCE
    END_CONFIGURATION
"#;

/// A block's call is started by a scan and checked by the next ones, which
/// never wait: until the host finishes, DONE stays FALSE. The scan after
/// the host finished sees it, and a call the host finishes at once is DONE
/// in the scan that made it. The host gets the call's parameter.
#[rstest]
fn a_block_starts_its_call_and_checks_it_at_each_scan(mut with_db: db::RootDatabase) {
    let component = build(&mut with_db, WAITING).expect("the module wraps");
    let gate = std::sync::Arc::new(Gate::default());
    let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let (host, seen) = (gate.clone(), asked.clone());
    let mut plc = AsyncPlc::load_with(&component.bytes, move |linker| {
        let mut clock = linker
            .instance("wasi:clocks/monotonic-clock@0.3.0")
            .unwrap();
        clock
            .func_wrap_concurrent("wait-for", move |_, (how_long,): (u64,)| {
                seen.lock().unwrap().push(how_long);
                Gate::wait(host.clone(), ())
            })
            .unwrap();
    });
    plc.init().unwrap();
    for scan in 1..=3 {
        assert_eq!(plc.scan(0), Ok(vec![0, scan]), "the host has not finished");
    }
    assert_eq!(*asked.lock().unwrap(), [5_000_000], "one call, of 5 ms");
    gate.open();
    let finished: Vec<i32> = (0..3).map(|_| plc.scan(0).unwrap()[0]).collect();
    assert!(
        finished[0] <= 1 && finished[1] == 1,
        "the call is DONE once, within two scans of the host finishing: {finished:?}"
    );
    // The next calls the host finishes at once: DONE in the scan that made
    // each of them, one a scan.
    let before = plc.scan(0).unwrap()[0];
    assert_eq!(plc.scan(0).unwrap()[0], before + 1);
    assert_eq!(plc.scan(0).unwrap()[0], before + 2);
}

/// FUNCTIONs that move a block they do not hold: the PROGRAM's, handed as
/// VAR_IN_OUT, and a global's, reached through VAR_EXTERNAL.
const HANDED: &str = r#"
    {extern async 'wasi:clocks/monotonic-clock@0.3.0' 'wait-for'}
    FUNCTION_BLOCK WAIT_FOR
    VAR_INPUT how_long : LTIME; END_VAR
    VAR_OUTPUT DONE : BOOL; END_VAR
    END_FUNCTION_BLOCK

    FUNCTION Step : BOOL
    VAR_IN_OUT wait : WAIT_FOR; END_VAR
        wait(how_long := LT#5ms);
        Step := wait.DONE;
    END_FUNCTION

    FUNCTION StepShared : BOOL
    VAR_EXTERNAL shared : WAIT_FOR; END_VAR
        shared(how_long := LT#7ms);
        StepShared := shared.DONE;
    END_FUNCTION

    PROGRAM P
    VAR_EXTERNAL mine_done : DINT; shared_done : DINT; END_VAR
    VAR mine : WAIT_FOR; END_VAR
        IF Step(wait := mine) THEN
            mine_done := mine_done + 1;
        END_IF;
        IF StepShared() THEN
            shared_done := shared_done + 1;
        END_IF;
    END_PROGRAM

    CONFIGURATION Cfg
    VAR_GLOBAL
        shared : WAIT_FOR;
        mine_done AT %QD0 : DINT;
        shared_done AT %QD4 : DINT;
    END_VAR
        RESOURCE Res ON CPU
            TASK T(INTERVAL := T#10ms, PRIORITY := 1);
            PROGRAM Main WITH T : P;
        END_RESOURCE
    END_CONFIGURATION
"#;

/// A FUNCTION moves a block one step per call wherever the block lives: the
/// call is in the block's instance, not in the FUNCTION's frame, so it
/// starts in one scan and finishes at a later one, as when the PROGRAM calls
/// the block itself.
#[rstest]
fn a_function_moves_a_block_it_does_not_hold(mut with_db: db::RootDatabase) {
    let component = build(&mut with_db, HANDED).expect("the module wraps");
    let gate = std::sync::Arc::new(Gate::default());
    let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let (host, seen) = (gate.clone(), asked.clone());
    let mut plc = AsyncPlc::load_with(&component.bytes, move |linker| {
        let mut clock = linker
            .instance("wasi:clocks/monotonic-clock@0.3.0")
            .unwrap();
        clock
            .func_wrap_concurrent("wait-for", move |_, (how_long,): (u64,)| {
                seen.lock().unwrap().push(how_long);
                Gate::wait(host.clone(), ())
            })
            .unwrap();
    });
    plc.init().unwrap();
    for _ in 0..3 {
        assert_eq!(plc.scan(0), Ok(vec![0, 0]), "the host has not finished");
    }
    assert_eq!(
        *asked.lock().unwrap(),
        [5_000_000, 7_000_000],
        "one call each, from its own block"
    );
    gate.open();
    let finished = (0..3).map(|_| plc.scan(0).unwrap()).last().unwrap();
    assert!(
        finished[0] >= 1 && finished[1] >= 1,
        "both calls finished, each in its block: {finished:?}"
    );
    // The host now finishes each call at once: DONE in every scan.
    assert_eq!(plc.scan(0).unwrap(), vec![finished[0] + 1, finished[1] + 1]);
}

/// What `fetch` returns: the fields of its result, its DONE left out.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    wasmtime::component::ComponentType,
    wasmtime::component::Lift,
    wasmtime::component::Lower,
)]
#[component(record)]
struct FetchOutputs {
    value: u32,
    ok: bool,
}

/// Two blocks of one async import from a host's module, each its own call:
/// five values, a STRING among them, passed in memory, and a result of two
/// fields, written into each block's VAR_OUTPUTs.
const FETCHING: &str = r#"
    {extern async 'probe' 'fetch'}
    FUNCTION_BLOCK FETCH
    VAR_INPUT a : DINT; b : DINT; c : DINT; label : STRING[8]; END_VAR
    VAR_OUTPUT DONE : BOOL; value : UDINT; ok : BOOL; END_VAR
    END_FUNCTION_BLOCK

    PROGRAM P
    VAR_EXTERNAL first : UDINT; second : UDINT; END_VAR
    VAR one : FETCH; two : FETCH; END_VAR
        one(a := 1, b := 2, c := 3, label := 'ab');
        two(a := 10, b := 20, c := 30, label := 'xyz');
        IF one.DONE AND one.ok THEN
            first := one.value;
        END_IF;
        IF two.DONE AND two.ok THEN
            second := two.value;
        END_IF;
    END_PROGRAM

    CONFIGURATION Cfg
    VAR_GLOBAL
        first AT %QD0 : UDINT;
        second AT %QD4 : UDINT;
    END_VAR
        RESOURCE Res ON CPU
            TASK T(INTERVAL := T#10ms, PRIORITY := 1);
            PROGRAM Main WITH T : P;
        END_RESOURCE
    END_CONFIGURATION
"#;

/// The world declares the host's async function an `async func`, its
/// result the record of the block's VAR_OUTPUTs but DONE.
#[rstest]
fn a_host_s_async_function_is_an_async_func(mut with_db: db::RootDatabase) {
    let component = build(&mut with_db, FETCHING).expect("the module wraps");
    assert!(
        component.world.contains(
            "    import probe: interface {
        record fetch-outputs {
            value: u32,
            ok: bool,
        }
        fetch: async func(a: s32, b: s32, c: s32, label: string) -> fetch-outputs;
    }"
        ),
        "{}",
        component.world
    );
}

/// Each block's call reaches the host with its own parameters, and its
/// result lands in that block once the host finished.
#[rstest]
fn each_block_gets_its_own_call_s_result(mut with_db: db::RootDatabase) {
    let component = build(&mut with_db, FETCHING).expect("the module wraps");
    let gate = std::sync::Arc::new(Gate::default());
    let labels = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let (host, seen) = (gate.clone(), labels.clone());
    let mut plc = AsyncPlc::load_with(&component.bytes, move |linker| {
        let mut probe = linker.instance("probe").unwrap();
        probe
            .func_wrap_concurrent(
                "fetch",
                move |_, (a, b, c, label): (i32, i32, i32, String)| {
                    let value = (a + b + c) as u32 * 100 + label.len() as u32;
                    seen.lock().unwrap().push(label);
                    Gate::wait(host.clone(), (FetchOutputs { value, ok: true },))
                },
            )
            .unwrap();
    });
    plc.init().unwrap();
    assert_eq!(plc.scan(0), Ok(vec![0, 0]));
    assert_eq!(plc.scan(0), Ok(vec![0, 0]));
    assert_eq!(
        *labels.lock().unwrap(),
        ["ab", "xyz"],
        "one call each, so far"
    );
    gate.open();
    let outputs = (0..3).map(|_| plc.scan(0).unwrap()).last().unwrap();
    assert_eq!(outputs, vec![602, 6003]);
}

/// The declaration says whether a function is async, and the build checks
/// it against WASI's WIT: a block imports an `async func`, and a FUNCTION a
/// `func`. A scan never waits for a FUNCTION's call.
#[rstest]
fn the_build_checks_async_against_wasi(mut with_db: db::RootDatabase) {
    let block_of_a_func = r#"
        {extern async 'wasi:clocks/monotonic-clock@0.3.0' 'now'}
        FUNCTION_BLOCK NOW
        VAR_OUTPUT DONE : BOOL; mark : ULINT; END_VAR
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR now : NOW; END_VAR
            now();
        END_PROGRAM
    "#;
    assert_eq!(
        build(&mut with_db, block_of_a_func).err(),
        Some(
            "`now` of `wasi:clocks/monotonic-clock@0.3.0` is imported with {extern async}, and \
             WASI's WIT declares it a `func`: import it with {extern} on a FUNCTION"
                .to_string()
        )
    );

    let function_of_an_async_func = r#"
        {extern 'wasi:clocks/monotonic-clock@0.3.0' 'wait-for'}
        FUNCTION WAIT_FOR
        VAR_INPUT how_long : LTIME; END_VAR
        END_FUNCTION

        {export}
        FUNCTION Pause
            WAIT_FOR(how_long := LT#1s);
        END_FUNCTION
    "#;
    let mut db = db::RootDatabase::default();
    assert_eq!(
        build(&mut db, function_of_an_async_func).err(),
        Some(
            "`wait-for` of `wasi:clocks/monotonic-clock@0.3.0` is imported with {extern}, and \
             WASI's WIT declares it an `async func`: import it with {extern async} on a \
             FUNCTION_BLOCK"
                .to_string()
        )
    );
}
