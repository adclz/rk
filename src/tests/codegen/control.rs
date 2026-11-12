// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The `rk:plc/control` interface, called on the core module in the shapes
//! of the canonical ABI, as a component will call it: a list passed in is
//! placed by `cabi_realloc`, and a result is read at the address returned.

use rstest::rstest;

use super::{compile_to_mir_and_wasm, compile_to_wasm, test_engine, with_db};
use rk::control::{ScanError, TestError, TestState};

const INTERFACE: &str = wasm_codegen::control::INTERFACE;

/// A module instantiated on a core host, called through the interface.
struct Plc {
    store: wasmtime::Store<()>,
    instance: wasmtime::Instance,
    memory: wasmtime::Memory,
}

impl Plc {
    fn load(wasm: &[u8]) -> Self {
        let module = wasmtime::Module::new(&test_engine(), wasm).expect("a valid module");
        let mut store = wasmtime::Store::new(module.engine(), ());
        let (instance, memory) = super::instantiate_returning_memory(&mut store, &module);
        Self {
            store,
            instance,
            memory,
        }
    }

    fn func<P: wasmtime::WasmParams, R: wasmtime::WasmResults>(
        &mut self,
        name: &str,
    ) -> wasmtime::TypedFunc<P, R> {
        self.instance
            .get_typed_func::<P, R>(&mut self.store, name)
            .unwrap_or_else(|e| panic!("`{name}`: {e}"))
    }

    /// Places `bytes` where `cabi_realloc` says, as a host lowers a list.
    fn list(&mut self, bytes: &[u8]) -> (i32, i32) {
        let realloc = self.func::<(i32, i32, i32, i32), i32>("cabi_realloc");
        let ptr = realloc
            .call(&mut self.store, (0, 0, 1, bytes.len() as i32))
            .expect("cabi_realloc");
        self.memory
            .write(&mut self.store, ptr as usize, bytes)
            .expect("the list fits in memory");
        (ptr, bytes.len() as i32)
    }

    fn read(&self, ptr: u32, len: u32) -> Vec<u8> {
        let mut bytes = vec![0; len as usize];
        self.memory
            .read(&self.store, ptr as usize, &mut bytes)
            .expect("inside memory");
        bytes
    }

    /// The result at `at`, as the canonical ABI lays it out: its tag, then
    /// at 4 a list's `(ptr, len)`, or an error variant's case with what the
    /// case holds 4 bytes on: `(case, Some(text))` for a message.
    fn result(&self, at: i32) -> Result<Vec<u8>, (u8, Option<String>)> {
        let head = self.read(at as u32, 16);
        let word = |i: usize| u32::from_le_bytes(head[i..i + 4].try_into().unwrap());
        match head[0] {
            0 => Ok(self.read(word(4), word(8))),
            _ => {
                let case = head[4];
                let text = (case == 0).then(|| {
                    String::from_utf8(self.read(word(8), word(12))).expect("a message is text")
                });
                Err((case, text))
            }
        }
    }

    fn init(&mut self) -> Result<(), String> {
        let init = self.func::<(), i32>(&format!("{INTERFACE}#init"));
        let at = init.call(&mut self.store, ()).expect("init returns");
        // `result<_, string>`: the tag, and the message when there is one.
        let head = self.read(at as u32, 12);
        match head[0] {
            0 => Ok(()),
            _ => Err(String::from_utf8(self.read(
                u32::from_le_bytes(head[4..8].try_into().unwrap()),
                u32::from_le_bytes(head[8..12].try_into().unwrap()),
            ))
            .unwrap()),
        }
    }

    fn run_task(&mut self, task: i32, inputs: &[u8]) -> Result<Vec<u8>, ScanError> {
        let (ptr, len) = self.list(inputs);
        let run = self.func::<(i32, i32, i32), i32>(&format!("{INTERFACE}#run-task"));
        let at = run
            .call(&mut self.store, (task, ptr, len))
            .expect("run-task returns");
        self.result(at).map_err(|error| match error {
            (0, Some(message)) => ScanError::Raised(message),
            (1, None) => ScanError::NoSuchTask,
            other => panic!("no such scan error: {other:?}"),
        })
    }

