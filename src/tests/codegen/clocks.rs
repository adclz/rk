// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The clocks `Std.Timers` reads: WASI's system clock and time zone,
//! imported as the canonical ABI lays them out and wired as `rk test` wires
//! them. One module holds every reading, because lowering `Std.Convert` is
//! what the test spends its time on.

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use debug_format::{DebugInfo, VarValue};
use rstest::rstest;

use super::{TestPlc, compile_with_libraries, fault_message, test_engine, with_db};

/// The interfaces `Std.Timers` imports its clocks from.
const MONOTONIC_CLOCK: &str = "wasi:clocks/monotonic-clock@0.3.0";
const SYSTEM_CLOCK: &str = "wasi:clocks/system-clock@0.3.0";
const TIMEZONE: &str = "wasi:clocks/timezone@0.3.0";

/// What the module reads, each a FUNCTION returning a LINT. Their names
/// are not the stdlib's: names are caseless, and a `wall_ldt` here would be
/// what `WALL_LDT()` names in its own body.
const READINGS: &str = "
USING Std.Timers;
USING Std.Convert;

FUNCTION read_wall_ldt : LINT
    read_wall_ldt := LDT_TO_LINT(WALL_LDT());
END_FUNCTION

FUNCTION read_wall_dt : LINT
    read_wall_dt := DT_TO_LINT(WALL_DT());
END_FUNCTION

(* The zone's offset at an instant given in nanoseconds since 1970-01-01. *)
FUNCTION read_offset_at : LINT
VAR_INPUT
    instant : LINT;
END_VAR
    read_offset_at := LTIME_TO_LINT(LOCAL_OFFSET(IN := LINT_TO_LDT(instant)));
END_FUNCTION

FUNCTION read_offset_now : LINT
    read_offset_now := LTIME_TO_LINT(LOCAL_OFFSET(IN := WALL_LDT()));
END_FUNCTION

(* How far LOCAL_LDT is from WALL_LDT read just before it. *)
FUNCTION read_local_shift : LINT
VAR
    wall, local : LDT;
END_VAR
    wall := WALL_LDT();
    local := LOCAL_LDT();
    read_local_shift := LDT_TO_LINT(local) - LDT_TO_LINT(wall);
END_FUNCTION
";

/// The readings, compiled against `Std.Timers` and what it reads.
struct Readings {
    engine: wasmtime::Engine,
    module: wasmtime::Module,
}

impl Readings {
    fn compile(db: &mut db::RootDatabase) -> Self {
        let wasm = compile_with_libraries(
            db,
            &[
                include_str!("../../../stdlib/Timers.st"),
                include_str!("../../../stdlib/Convert.st"),
                include_str!("../../../stdlib/Strings.st"),
            ],
            READINGS,
        );
        let engine = test_engine();
        let module = wasmtime::Module::new(&engine, &wasm).unwrap();
        Self { engine, module }
    }

    /// Instantiates the module on a host wired as `rk test` wires it, with
    /// a wall clock stopped at `wall`, WASI's `instant`, in place of the
    /// host's, and `zone` in place of its `utc-offset`, when they are given.
    fn on_host(&self, wall: Option<(i64, u32)>, zone: Option<fn(i64, i32) -> Option<i64>>) -> Host {
        let mut store = wasmtime::Store::new(&self.engine, ());
        let mut linker = wasmtime::Linker::new(&self.engine);
        rk::test_host::wire_clocks(&mut linker, &self.module).unwrap();
        linker.allow_shadowing(true);
        if let Some(instant) = wall {
            rk::test_host::wire_wall(&mut linker, SYSTEM_CLOCK, move || instant).unwrap();
        }
        if let Some(zone) = zone {
            rk::test_host::wire_zone(&mut linker, TIMEZONE, zone).unwrap();
        }
        let instance = linker.instantiate(&mut store, &self.module).unwrap();
        Host { store, instance }
    }
}

struct Host {
    store: wasmtime::Store<()>,
    instance: wasmtime::Instance,
}

impl Host {
    fn read(&mut self, reading: &str) -> i64 {
        let reading = self
            .instance
            .get_typed_func::<(), i64>(&mut self.store, reading)
            .unwrap();
        reading.call(&mut self.store, ()).unwrap()
    }

    /// The message `reading` raises.
    fn raised(&mut self, reading: &str) -> String {
        let call = self
            .instance
            .get_typed_func::<(), i64>(&mut self.store, reading)
            .unwrap();
        let err = call
            .call(&mut self.store, ())
            .expect_err("the reading raises");
        let memory = rk::test_host::memory_of(&mut self.store, &self.instance).unwrap();
        fault_message(&mut self.store, memory, err)
    }

    fn offset_at(&mut self, instant: i64) -> i64 {
        let offset_at = self
            .instance
            .get_typed_func::<i64, i64>(&mut self.store, "read_offset_at")
            .unwrap();
        offset_at.call(&mut self.store, instant).unwrap()
    }
}

/// The host's clock, in nanoseconds since 1970-01-01.
fn host_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i64
}

