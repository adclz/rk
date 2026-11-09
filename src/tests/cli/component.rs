// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A built module wrapped into a component, run as a component host runs
//! it: through the control interface, with nothing but its typed functions
//! to call.

use rstest::rstest;
use wasmtime::component::{Component, Instance, Linker, TypedFunc};

use crate::tests::codegen::{compile_to_wasm_as_built, with_db};

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
        let engine = engine();
        let component = Component::new(&engine, bytes).expect("a valid component");
        let mut linker = Linker::<()>::new(&engine);
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
        {extern 'wasi:clocks/monotonic-clock@0.2.6' 'now'}
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
            .contains("import wasi:clocks/monotonic-clock@0.2.6;"),
        "{}",
        component.world
    );
    assert!(component.world.contains(&format!("export {CONTROL};")));
    let files: Vec<&str> = component
        .wit
        .iter()
        .map(|(path, _)| path.as_str())
        .collect();
    for file in [
        "deps/rk-plc/control.wit",
        "deps/wasi-clocks/monotonic-clock.wit",
        "deps/wasi-io/poll.wit",
    ] {
        assert!(files.contains(&file), "{file} in {files:?}");
    }

    let engine = engine();
    let loaded = Component::new(&engine, &component.bytes).expect("a valid component");
    let ty = loaded.component_type();
    let imports: Vec<&str> = ty.imports(&engine).map(|(name, _)| name).collect();
    assert_eq!(imports, ["wasi:clocks/monotonic-clock@0.2.6"]);
    let exports: Vec<&str> = ty.exports(&engine).map(|(name, _)| name).collect();
    assert_eq!(exports, [CONTROL]);
}