    fn save_retain(&mut self) -> Vec<u8> {
        let save = self.func::<(), i32>(&format!("{INTERFACE}#save-retain"));
        let at = save.call(&mut self.store, ()).expect("save-retain returns");
        let head = self.read(at as u32, 8);
        let word = |i: usize| u32::from_le_bytes(head[i..i + 4].try_into().unwrap());
        self.read(word(0), word(4))
    }

    fn restore_retain(&mut self, bytes: &[u8]) {
        let (ptr, len) = self.list(bytes);
        let restore = self.func::<(i32, i32), ()>(&format!("{INTERFACE}#restore-retain"));
        restore
            .call(&mut self.store, (ptr, len))
            .expect("restore-retain returns");
    }

    fn run_test(&mut self, index: i32) -> Result<TestState, TestError> {
        let run = self.func::<i32, i32>(&format!("{INTERFACE}#run-test"));
        let at = run.call(&mut self.store, index).expect("run-test returns");
        // `result<test-state, test-error>`: the tag, and at 4 the state, a
        // byte, or the error's case.
        let head = self.read(at as u32, 4 + 1);
        if head[0] == 0 {
            return match head[4] {
                0 => Ok(TestState::Passed),
                1 => Ok(TestState::Running),
                other => panic!("no such test state: {other}"),
            };
        }
        match self.result(at) {
            Err((0, Some(message))) => Err(TestError::Failed(message)),
            Err((1, None)) => Err(TestError::NoSuchTest),
            other => panic!("no such test error: {other:?}"),
        }
    }
}

/// A task that reads one input and writes it back plus one, and keeps a
/// count of its scans that survives a restart.
const COUNTER: &str = r#"
    PROGRAM P
    VAR_EXTERNAL input : DINT; output : DINT; END_VAR
    VAR RETAIN scans : DINT; END_VAR
    VAR divisor : DINT := 1; END_VAR
        scans := scans + 1;
        output := input + scans * 1000 / divisor;
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

fn dint(bytes: &[u8]) -> i32 {
    i32::from_le_bytes(bytes[..4].try_into().expect("four bytes"))
}

/// A scan takes the inputs whole, runs the task's programs, and gives the
/// outputs back whole; `init` gives the starting values first.
#[rstest]
fn a_task_runs_on_the_inputs_it_is_given(mut with_db: db::RootDatabase) {
    let wasm = compile_to_wasm(&mut with_db, COUNTER);
    let mut plc = Plc::load(&wasm);
    assert_eq!(plc.init(), Ok(()));
    let outputs = plc
        .run_task(0, &41i32.to_le_bytes())
        .expect("the scan runs");
    assert_eq!(outputs.len(), 4, "the output band, whole");
    assert_eq!(dint(&outputs), 1041);
    let outputs = plc.run_task(0, &7i32.to_le_bytes()).expect("the scan runs");
    assert_eq!(dint(&outputs), 2007, "the second scan counts on");
}

