// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A built module wrapped into a component, run as a component host runs
//! it: through the control interface, with nothing but its typed functions
//! to call.

use debug_format::test_manifest::{TEST_MANIFEST_SECTION, TestManifest};
use rstest::rstest;
use wasmtime::component::{Component, Instance, Linker, TypedFunc};

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
        Err("a STRING passed to the host is not UTF-8".to_string())
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
