// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The clock `Std.Timers` reads, on a host where the test moves it by hand.

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use debug_format::{DebugInfo, VarValue};
use rstest::rstest;

use super::{TestPlc, compile_with_libraries, with_db};

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
            linker.func_wrap("wasi:clocks/monotonic-clock@0.2.6", "now", move || {
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
