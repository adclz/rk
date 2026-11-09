// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A built module wrapped into a component, run as a component host runs
//! it: through the control interface, with nothing but its typed functions
//! to call.

use debug_format::test_manifest::{TEST_MANIFEST_SECTION, TestManifest};
use rstest::rstest;
use wasmtime::component::{Component, Instance, Linker, TypedFunc};

use crate::tests::codegen::{
    compile_to_wasm_as_built, compile_with_libraries, expect_section, with_db,
};

const CONTROL: &str = debug_format::wit::CONTROL_INTERFACE;

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
        let control = self
            .instance
            .get_export_index(&mut self.store, None, CONTROL)
            .expect("the component exports the control interface");
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
        init.call(&mut self.store, ()).expect("init returns").0
    }

    fn run_task(&mut self, task: u32, inputs: &[u8]) -> Result<Vec<u8>, String> {
        let run = self.func::<(u32, &[u8]), (Result<Vec<u8>, String>,)>("run-task");
        run.call(&mut self.store, (task, inputs))
            .expect("run-task returns")
            .0
    }

    fn run_test(&mut self, test: u32) -> Result<(), String> {
        let run = self.func::<(u32,), (Result<(), String>,)>("run-test");
        run.call(&mut self.store, (test,))
            .expect("run-test returns")
            .0
    }
}

fn wrap(wasm: &[u8]) -> rk::component::Component {
    rk::component::wrap(wasm, "plant").expect("the module wraps")
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
        Err("division by zero".to_string())
    );
    assert_eq!(
        plc.run_task(0, &8i32.to_le_bytes()),
        Ok(125i32.to_le_bytes().to_vec())
    );
    assert_eq!(plc.run_task(1, &[]), Err("no such task".to_string()));
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
    assert_eq!(plc.run_test(0), Err("pump stuck".to_string()));
    assert_eq!(plc.run_test(1), Err("no such test".to_string()));
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
    let component = wrap(&compile_to_wasm_as_built(&mut with_db, source));
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
    assert_eq!(exports, [CONTROL]);
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
    let refused = rk::component::wrap(&compile_to_wasm_as_built(&mut with_db, source), "plant")
        .err()
        .expect("the module is refused");
    assert_eq!(
        refused,
        "the module imports `wasi:clocks/monotonic-clock@0.2.6`, \
         which no WIT package rk carries describes"
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
        Err("the host has no time zone".to_string())
    );
    assert_eq!(plc.run_test(test("test_wall")), Ok(()));
}
