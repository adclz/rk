// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Generated declarations at run time, on wasmtime: the clocks through
//! aliases, a record's outputs and an option's, a wait through an async
//! block, on the host `rk test` runs tests on; standard input copied to
//! standard output through the streams' and futures' built-ins, random
//! numbers through a tuple's outputs, and the errors of WASI's sockets,
//! read out of the words they come back in, on a component host.

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;
use wasmtime_wasi::WasiCtxBuilder;
use wasmtime_wasi::cli::{IsTerminal, StdinStream, StdoutStream};

use super::{Bindings, built, generated, wrapped};
use crate::tests::cli::component_host::Plc;
use crate::tests::codegen::run_tests;
use crate::tests::utils::with_db;

/// The world of `interfaces`, WASI's, for the generator.
fn world_of(interfaces: &[&str]) -> String {
    let imports: String = interfaces
        .iter()
        .map(|interface| format!("    import {interface};\n"))
        .collect();
    format!("package rk:runtime;\n\nworld runtime {{\n{imports}}}\n")
}

/// The `{test}`s of `source`, built with `bindings` as `rk test` builds a
/// workspace and run on its host: each one's name and how it ended.
fn tested(db: &mut RootDatabase, bindings: &Bindings, source: &str) -> String {
    let (mut mir, _) = built(db, &bindings.files, &[source]);
    mir::raise_hook::add(db, &mut mir);
    let core = wasm_codegen::generate_wasm(db, &mir).finish();
    let component = wrapped(db, &mir, &core);
    run_tests(&component.bytes, None)
        .expect("the tests run")
        .iter()
        .map(|record| match &record.reason {
            Some(reason) => format!("{} {:?}: {reason}", record.name, record.status),
            None => format!("{} {:?}", record.name, record.status),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The clocks, read through the generated declarations: `NOW` returns a
/// `MARK`, an alias; the wall clock comes back as a record's outputs; the
/// zone's offset as an option's.
#[rstest]
fn the_clocks_read_through_generated_declarations(mut with_db: RootDatabase) {
    let bindings = generated(&world_of(&[
        "wasi:clocks/monotonic-clock@0.3.0",
        "wasi:clocks/system-clock@0.3.0",
        "wasi:clocks/timezone@0.3.0",
    ]));
    let source = r#"
        {test}
        FUNCTION test_the_clocks
        VAR
            first, second, resolution : ULINT;
            seconds : LINT;
            nanoseconds : UDINT;
            known : BOOL;
            offset : LINT;
        END_VAR
            first := Wasi.Clocks.MonotonicClock.NOW();
            second := Wasi.Clocks.MonotonicClock.NOW();
            IF second < first THEN __RAISE('the monotonic clock went back'); END_IF;
            resolution := Wasi.Clocks.MonotonicClock.GET_RESOLUTION();
            IF resolution = 0 THEN __RAISE('the monotonic clock has no resolution'); END_IF;
            Wasi.Clocks.SystemClock.NOW(seconds => seconds, nanoseconds => nanoseconds);
            IF seconds < 1700000000 THEN __RAISE('the wall clock is before 2023'); END_IF;
            IF nanoseconds >= 1000000000 THEN __RAISE('nanoseconds past a second'); END_IF;
            Wasi.Clocks.Timezone.UTC_OFFSET(
                when_seconds := seconds,
                when_nanoseconds := nanoseconds,
                is_some => known,
                value => offset);
            IF known AND (offset >= 86400000000000 OR offset <= -86400000000000) THEN
                __RAISE('an offset past a day');
            END_IF;
        END_FUNCTION
    "#;
    assert_snapshot!(tested(&mut with_db, &bindings, source), @"test_the_clocks Pass");
}

/// A wait through the generated async block: its calls take the wait a
/// step at a time, and `DONE` comes once the time has passed.
#[rstest]
fn a_wait_ends_once_its_time_has_passed(mut with_db: RootDatabase) {
    let bindings = generated(&world_of(&["wasi:clocks/monotonic-clock@0.3.0"]));
    let source = r#"
        {test}
        FUNCTION_BLOCK test_a_wait
        VAR_OUTPUT DONE : BOOL; END_VAR
        VAR
            wait : Wasi.Clocks.MonotonicClock.WAIT_FOR;
            started : BOOL;
            start, now : ULINT;
        END_VAR
            IF NOT started THEN
                start := Wasi.Clocks.MonotonicClock.NOW();
                started := TRUE;
            END_IF;
            wait(how_long := ULINT#2000000);
            IF wait.DONE THEN
                now := Wasi.Clocks.MonotonicClock.NOW();
                IF now - start < 2000000 THEN __RAISE('the wait ended early'); END_IF;
                DONE := TRUE;
            END_IF;
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(tested(&mut with_db, &bindings, source), @"test_a_wait Pass");
}

/// Bytes a test hands a host's standard input, and what its standard
/// output takes.
#[derive(Clone, Default)]
struct Pipe(Arc<Mutex<VecDeque<u8>>>);

impl tokio::io::AsyncRead for Pipe {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let mut bytes = self.0.lock().unwrap();
        let count = bytes.len().min(buf.remaining());
        let taken: Vec<u8> = bytes.drain(..count).collect();
        buf.put_slice(&taken);
        Poll::Ready(Ok(()))
    }
}

impl tokio::io::AsyncWrite for Pipe {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        self.0.lock().unwrap().extend(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl IsTerminal for Pipe {
    fn is_terminal(&self) -> bool {
        false
    }
}

impl StdinStream for Pipe {
    fn async_stream(&self) -> Box<dyn tokio::io::AsyncRead + Send + Sync> {
        Box::new(self.clone())
    }
}

impl StdoutStream for Pipe {
    fn async_stream(&self) -> Box<dyn tokio::io::AsyncWrite + Send + Sync> {
        Box::new(self.clone())
    }
}

/// `program`, a PROGRAM `P` that writes its verdict to `output`, built with
/// `bindings` and a task that runs it, on a component host with `stdin`
/// and `stdout`.
fn plc(
    db: &mut RootDatabase,
    bindings: &Bindings,
    program: &str,
    stdin: Pipe,
    stdout: Pipe,
) -> Plc {
    let ctx = WasiCtxBuilder::new().stdin(stdin).stdout(stdout).build();
    plc_with(db, bindings, program, ctx)
}

/// [`plc`] on a host with the WASI context `ctx`.
fn plc_with(
    db: &mut RootDatabase,
    bindings: &Bindings,
    program: &str,
    ctx: wasmtime_wasi::WasiCtx,
) -> Plc {
    let configuration = r#"
        CONFIGURATION Cfg
        VAR_GLOBAL output AT %QD0 : DINT; END_VAR
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM Main WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (mir, core) = built(db, &bindings.files, &[program, configuration]);
    let component = wrapped(db, &mir, &core);
    Plc::load(&component.bytes, ctx)
}

/// Scans until the output is `until`, at most `scans` times: the last
/// output.
fn scan_until(plc: &mut Plc, until: i32, scans: usize) -> i32 {
    let mut output = 0;
    for _ in 0..scans {
        output = plc.scan(0);
        if output == until {
            break;
        }
    }
    output
}

/// Standard input copied to standard output through the generated
/// declarations: stdin's stream and future, a stream made for stdout, its
/// reads and writes, the futures read, everything dropped. 1 when the copy
/// ended well, a negative number saying what went wrong otherwise.
const ECHO: &str = r#"
    PROGRAM P
    VAR_EXTERNAL output : DINT; END_VAR
    VAR
        step : INT;
        closing : BOOL;
        input, input_end, readable, writable, output_end : UDINT;
        chunk : Wasi.Cli.Stdin.BYTES_STREAM_READ;
        write : Wasi.Cli.Stdout.BYTES_STREAM_WRITE;
        written : Wasi.Cli.Stdout.RESULT_FUTURE_READ;
        read : Wasi.Cli.Stdin.RESULT_FUTURE_READ;
    END_VAR
        CASE step OF
        0:
            Wasi.Cli.Stdin.READ_VIA_STREAM(item0 => input, item1 => input_end);
            Wasi.Cli.Stdout.BYTES_STREAM_NEW(readable => readable, writable => writable);
            output_end := Wasi.Cli.Stdout.WRITE_VIA_STREAM(data := readable);
            step := 1;
        1:
            chunk(stream := input);
            IF chunk.DONE THEN
                IF chunk.closed THEN
                    Wasi.Cli.Stdin.BYTES_STREAM_DROP_READABLE(stream := input);
                    closing := TRUE;
                END_IF;
                IF chunk.read > 0 THEN
                    step := 2;
                ELSIF closing THEN
                    step := 3;
                END_IF;
            END_IF;
        2:
            write(stream := writable, data := chunk.data);
            IF write.DONE THEN
                IF write.closed OR write.written <> chunk.read THEN
                    output := -2;
                    step := 9;
                ELSIF closing THEN
                    step := 3;
                ELSE
                    step := 1;
                END_IF;
            END_IF;
        3:
            Wasi.Cli.Stdout.BYTES_STREAM_DROP_WRITABLE(stream := writable);
            step := 4;
        4:
            written(future := output_end);
            IF written.DONE THEN
                Wasi.Cli.Stdout.RESULT_FUTURE_DROP_READABLE(future := output_end);
                IF written.failed THEN output := -4; step := 9; ELSE step := 5; END_IF;
            END_IF;
        5:
            read(future := input_end);
            IF read.DONE THEN
                Wasi.Cli.Stdin.RESULT_FUTURE_DROP_READABLE(future := input_end);
                IF read.failed THEN output := -5; ELSE output := 1; END_IF;
                step := 9;
            END_IF;
        END_CASE;
    END_PROGRAM
"#;

/// What comes in on standard input goes out on standard output, chunk by
/// chunk, through nothing but generated declarations.
#[rstest]
fn standard_input_copied_to_standard_output(mut with_db: RootDatabase) {
    let bindings = generated(&world_of(&[
        "wasi:cli/stdin@0.3.0",
        "wasi:cli/stdout@0.3.0",
    ]));
    let text = "hello, rk: through generated streams and futures\n".repeat(200);
    let stdin = Pipe(Arc::new(Mutex::new(text.bytes().collect())));
    let stdout = Pipe::default();
    let mut plc = plc(&mut with_db, &bindings, ECHO, stdin, stdout.clone());
    assert_eq!(scan_until(&mut plc, 1, 200), 1);
    let copied: Vec<u8> = stdout.0.lock().unwrap().iter().copied().collect();
    assert_eq!(String::from_utf8(copied).unwrap(), text);
}

/// Random numbers through the generated declarations: a `u64` returned,
/// and a tuple's two outputs. 1 when they look random.
#[rstest]
fn random_numbers_through_generated_declarations(mut with_db: RootDatabase) {
    let bindings = generated(&world_of(&[
        "wasi:random/random@0.3.0",
        "wasi:random/insecure@0.3.0",
        "wasi:random/insecure-seed@0.3.0",
    ]));
    let program = r#"
        PROGRAM P
        VAR_EXTERNAL output : DINT; END_VAR
        VAR first, second, low, high, insecure : ULINT; END_VAR
            first := Wasi.Random.Random.GET_RANDOM_U64();
            second := Wasi.Random.Random.GET_RANDOM_U64();
            insecure := Wasi.Random.Insecure.GET_INSECURE_RANDOM_U64();
            Wasi.Random.InsecureSeed.GET_INSECURE_SEED(item0 => low, item1 => high);
            output := 0;
            IF first <> second THEN output := output + 1; END_IF;
            IF low <> high THEN output := output + 10; END_IF;
            IF insecure <> first AND insecure <> second THEN output := output + 100; END_IF;
        END_PROGRAM
    "#;
    let mut plc = plc(
        &mut with_db,
        &bindings,
        program,
        Pipe::default(),
        Pipe::default(),
    );
    assert_eq!(plc.scan(0), 111);
}

/// The errors of WASI's sockets, a variant, and the values beside them,
/// read out of the words they come back in: a socket made, the address it
/// has not, a setting refused, one taken and read back, a size, and the
/// error of a future, from a send on a socket that is not connected. 1
/// when each is what wasmtime's sockets give, and otherwise the number of
/// the check that failed, negative.
#[rstest]
fn the_errors_of_wasi_s_sockets_are_read_out_of_words(mut with_db: RootDatabase) {
    let bindings = generated(&world_of(&["wasi:sockets/types@0.3.0"]));
    let program = r#"
        PROGRAM P
        VAR_EXTERNAL output : DINT; END_VAR
        VAR
            step : INT;
            failed : BOOL;
            socket, readable, writable, sent : UDINT;
            error : Wasi.Sockets.Types.ERROR_CODE;
            address : Wasi.Sockets.Types.IP_SOCKET_ADDRESS;
            port : UINT;
            limit : USINT;
            size : ULINT;
            read : Wasi.Sockets.Types.RESULT_FUTURE_READ;
        END_VAR
            CASE step OF
            0:
                step := 9;
                Wasi.Sockets.Types.TCP_SOCKET_CREATE(
                    address_family := Wasi.Sockets.Types.IP_ADDRESS_FAMILY#IPV4,
                    failed => failed, value => socket, error => error);
                IF failed OR error <> 0 THEN output := -1; RETURN; END_IF;
                // A socket that is not bound has no address.
                Wasi.Sockets.Types.TCP_SOCKET_GET_LOCAL_ADDRESS(self := socket,
                    failed => failed, value => address, value_ipv4_port => port,
                    error => error);
                IF NOT failed OR error <> Wasi.Sockets.Types.ERROR_CODE#INVALID_STATE
                    OR address <> 0 OR port <> 0
                THEN
                    output := -2; RETURN;
                END_IF;
                // A hop limit of 0 is refused.
                Wasi.Sockets.Types.TCP_SOCKET_SET_HOP_LIMIT(self := socket, value := 0,
                    failed => failed, error => error);
                IF NOT failed OR error <> Wasi.Sockets.Types.ERROR_CODE#INVALID_ARGUMENT THEN
                    output := -3; RETURN;
                END_IF;
                Wasi.Sockets.Types.TCP_SOCKET_SET_HOP_LIMIT(self := socket, value := 42,
                    failed => failed, error => error);
                IF failed OR error <> 0 THEN output := -4; RETURN; END_IF;
                Wasi.Sockets.Types.TCP_SOCKET_GET_HOP_LIMIT(self := socket,
                    failed => failed, value => limit, error => error);
                IF failed OR limit <> 42 OR error <> 0 THEN output := -5; RETURN; END_IF;
                Wasi.Sockets.Types.TCP_SOCKET_GET_RECEIVE_BUFFER_SIZE(self := socket,
                    failed => failed, value => size, error => error);
                IF failed OR size = 0 OR error <> 0 THEN output := -6; RETURN; END_IF;
                // A send on a socket that is not connected: its future fails.
                Wasi.Sockets.Types.BYTES_STREAM_NEW(readable => readable, writable => writable);
                sent := Wasi.Sockets.Types.TCP_SOCKET_SEND(self := socket, data := readable);
                step := 1;
            1:
                read(future := sent);
                IF read.DONE THEN
                    step := 9;
                    IF NOT read.failed
                        OR read.error <> Wasi.Sockets.Types.ERROR_CODE#INVALID_STATE
                    THEN
                        output := -7; RETURN;
                    END_IF;
                    Wasi.Sockets.Types.RESULT_FUTURE_DROP_READABLE(future := sent);
                    Wasi.Sockets.Types.BYTES_STREAM_DROP_WRITABLE(stream := writable);
                    Wasi.Sockets.Types.TCP_SOCKET_DROP(handle := socket);
                    output := 1;
                END_IF;
            END_CASE;
        END_PROGRAM
    "#;
    // TCP is what a host allows: wasmtime's does not until told to.
    let ctx = WasiCtxBuilder::new().allow_tcp(true).build();
    let mut plc = plc_with(&mut with_db, &bindings, program, ctx);
    let mut output = 0;
    for _ in 0..50 {
        output = plc.scan(0);
        if output != 0 {
            break;
        }
    }
    assert_eq!(output, 1, "the number of the check that failed, negative");
}