/// The host zone's offset at an instant, in nanoseconds, east positive.
fn host_offset(seconds: i64) -> i64 {
    let at = jiff::Timestamp::from_second(seconds).unwrap();
    i64::from(jiff::tz::TimeZone::system().to_offset(at).seconds()) * 1_000_000_000
}

const SECOND: i64 = 1_000_000_000;

#[rstest]
fn the_clocks_read_the_host(mut with_db: db::RootDatabase) {
    let readings = Readings::compile(&mut with_db);

    // `WALL_LDT` is the instant the host's clock shows, read through the
    // `instant` record the import writes where the module points, and
    // `WALL_DT` the same instant in seconds.
    let mut host = readings.on_host(None, None);
    let before = host_now();
    let wall = host.read("read_wall_ldt");
    let after = host_now();
    assert!(
        before <= wall && wall <= after,
        "{before} <= {wall} <= {after}"
    );
    let before = host_now() / SECOND;
    let wall = host.read("read_wall_dt");
    let after = host_now() / SECOND;
    assert!(
        before <= wall && wall <= after,
        "{before} <= {wall} <= {after}"
    );

    // `LOCAL_OFFSET` is the host zone's offset at the instant, now and at a
    // summer noon the zone's rules may shift, as `rk test` answers it.
    assert_eq!(
        host.read("read_offset_now"),
        host_offset(host_now() / SECOND)
    );
    let summer_noon = 1_782_907_200; // 2026-07-01 12:00:00 UTC
    assert_eq!(
        host.offset_at(summer_noon * SECOND),
        host_offset(summer_noon)
    );

    // The instant reaches the zone as WASI's `instant` counts it: the whole
    // seconds since 1970-01-01, rounded down, so negative before, and the
    // nanoseconds after them. This zone answers with what it was given, as
    // `seconds * 1000 + milliseconds`: the instant in milliseconds, rounded
    // down.
    fn echo(seconds: i64, nanoseconds: i32) -> Option<i64> {
        assert!((0..1_000_000_000).contains(&nanoseconds), "{nanoseconds}");
        Some(seconds * 1_000 + i64::from(nanoseconds) / 1_000_000)
    }
    let mut host = readings.on_host(None, Some(echo));
    for (instant, expected) in [
        (SECOND + 250_000_000, 1_250), // 1970-01-01 00:00:01.25
        (0, 0),
        (-500_000_000, -500),            // 1969-12-31 23:59:59.5
        (-SECOND - 250_000_000, -1_250), // 1969-12-31 23:59:58.75
        (i64::MIN, i64::MIN.div_euclid(1_000_000)),
    ] {
        assert_eq!(host.offset_at(instant), expected, "{instant}");
    }

    // A wall clock stopped at an instant reads it exactly: `WALL_LDT` to the
    // nanosecond and `WALL_DT` to the second, rounded down. Before 1970 the
    // seconds are negative and the nanoseconds still count forward.
    let summer_noon_and_a_bit = (summer_noon, 123_456_789);
    for (instant, ldt, dt) in [
        (
            summer_noon_and_a_bit,
            summer_noon * SECOND + 123_456_789,
            summer_noon,
        ),
        ((0, 0), 0, 0),
        ((-1, 999_999_999), -1, -1), // 1969-12-31 23:59:59.999999999
        ((-1, 0), -SECOND, -1),      // 1969-12-31 23:59:59
    ] {
        let mut host = readings.on_host(Some(instant), None);
        assert_eq!(host.read("read_wall_ldt"), ldt, "{instant:?}");
        assert_eq!(host.read("read_wall_dt"), dt, "{instant:?}");
    }
    // The zone is asked about the instant the wall clock shows, split back
    // into WASI's seconds and nanoseconds: a nanosecond before 1970 is the
    // second before, and 999,999,999 nanoseconds after it.
    let mut host = readings.on_host(Some((-1, 999_999_999)), Some(echo));
    assert_eq!(host.read("read_offset_now"), -1_000 + 999);

    // `LOCAL_LDT` is `WALL_LDT` shifted by the zone's offset, in the same
    // encoding: with a zone an hour east, an hour later.
    fn an_hour_east(_: i64, _: i32) -> Option<i64> {
        Some(3_600 * SECOND)
    }
    let mut host = readings.on_host(Some(summer_noon_and_a_bit), Some(an_hour_east));
    assert_eq!(host.read("read_local_shift"), 3_600 * SECOND);

    // A host with no zone answers none, and what needs the zone raises.
    fn no_zone(_: i64, _: i32) -> Option<i64> {
        None
    }
    let mut host = readings.on_host(Some(summer_noon_and_a_bit), Some(no_zone));
    for reading in ["read_offset_now", "read_local_shift"] {
        assert_eq!(
            host.raised(reading),
            "the host has no time zone",
            "{reading}"
        );
    }
    assert_eq!(
        host.read("read_wall_ldt"),
        summer_noon * SECOND + 123_456_789,
        "the wall clock needs no zone"
    );
}