/// A task the schedule does not have is an error, and so is a scan that
/// raised, after which the next scan runs: nothing is left to trap.
#[rstest]
fn what_stops_a_scan_is_an_error(mut with_db: db::RootDatabase) {
    let wasm = compile_to_wasm(&mut with_db, COUNTER);
    let mut plc = Plc::load(&wasm);
    assert_eq!(plc.init(), Ok(()));
    assert_eq!(plc.run_task(1, &[0; 4]), Err(ScanError::NoSuchTask));

    // A divisor of 0, written where the program reads it.
    let info = debug_format::DebugInfo::from_wasm(&wasm);
    let divisor = info.resolve("Main.divisor").expect("Main.divisor");
    let write = |plc: &mut Plc, value: i32| {
        plc.memory
            .write(
                &mut plc.store,
                divisor.address as usize,
                &value.to_le_bytes(),
            )
            .unwrap();
    };
    write(&mut plc, 0);
    assert_eq!(
        plc.run_task(0, &[0; 4]),
        Err(ScanError::Raised("division by zero".to_string()))
    );
    write(&mut plc, 1);
    let outputs = plc
        .run_task(0, &3i32.to_le_bytes())
        .expect("the next scan runs");
    assert_eq!(
        dint(&outputs),
        2003,
        "the raised scan counted before it raised"
    );
}

/// The retain band leaves with `save-retain` and comes back into another
/// instance with `restore-retain`, after its starting values.
#[rstest]
fn the_retain_band_survives_a_new_instance(mut with_db: db::RootDatabase) {
    let wasm = compile_to_wasm(&mut with_db, COUNTER);
    let mut plc = Plc::load(&wasm);
    plc.init().unwrap();
    plc.run_task(0, &[0; 4]).unwrap();
    plc.run_task(0, &[0; 4]).unwrap();
    let retained = plc.save_retain();
    assert!(!retained.is_empty());

    let mut restarted = Plc::load(&wasm);
    restarted.init().unwrap();
    restarted.restore_retain(&retained);
    let outputs = restarted.run_task(0, &[0; 4]).unwrap();
    assert_eq!(
        dint(&outputs),
        3000,
        "the third scan, counted on from the two before"
    );
}

/// A test is run by its place in the `test-manifest`, again and again in
/// one instance: one that raised and then passes reads as passed.
#[rstest]
fn a_test_runs_by_its_place_in_the_manifest(mut with_db: db::RootDatabase) {
    let source = r#"
        CONFIGURATION Cfg
        VAR_GLOBAL raised : BOOL; END_VAR
        END_CONFIGURATION

        {test}
        FUNCTION test_first_raises
        VAR_EXTERNAL raised : BOOL; END_VAR
            IF NOT raised THEN
                raised := TRUE;
                __RAISE('first run');
            END_IF;
        END_FUNCTION

        {test}
        FUNCTION test_passes
        END_FUNCTION
    "#;
    let (mir, wasm) = compile_to_mir_and_wasm(&mut with_db, source);
    let order: Vec<&str> = mir
        .test_manifest
        .tests
        .iter()
        .map(|t| t.path.as_str())
        .collect();
    let first = order
        .iter()
        .position(|path| *path == "test_first_raises")
        .unwrap() as i32;
    let passes = order
        .iter()
        .position(|path| *path == "test_passes")
        .unwrap() as i32;

    let mut plc = Plc::load(&wasm);
    assert_eq!(plc.run_test(passes), Ok(TestState::Passed));
    assert_eq!(
        plc.run_test(first),
        Err(TestError::Failed("first run".to_string()))
    );
    assert_eq!(
        plc.run_test(first),
        Ok(TestState::Passed),
        "the second run passes"
    );
    assert_eq!(plc.run_test(2), Err(TestError::NoSuchTest));
}

/// A release runs no test: it keeps none.
#[rstest]
fn a_release_runs_no_test(mut with_db: db::RootDatabase) {
    let source = r#"
        {test}
        FUNCTION test_passes
        END_FUNCTION
    "#;
    let (mir, _) = compile_to_mir_and_wasm(&mut with_db, source);
    let release =
        wasm_codegen::generate_wasm_profile(&with_db, &mir, wasm_codegen::Profile::Release)
            .finish();
    let mut plc = Plc::load(&release);
    assert_eq!(plc.run_test(0), Err(TestError::NoSuchTest));
}

