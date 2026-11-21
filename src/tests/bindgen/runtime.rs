// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Generated declarations at run time, on wasmtime: the clocks through
//! aliases, a record's STRUCT and an option's outputs, a wait through an
//! async block, on the host `rk test` runs tests on; standard input copied
//! to standard output through the streams' and futures' built-ins, random
//! numbers through a tuple's outputs, the errors of WASI's sockets, read
//! out of the words they come back in, and the addresses its sockets are
//! bound and connected to, packed into the words they are passed in, on a
//! component host.

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
/// `MARK`, an alias; the wall clock returns an `INSTANT`, the STRUCT of its
/// record, which the zone's offset takes, its offset coming back as an
/// option's outputs.
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
            instant : Wasi.Clocks.SystemClock.INSTANT;
            known : BOOL;
            offset : LINT;
        END_VAR
            first := Wasi.Clocks.MonotonicClock.NOW();
            second := Wasi.Clocks.MonotonicClock.NOW();
            IF second < first THEN __RAISE('the monotonic clock went back'); END_IF;
            resolution := Wasi.Clocks.MonotonicClock.GET_RESOLUTION();
            IF resolution = 0 THEN __RAISE('the monotonic clock has no resolution'); END_IF;
            instant := Wasi.Clocks.SystemClock.NOW();
            IF instant.seconds < 1700000000 THEN __RAISE('the wall clock is before 2023'); END_IF;
            IF instant.nanoseconds >= 1000000000 THEN __RAISE('nanoseconds past a second'); END_IF;
            Wasi.Clocks.Timezone.UTC_OFFSET(when := instant, is_some => known, value => offset);
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