/// A program running the three `TIME` timers, started together.
const TIMERS: &str = "
USING Std.Timers;

PROGRAM Main
VAR
    ton : TON_TIME;
    tp : TP_TIME;
    tof : TOF_TIME;
    (* The host drops it, which starts `tof`. *)
    hold : BOOL := TRUE;
END_VAR
    ton(IN := TRUE, PT := T#100ms);
    tp(IN := TRUE, PT := T#100ms);
    tof(IN := hold, PT := T#100ms);
END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM Run WITH T : Main;
    END_RESOURCE
END_CONFIGURATION
";

/// The timers on a monotonic clock the test moves by hand.
struct Timers {
    plc: TestPlc,
    info: DebugInfo,
    clock: Arc<AtomicI64>,
}

impl Timers {
    /// Loads the program with the monotonic clock at `now`, in nanoseconds,
    /// and starts the three timers there: `ton` and `tp` on the first scan,
    /// `tof` on the second, after `hold` dropped.
    fn started_at(wasm: &[u8], now: i64) -> Self {
        let clock = Arc::new(AtomicI64::new(now));
        let moved = Arc::clone(&clock);
        let plc = TestPlc::load_with_host(wasm, move |linker| {
            linker.func_wrap(MONOTONIC_CLOCK, "now", move || {
                moved.load(Ordering::Relaxed)
            })?;
            Ok(())
        })
        .unwrap();
        let info = DebugInfo::from_wasm(wasm);
        let mut timers = Self { plc, info, clock };
        timers.plc.scan().unwrap();
        timers
            .plc
            .write_var(&timers.info, "Run.hold", VarValue::Bool(false))
            .unwrap();
        timers.plc.scan().unwrap();
        timers
    }

    /// Moves the clock on by `elapsed` milliseconds and scans.
    fn after_ms(&mut self, elapsed: i64) {
        self.clock.fetch_add(elapsed * 1_000_000, Ordering::Relaxed);
        self.plc.scan().unwrap();
    }

    /// `Q` of each timer: `ton`, `tp`, `tof`.
    fn q(&self) -> [bool; 3] {
        ["Run.ton.Q", "Run.tp.Q", "Run.tof.Q"].map(|path| {
            match self.plc.read_var(&self.info, path) {
                Some(VarValue::Bool(q)) => q,
                other => panic!("{path}: {other:?}"),
            }
        })
    }
}

/// `PLC_TIME` cycles every 49.7 days, from wherever the host starts the
/// monotonic clock, and a timer must not notice: one started 50 ms before
/// the wrap neither expires early nor late, and one that expires after the
/// wrap does expire.
#[rstest]
fn the_timers_survive_the_clocks_wrap(mut with_db: db::RootDatabase) {
    let wasm = compile_with_libraries(
        &mut with_db,
        &[
            include_str!("../../../stdlib/Timers.st"),
            include_str!("../../../stdlib/Convert.st"),
            include_str!("../../../stdlib/Strings.st"),
        ],
        TIMERS,
    );
    const MS: i64 = 1_000_000;
    let wrap = (1i64 << 31) * MS;

    // Started 50 ms before the wrap: `START_TIME + PT` would wrap while the
    // clock has not, and read as the past.
    let mut timers = Timers::started_at(&wasm, wrap - 50 * MS);
    assert_eq!(timers.q(), [false, true, true], "just started");
    timers.after_ms(40);
    assert_eq!(
        timers.q(),
        [false, true, true],
        "40 ms of 100, before the wrap"
    );
    timers.after_ms(70);
    assert_eq!(
        timers.q(),
        [true, false, false],
        "110 ms of 100, past the wrap"
    );

    // Started 200 ms before the wrap: the clock wraps before `START_TIME +
    // PT` would, and read as the future for 24.8 days.
    let mut timers = Timers::started_at(&wasm, wrap - 200 * MS);
    timers.after_ms(250);
    assert_eq!(
        timers.q(),
        [true, false, false],
        "250 ms of 100, past the wrap"
    );
}