/// `cabi_realloc` hands out the scratch piece by piece, as large as the
/// largest list a function of the interface takes, and a function that
/// takes a list starts it over. What does not fit lands past everything the
/// module laid out, in memory grown once to take it: nothing traps.
#[rstest]
fn the_scratch_takes_what_a_host_passes(mut with_db: db::RootDatabase) {
    let wasm = compile_to_wasm(&mut with_db, COUNTER);
    let mut plc = Plc::load(&wasm);
    let realloc = plc.func::<(i32, i32, i32, i32), i32>("cabi_realloc");
    let scratch = realloc
        .call(&mut plc.store, (0, 0, 1, 3))
        .expect("3 bytes fit");
    assert_eq!(scratch % 8, 0, "aligned for any list");
    assert_eq!(
        realloc.call(&mut plc.store, (0, 0, 1, 1)).ok(),
        Some(scratch + 3),
        "the next piece is past the first"
    );
    assert_eq!(
        realloc.call(&mut plc.store, (0, 0, 4, 0)).ok(),
        Some(scratch + 4),
        "aligned as asked"
    );

    // A function that takes a list starts the scratch over.
    plc.restore_retain(&[]);
    let largest = plc.save_retain().len().max(4) as i32;
    assert_eq!(
        realloc.call(&mut plc.store, (0, 0, 1, largest)).ok(),
        Some(scratch),
        "the largest list fits at its start"
    );

    // One byte more does not fit: it lands at the end of the memory, which
    // grows to take it, once.
    let size = |plc: &Plc| plc.memory.data_size(&plc.store);
    let laid_out = size(&plc);
    let long = 3 * 65536;
    let spilled = realloc
        .call(&mut plc.store, (0, 0, 1, long))
        .expect("a list too long is no trap");
    assert_eq!(spilled as usize, laid_out, "past what the module laid out");
    assert_eq!(size(&plc), laid_out + long as usize, "grown to take it");
    assert_eq!(
        realloc.call(&mut plc.store, (0, 0, 1, long)).ok(),
        Some(spilled),
        "the same place"
    );
    assert_eq!(size(&plc), laid_out + long as usize, "and no more memory");

    // A block that moves keeps its bytes.
    plc.restore_retain(&[]);
    let first = realloc.call(&mut plc.store, (0, 0, 1, 1)).unwrap();
    plc.memory
        .write(&mut plc.store, first as usize, &[7])
        .unwrap();
    let moved = realloc.call(&mut plc.store, (first, 1, 1, 2)).unwrap();
    assert_eq!(moved, first + 1);
    assert_eq!(plc.read(moved as u32, 1), [7]);
}

/// A list longer than its band is no trap either: `run-task` and
/// `restore-retain` take as much of it as the band holds.
#[rstest]
fn a_list_longer_than_its_band_is_cut(mut with_db: db::RootDatabase) {
    let wasm = compile_to_wasm(&mut with_db, COUNTER);
    let mut plc = Plc::load(&wasm);
    plc.init().unwrap();
    let retained = plc.save_retain();
    assert!(!retained.is_empty(), "COUNTER retains something");
    let mut long = retained.clone();
    long[0] ^= 0xFF;
    long.extend(std::iter::repeat_n(0xAA, 200_000));
    plc.restore_retain(&long);
    let restored = plc.save_retain();
    assert_eq!(restored.len(), retained.len());
    assert_eq!(restored[..], long[..retained.len()]);
}

/// The interface is there whatever the module declares: a module with no
/// task, no test and no starting value answers each function.
#[rstest]
fn an_empty_module_answers_the_interface(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION nothing : DINT
            nothing := 0;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let mut plc = Plc::load(&wasm);
    assert_eq!(plc.init(), Ok(()));
    assert_eq!(plc.run_task(0, &[]), Err(ScanError::NoSuchTask));
    assert_eq!(plc.save_retain(), Vec::<u8>::new());
    plc.restore_retain(&[]);
    assert_eq!(plc.run_test(0), Err(TestError::NoSuchTest));
}