/// Bytes that are no text, zeros and invalid UTF-8 among them, copied from
/// standard input to standard output through an array: each chunk read into
/// it, then written from it by reference, the count saying how many.
#[rstest]
fn bytes_are_copied_through_an_array(mut with_db: RootDatabase) {
    let bindings = generated(&world_of(&[
        "wasi:cli/stdin@0.3.0",
        "wasi:cli/stdout@0.3.0",
    ]));
    // ECHO, with an array where it has a STRING.
    let mut program = ECHO.to_string();
    for (text, array) in [
        ("BYTES_STREAM_READ;", "BYTES_STREAM_READ_ARRAY;"),
        ("BYTES_STREAM_WRITE;", "BYTES_STREAM_WRITE_ARRAY;"),
        (
            "write(stream := writable, data := chunk.data);",
            "write(stream := writable, data := REF(chunk.data[0]), count := chunk.read);",
        ),
    ] {
        assert_eq!(program.matches(text).count(), 1, "{text}");
        program = program.replace(text, array);
    }
    // Every byte, three chunks' worth.
    let bytes: Vec<u8> = (0..20_000u32).map(|at| (at * 7 % 256) as u8).collect();
    let stdin = Pipe(Arc::new(Mutex::new(bytes.iter().copied().collect())));
    let stdout = Pipe::default();
    let mut plc = plc(&mut with_db, &bindings, &program, stdin, stdout.clone());
    assert_eq!(scan_until(&mut plc, 1, 200), 1);
    let copied: Vec<u8> = stdout.0.lock().unwrap().iter().copied().collect();
    assert_eq!(copied, bytes);
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
                IF failed OR error.kind <> 0 THEN output := -1; RETURN; END_IF;
                // A socket that is not bound has no address.
                Wasi.Sockets.Types.TCP_SOCKET_GET_LOCAL_ADDRESS(self := socket,
                    failed => failed, value => address, error => error);
                IF NOT failed OR error.kind <> Wasi.Sockets.Types.ERROR_CODE_KIND#INVALID_STATE
                    OR address.kind <> 0 OR address.ipv4.port <> 0
                THEN
                    output := -2; RETURN;
                END_IF;
                // A hop limit of 0 is refused.
                Wasi.Sockets.Types.TCP_SOCKET_SET_HOP_LIMIT(self := socket, value := 0,
                    failed => failed, error => error);
                IF NOT failed OR error.kind <> Wasi.Sockets.Types.ERROR_CODE_KIND#INVALID_ARGUMENT
                THEN
                    output := -3; RETURN;
                END_IF;
                Wasi.Sockets.Types.TCP_SOCKET_SET_HOP_LIMIT(self := socket, value := 42,
                    failed => failed, error => error);
                IF failed OR error.kind <> 0 THEN output := -4; RETURN; END_IF;
                Wasi.Sockets.Types.TCP_SOCKET_GET_HOP_LIMIT(self := socket,
                    failed => failed, value => limit, error => error);
                IF failed OR limit <> 42 OR error.kind <> 0 THEN output := -5; RETURN; END_IF;
                Wasi.Sockets.Types.TCP_SOCKET_GET_RECEIVE_BUFFER_SIZE(self := socket,
                    failed => failed, value => size, error => error);
                IF failed OR size = 0 OR error.kind <> 0 THEN output := -6; RETURN; END_IF;
                // A send on a socket that is not connected: its future fails.
                Wasi.Sockets.Types.BYTES_STREAM_NEW(readable => readable, writable => writable);
                sent := Wasi.Sockets.Types.TCP_SOCKET_SEND(self := socket, data := readable);
                step := 1;
            1:
                read(future := sent);
                IF read.DONE THEN
                    step := 9;
                    IF NOT read.failed
                        OR read.error.kind <> Wasi.Sockets.Types.ERROR_CODE_KIND#INVALID_STATE
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

/// The writer of WASI's `send`, on a socket that is not connected: the
/// future `send` answers with fails, and its `error-code` is the writer's
/// `ERROR` and `ERRORID`. 1 when it is, and otherwise the number of the
/// check that failed, negative.
#[rstest]
fn a_writer_reports_the_error_of_the_future_it_reads(mut with_db: RootDatabase) {
    let bindings = generated(&world_of(&["wasi:sockets/types@0.3.0"]));
    let program = r#"
        PROGRAM P
        VAR_EXTERNAL output : DINT; END_VAR
        VAR
            step : INT;
            failed : BOOL;
            socket : UDINT;
            error : Wasi.Sockets.Types.ERROR_CODE;
            send : Wasi.Sockets.Types.TCP_SOCKET_SEND_WRITER;
        END_VAR
            CASE step OF
            0:
                Wasi.Sockets.Types.TCP_SOCKET_CREATE(
                    address_family := Wasi.Sockets.Types.IP_ADDRESS_FAMILY#IPV4,
                    failed => failed, value => socket, error => error);
                IF failed THEN output := -1; RETURN; END_IF;
                step := 1;
            1:
                send(REQ := TRUE, IN := 'x', self := socket);
                IF send.ENDED THEN
                    IF NOT send.ERROR
                        OR send.ERRORID.kind <> Wasi.Sockets.Types.ERROR_CODE_KIND#INVALID_STATE
                    THEN
                        output := -2; RETURN;
                    END_IF;
                    output := 1;
                END_IF;
            END_CASE;
        END_PROGRAM
    "#;
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

/// An address, a variant, passed to WASI's sockets and read back from
/// them: a UDP socket and a TCP socket bound to the loopback address, as
/// values, the cases sharing them; a second TCP socket connected to the
/// first's port in an async call, whose parameters are in memory, and the
/// address it is connected to. 1 when each address came back as it went,
/// and otherwise the number of the check that failed, negative.
#[rstest]
fn an_address_passed_to_wasi_s_sockets_comes_back(mut with_db: RootDatabase) {
    let bindings = generated(&world_of(&["wasi:sockets/types@0.3.0"]));
    let program = r#"
        PROGRAM P
        VAR_EXTERNAL output : DINT; END_VAR
        VAR
            step : INT;
            failed : BOOL;
            datagrams, listener, accepted, client : UDINT;
            error : Wasi.Sockets.Types.ERROR_CODE;
            loopback, address : Wasi.Sockets.Types.IP_SOCKET_ADDRESS;
            listening : UINT;
            connect : Wasi.Sockets.Types.TCP_SOCKET_CONNECT;
        END_VAR
            // 127.0.0.1, port 0: the host picks one.
            loopback.kind := Wasi.Sockets.Types.IP_SOCKET_ADDRESS_KIND#IPV4;
            loopback.ipv4.address.item0 := 127;
            loopback.ipv4.address.item3 := 1;
            CASE step OF
            0:
                step := 9;
                Wasi.Sockets.Types.UDP_SOCKET_CREATE(
                    address_family := Wasi.Sockets.Types.IP_ADDRESS_FAMILY#IPV4,
                    failed => failed, value => datagrams);
                IF failed THEN output := -1; RETURN; END_IF;
                Wasi.Sockets.Types.UDP_SOCKET_BIND(self := datagrams, local_address := loopback,
                    failed => failed, error => error);
                IF failed THEN output := -2; RETURN; END_IF;
                Wasi.Sockets.Types.UDP_SOCKET_GET_LOCAL_ADDRESS(self := datagrams,
                    failed => failed, value => address);
                IF failed OR address.kind <> Wasi.Sockets.Types.IP_SOCKET_ADDRESS_KIND#IPV4
                    OR address.ipv4.port = 0 OR address.ipv4.address.item0 <> 127
                    OR address.ipv4.address.item1 <> 0 OR address.ipv4.address.item2 <> 0
                    OR address.ipv4.address.item3 <> 1
                THEN
                    output := -3; RETURN;
                END_IF;

                Wasi.Sockets.Types.TCP_SOCKET_CREATE(
                    address_family := Wasi.Sockets.Types.IP_ADDRESS_FAMILY#IPV4,
                    failed => failed, value => listener);
                IF failed THEN output := -4; RETURN; END_IF;
                Wasi.Sockets.Types.TCP_SOCKET_BIND(self := listener, local_address := loopback,
                    failed => failed, error => error);
                IF failed THEN output := -5; RETURN; END_IF;
                Wasi.Sockets.Types.TCP_SOCKET_GET_LOCAL_ADDRESS(self := listener,
                    failed => failed, value => address);
                listening := address.ipv4.port;
                IF failed OR listening = 0 THEN output := -6; RETURN; END_IF;
                Wasi.Sockets.Types.TCP_SOCKET_LISTEN(self := listener,
                    failed => failed, value => accepted);
                IF failed THEN output := -7; RETURN; END_IF;
                Wasi.Sockets.Types.TCP_SOCKET_CREATE(
                    address_family := Wasi.Sockets.Types.IP_ADDRESS_FAMILY#IPV4,
                    failed => failed, value => client);
                IF failed THEN output := -8; RETURN; END_IF;
                // Bound here, or the host binds it to any address as it
                // connects, which this host does not allow.
                Wasi.Sockets.Types.TCP_SOCKET_BIND(self := client, local_address := loopback,
                    failed => failed, error => error);
                IF failed THEN output := -11; RETURN; END_IF;
                step := 1;
            1:
                address := loopback;
                address.ipv4.port := listening;
                connect(self := client, remote_address := address);
                IF connect.DONE THEN
                    step := 9;
                    IF connect.failed THEN output := -9; RETURN; END_IF;
                    Wasi.Sockets.Types.TCP_SOCKET_GET_REMOTE_ADDRESS(self := client,
                        failed => failed, value => address);
                    IF failed OR address.kind <> Wasi.Sockets.Types.IP_SOCKET_ADDRESS_KIND#IPV4
                        OR address.ipv4.port <> listening OR address.ipv4.address.item0 <> 127
                        OR address.ipv4.address.item3 <> 1
                    THEN
                        output := -10; RETURN;
                    END_IF;
                    output := 1;
                END_IF;
            END_CASE;
        END_PROGRAM
    "#;
    // WASI's sockets are tokio's: a host that listens and connects runs
    // within its runtime, which moves the connection on while the scans
    // check it.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("a tokio runtime");
    let _within = runtime.enter();
    // What a guest may bind and connect to is the host's to allow: here,
    // this machine alone.
    let mut ctx = WasiCtxBuilder::new();
    ctx.allow_tcp(true)
        .allow_udp(true)
        .socket_addr_check(|address, _| Box::pin(async move { address.ip().is_loopback() }));
    let mut plc = plc_with(&mut with_db, &bindings, program, ctx.build());
    let mut output = 0;
    for _ in 0..200 {
        output = plc.scan(0);
        if output != 0 {
            break;
        }
    }
    assert_eq!(output, 1, "the number of the check that failed, negative");
}

/// A listener hands over each connection it accepts as the handle of a
/// socket, which its stream gives into an ARRAY OF UDINT: a client
/// connects to a socket listening on the loopback address, the listener's
/// stream gives the accepted socket, whose peer is the client, and the
/// bytes the client sends come out of it, however many reads they take.
/// Each handle is the program's own, which it drops. 1 when each is what
/// it should be, and otherwise the number of the check that failed,
/// negative.
#[rstest]
fn a_listener_hands_over_the_sockets_it_accepts(mut with_db: RootDatabase) {
    let bindings = generated(&world_of(&["wasi:sockets/types@0.3.0"]));
    let program = r#"
        PROGRAM P
        VAR_EXTERNAL output : DINT; END_VAR
        VAR
            step : INT;
            failed : BOOL;
            // Each socket of its resource's type, a stream's end a UDINT.
            listener, client, accepted : Wasi.Sockets.Types.TCP_SOCKET;
            connections : UDINT;
            outgoing, sending, sent, incoming, ending : UDINT;
            loopback, address : Wasi.Sockets.Types.IP_SOCKET_ADDRESS;
            listening, peer : UINT;
            connect : Wasi.Sockets.Types.TCP_SOCKET_CONNECT;
            accept : Wasi.Sockets.Types.TCP_SOCKET_STREAM_READ;
            write : Wasi.Sockets.Types.BYTES_STREAM_WRITE;
            read : Wasi.Sockets.Types.BYTES_STREAM_READ_ARRAY;
            got : ARRAY[0..3] OF BYTE;
            count, taken : UDINT;
        END_VAR
            // 127.0.0.1, port 0: the host picks one.
            loopback.kind := Wasi.Sockets.Types.IP_SOCKET_ADDRESS_KIND#IPV4;
            loopback.ipv4.address.item0 := 127;
            loopback.ipv4.address.item3 := 1;
            CASE step OF
            0:
                step := 9;
                Wasi.Sockets.Types.TCP_SOCKET_CREATE(
                    address_family := Wasi.Sockets.Types.IP_ADDRESS_FAMILY#IPV4,
                    failed => failed, value => listener);
                IF failed THEN output := -1; RETURN; END_IF;
                Wasi.Sockets.Types.TCP_SOCKET_BIND(self := listener, local_address := loopback,
                    failed => failed);
                IF failed THEN output := -2; RETURN; END_IF;
                Wasi.Sockets.Types.TCP_SOCKET_GET_LOCAL_ADDRESS(self := listener,
                    failed => failed, value => address);
                listening := address.ipv4.port;
                IF failed OR listening = 0 THEN output := -3; RETURN; END_IF;
                Wasi.Sockets.Types.TCP_SOCKET_LISTEN(self := listener,
                    failed => failed, value => connections);
                IF failed THEN output := -4; RETURN; END_IF;
                Wasi.Sockets.Types.TCP_SOCKET_CREATE(
                    address_family := Wasi.Sockets.Types.IP_ADDRESS_FAMILY#IPV4,
                    failed => failed, value => client);
                IF failed THEN output := -5; RETURN; END_IF;
                // Bound here, or the host binds it to any address as it
                // connects, which this host does not allow.
                Wasi.Sockets.Types.TCP_SOCKET_BIND(self := client, local_address := loopback,
                    failed => failed);
                IF failed THEN output := -6; RETURN; END_IF;
                Wasi.Sockets.Types.TCP_SOCKET_GET_LOCAL_ADDRESS(self := client,
                    failed => failed, value => address);
                peer := address.ipv4.port;
                IF failed OR peer = 0 THEN output := -7; RETURN; END_IF;
                step := 1;
            1:
                address := loopback;
                address.ipv4.port := listening;
                connect(self := client, remote_address := address);
                IF connect.DONE THEN
                    step := 9;
                    IF connect.failed THEN output := -8; RETURN; END_IF;
                    step := 2;
                END_IF;
            2:
                accept(stream := connections);
                IF accept.DONE THEN
                    step := 9;
                    IF accept.read <> 1 OR accept.closed THEN output := -9; RETURN; END_IF;
                    accepted := accept.data[0];
                    // A socket of the program's own, connected to the client.
                    Wasi.Sockets.Types.TCP_SOCKET_GET_REMOTE_ADDRESS(self := accepted,
                        failed => failed, value => address);
                    IF failed OR address.ipv4.port <> peer
                        OR address.ipv4.address.item0 <> 127
                    THEN
                        output := -10; RETURN;
                    END_IF;
                    // The client sends what it writes into a stream; the
                    // accepted socket gives what it receives in another.
                    Wasi.Sockets.Types.BYTES_STREAM_NEW(readable => outgoing, writable => sending);
                    sent := Wasi.Sockets.Types.TCP_SOCKET_SEND(self := client, data := outgoing);
                    Wasi.Sockets.Types.TCP_SOCKET_RECEIVE(self := accepted,
                        item0 => incoming, item1 => ending);
                    step := 3;
                END_IF;
            3:
                write(stream := sending, data := 'ping');
                IF write.DONE THEN
                    step := 9;
                    IF write.written <> 4 OR write.closed THEN output := -11; RETURN; END_IF;
                    step := 4;
                END_IF;
            4:
                read(stream := incoming);
                IF read.DONE THEN
                    step := 9;
                    IF read.closed THEN output := -12; RETURN; END_IF;
                    taken := 0;
                    WHILE taken < read.read AND count < 4 DO
                        got[count] := read.data[taken];
                        taken := taken + 1;
                        count := count + 1;
                    END_WHILE;
                    IF taken < read.read THEN output := -13; RETURN; END_IF;
                    step := 4;
                    IF count = 4 THEN
                        step := 9;
                        // 'ping'
                        IF got[0] <> 16#70 OR got[1] <> 16#69 OR got[2] <> 16#6E
                            OR got[3] <> 16#67
                        THEN
                            output := -14; RETURN;
                        END_IF;
                        Wasi.Sockets.Types.BYTES_STREAM_DROP_READABLE(stream := incoming);
                        Wasi.Sockets.Types.BYTES_STREAM_DROP_WRITABLE(stream := sending);
                        Wasi.Sockets.Types.RESULT_FUTURE_DROP_READABLE(future := ending);
                        Wasi.Sockets.Types.RESULT_FUTURE_DROP_READABLE(future := sent);
                        Wasi.Sockets.Types.TCP_SOCKET_DROP(handle := accepted);
                        Wasi.Sockets.Types.TCP_SOCKET_DROP(handle := client);
                        Wasi.Sockets.Types.TCP_SOCKET_STREAM_DROP_READABLE(stream := connections);
                        Wasi.Sockets.Types.TCP_SOCKET_DROP(handle := listener);
                        output := 1;
                    END_IF;
                END_IF;
            END_CASE;
        END_PROGRAM
    "#;
    // WASI's sockets are tokio's: the host accepts, sends and receives
    // within its runtime while the scans check on it.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("a tokio runtime");
    let _within = runtime.enter();
    // What a guest may bind and connect to is the host's to allow: here,
    // this machine alone.
    let mut ctx = WasiCtxBuilder::new();
    ctx.allow_tcp(true)
        .socket_addr_check(|address, _| Box::pin(async move { address.ip().is_loopback() }));
    let mut plc = plc_with(&mut with_db, &bindings, program, ctx.build());
    assert_eq!(
        scan_until(&mut plc, 1, 500),
        1,
        "the number of the check that failed, negative"
    );
}

/// A datagram sent from the first bytes of an array, as a frame is: the
/// program sends 6 bytes of an 8-byte frame to a socket the test listens
/// on, from a UDP socket of its own bound to the loopback address. 1 when
/// the send ended well, and otherwise the number of the check that failed,
/// negative; the test then reads what came.
#[rstest]
fn a_datagram_goes_from_the_first_bytes_of_an_array(mut with_db: RootDatabase) {
    let receiver = std::net::UdpSocket::bind("127.0.0.1:0").expect("a socket of the test's own");
    receiver
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .expect("a time limit");
    let port = receiver.local_addr().expect("its address").port();
    let bindings = generated(&world_of(&["wasi:sockets/types@0.3.0"]));
    let program = r#"
        PROGRAM P
        VAR_EXTERNAL output : DINT; END_VAR
        VAR
            step : INT;
            failed : BOOL;
            socket : UDINT;
            loopback, target : Wasi.Sockets.Types.IP_SOCKET_ADDRESS;
            frame : ARRAY[0..7] OF BYTE := [16#12, 16#34, 16#00, 16#00, 16#00, 16#06, 16#FF, 16#FF];
            send : Wasi.Sockets.Types.UDP_SOCKET_SEND;
        END_VAR
            // 127.0.0.1, port 0: the host picks one.
            loopback.kind := Wasi.Sockets.Types.IP_SOCKET_ADDRESS_KIND#IPV4;
            loopback.ipv4.address.item0 := 127;
            loopback.ipv4.address.item3 := 1;
            CASE step OF
            0:
                step := 9;
                Wasi.Sockets.Types.UDP_SOCKET_CREATE(
                    address_family := Wasi.Sockets.Types.IP_ADDRESS_FAMILY#IPV4,
                    failed => failed, value => socket);
                IF failed THEN output := -1; RETURN; END_IF;
                Wasi.Sockets.Types.UDP_SOCKET_BIND(self := socket, local_address := loopback,
                    failed => failed);
                IF failed THEN output := -2; RETURN; END_IF;
                step := 1;
            1:
                target := loopback;
                target.ipv4.port := PORT;
                send(self := socket, data := frame, count := 6,
                    remote_address_is_some := TRUE, remote_address := target);
                IF send.DONE THEN
                    step := 9;
                    IF send.failed THEN output := -3; RETURN; END_IF;
                    Wasi.Sockets.Types.UDP_SOCKET_DROP(handle := socket);
                    output := 1;
                END_IF;
            END_CASE;
        END_PROGRAM
    "#
    .replace("PORT", &port.to_string());
    // WASI's sockets are tokio's: the host sends within its runtime while
    // the scans check on it.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("a tokio runtime");
    let _within = runtime.enter();
    // What a guest may bind and send to is the host's to allow: here, this
    // machine alone.
    let mut ctx = WasiCtxBuilder::new();
    ctx.allow_udp(true)
        .socket_addr_check(|address, _| Box::pin(async move { address.ip().is_loopback() }));
    let mut plc = plc_with(&mut with_db, &bindings, &program, ctx.build());
    assert_eq!(
        scan_until(&mut plc, 1, 500),
        1,
        "the number of the check that failed, negative"
    );
    let mut came = [0u8; 64];
    let (length, from) = receiver.recv_from(&mut came).expect("the datagram");
    assert_eq!(&came[..length], &[0x12, 0x34, 0x00, 0x00, 0x00, 0x06]);
    assert!(from.ip().is_loopback(), "{from}");
}
