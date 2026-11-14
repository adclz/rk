// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Reading a stream and a future through `{extern async}` blocks, over the
//! component model's `stream.read` and `future.read`: WASI 0.3's standard
//! input, on a host whose input is ready, held back, or broken. A scan
//! never waits: a read the host cannot answer yet stays pending, and a
//! later scan finds it ended.

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use rstest::rstest;
use wasmtime_wasi::WasiCtxBuilder;
use wasmtime_wasi::cli::{IsTerminal, StdinStream};

use super::component_host::{Gate, Plc};
use crate::tests::codegen::{compile_to_mir_and_wasm_as_built, with_db};

/// What a task that reads its standard input says, in its output: ten for
/// each chunk that came, a hundred for each that came as expected, one once
/// the stream ended, a thousand once its future gave how, and ten thousand
/// when that was a failure.
fn verdict(chunks: i32, expected: i32, failed: bool) -> i32 {
    chunks * 10 + expected * 100 + 1 + 1000 + if failed { 10_000 } else { 0 }
}

/// stdin's `read-via-stream` and the built-ins for its stream and its
/// future, with the stream read's block whose bytes land in `buffer`.
fn imports(buffer: &str) -> String {
    format!(
        r#"
    {{extern 'wasi:cli/stdin@0.3.0' 'read-via-stream'}}
    FUNCTION READ_STDIN
    VAR_OUTPUT stream : UDINT; finished : UDINT; END_VAR
    END_FUNCTION

    {{extern async 'wasi:cli/stdin@0.3.0' '[stream-read-0]read-via-stream'}}
    FUNCTION_BLOCK READ_CHUNK
    VAR_INPUT stream : UDINT; END_VAR
    VAR_OUTPUT DONE : BOOL; data : {buffer}; read : UDINT; closed : BOOL; END_VAR
    END_FUNCTION_BLOCK

    {{extern 'wasi:cli/stdin@0.3.0' '[stream-drop-readable-0]read-via-stream'}}
    FUNCTION DROP_STREAM
    VAR_INPUT stream : UDINT; END_VAR
    END_FUNCTION

    {{extern async 'wasi:cli/stdin@0.3.0' '[future-read-1]read-via-stream'}}
    FUNCTION_BLOCK READ_END
    VAR_INPUT future : UDINT; END_VAR
    VAR_OUTPUT DONE : BOOL; failed : BOOL; code : USINT; END_VAR
    END_FUNCTION_BLOCK

    {{extern 'wasi:cli/stdin@0.3.0' '[future-drop-readable-1]read-via-stream'}}
    FUNCTION DROP_FUTURE
    VAR_INPUT future : UDINT; END_VAR
    END_FUNCTION
"#
    )
}

/// A task that reads its standard input chunk by chunk, `checks` judging
/// each chunk by its place, `chunks`, until the stream ends, and reads the
/// future that says how it ended, and drops both. It reads the future once
/// the stream ended, or from its first scan when `alongside`.
fn reader(buffer: &str, checks: &str, alongside: bool) -> String {
    format!(
        r#"{imports}
    PROGRAM P
    VAR_EXTERNAL output : DINT; END_VAR
    VAR
        opened, closed, ended, reading : BOOL;
        alongside : BOOL := {alongside};
        stream, finished : UDINT;
        chunk : READ_CHUNK;
        ending : READ_END;
        chunks, expected : DINT;
    END_VAR
        IF NOT opened THEN
            READ_STDIN(stream => stream, finished => finished);
            opened := TRUE;
        END_IF;
        reading := NOT closed;
        IF NOT closed THEN
            chunk(stream := stream);
            IF chunk.DONE THEN
                IF chunk.read > 0 THEN
{checks}
                    chunks := chunks + 1;
                END_IF;
                IF chunk.closed THEN
                    DROP_STREAM(stream := stream);
                    closed := TRUE;
                END_IF;
            END_IF;
        END_IF;
        IF NOT ended AND (alongside OR NOT reading) THEN
            ending(future := finished);
            IF ending.DONE THEN
                DROP_FUTURE(future := finished);
                ended := TRUE;
            END_IF;
        END_IF;
        output := chunks * 10 + expected * 100;
        IF closed THEN
            output := output + 1;
        END_IF;
        IF ended THEN
            output := output + 1000;
            IF ending.failed THEN
                output := output + 10000;
            END_IF;
        END_IF;
    END_PROGRAM

    CONFIGURATION Cfg
    VAR_GLOBAL output AT %QD0 : DINT; END_VAR
        RESOURCE Res ON CPU
            TASK T(INTERVAL := T#10ms, PRIORITY := 1);
            PROGRAM Main WITH T : P;
        END_RESOURCE
    END_CONFIGURATION
"#,
        imports = imports(buffer),
        alongside = if alongside { "TRUE" } else { "FALSE" },
    )
}

/// A reader of text, in a STRING of 4: "hello, rk" comes as "hell",
/// "o, r", "k".
fn text_reader(alongside: bool) -> String {
    reader(
        "STRING[4]",
        r#"
                    IF chunks = 0 AND chunk.data = 'hell' THEN expected := expected + 1; END_IF;
                    IF chunks = 1 AND chunk.data = 'o, r' THEN expected := expected + 1; END_IF;
                    IF chunks = 2 AND chunk.data = 'k' THEN expected := expected + 1; END_IF;"#,
        alongside,
    )
}

/// The text, held back until `gate` opens.
fn held_text(gate: &Arc<Gate>) -> Stdin {
    Stdin::Gated {
        gate: gate.clone(),
        bytes: Arc::new(Mutex::new(b"hello, rk".iter().copied().collect())),
    }
}

/// A host's standard input, as a test gives it.
#[derive(Clone)]
enum Stdin {
    /// The bytes, there at once.
    Ready(Arc<Mutex<VecDeque<u8>>>),
    /// The bytes, held back until the gate opens.
    Gated {
        gate: Arc<Gate>,
        bytes: Arc<Mutex<VecDeque<u8>>>,
    },
    /// An input that fails at its first read.
    Broken,
}

impl Stdin {
    fn ready(bytes: &[u8]) -> Self {
        Stdin::Ready(Arc::new(Mutex::new(bytes.iter().copied().collect())))
    }
}

impl tokio::io::AsyncRead for Stdin {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let bytes = match &*self {
            Stdin::Broken => return Poll::Ready(Err(std::io::Error::other("the input broke"))),
            Stdin::Gated { gate, .. } if !gate.passes(cx) => return Poll::Pending,
            Stdin::Ready(bytes) | Stdin::Gated { bytes, .. } => bytes,
        };
        let mut bytes = bytes.lock().unwrap();
        let count = bytes.len().min(buf.remaining());
        let taken: Vec<u8> = bytes.drain(..count).collect();
        buf.put_slice(&taken);
        Poll::Ready(Ok(()))
    }
}

impl IsTerminal for Stdin {
    fn is_terminal(&self) -> bool {
        false
    }
}

impl StdinStream for Stdin {
    fn async_stream(&self) -> Box<dyn tokio::io::AsyncRead + Send + Sync> {
        Box::new(self.clone())
    }
}

/// `source` built alone, as a component on a host with `stdin`.
fn load(db: &mut db::RootDatabase, source: &str, stdin: Stdin) -> Plc {
    let (mir, core) = compile_to_mir_and_wasm_as_built(db, source);
    let interfaces = rk::component::interfaces(db, &mir).expect("typed interfaces");
    let component = rk::component::wrap(&core, "plant", &interfaces).expect("the module wraps");
    Plc::load(&component.bytes, WasiCtxBuilder::new().stdin(stdin).build())
}

/// Scans until the task's output is `until`, or `scans` have run: the last
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

/// A STRING takes each chunk as it comes, its length the bytes that came,
/// up to what it holds: the text comes in pieces of 4, the stream then
/// ends, and its future says it ended well.
#[rstest]
fn a_string_takes_a_stream_chunk_by_chunk(mut with_db: db::RootDatabase) {
    let mut plc = load(
        &mut with_db,
        &text_reader(false),
        Stdin::ready(b"hello, rk"),
    );
    let done = verdict(3, 3, false);
    assert_eq!(scan_until(&mut plc, done, 20), done);
}

/// An ARRAY OF BYTE takes bytes as they are, zero and the high ones
/// included, one an element, and `read` says how many came.
#[rstest]
fn an_array_of_bytes_takes_each_byte_in_its_element(mut with_db: db::RootDatabase) {
    let source = reader(
        "ARRAY[0..3] OF BYTE",
        r#"
                    IF chunks = 0 AND chunk.read = 4 AND chunk.data[0] = BYTE#16#00
                        AND chunk.data[1] = BYTE#16#FF AND chunk.data[2] = BYTE#16#41
                        AND chunk.data[3] = BYTE#16#7F THEN
                        expected := expected + 1;
                    END_IF;
                    IF chunks = 1 AND chunk.read = 1 AND chunk.data[0] = BYTE#16#80 THEN
                        expected := expected + 1;
                    END_IF;"#,
        false,
    );
    let mut plc = load(
        &mut with_db,
        &source,
        Stdin::ready(&[0x00, 0xFF, 0x41, 0x7F, 0x80]),
    );
    let done = verdict(2, 2, false);
    assert_eq!(scan_until(&mut plc, done, 20), done);
}

/// A read the host cannot answer yet stays pending, scan after scan, and
/// ends at the scan after the host answers: the block checks the stream's
/// end, and no scan waits for it.
#[rstest]
fn a_read_the_host_holds_back_stays_pending(mut with_db: db::RootDatabase) {
    let gate = Arc::new(Gate::default());
    let mut plc = load(&mut with_db, &text_reader(false), held_text(&gate));
    let held: Vec<i32> = (0..3).map(|_| plc.scan(0)).collect();
    assert_eq!(held, [0, 0, 0], "no chunk came while the host held it back");
    gate.open();
    let done = verdict(3, 3, false);
    assert_eq!(scan_until(&mut plc, done, 20), done);
}

/// A future read stays pending while its future has no value, and ends at
/// a scan after it has one: the task reads how its input ends from its
/// first scan, while the host holds the input back, and the future has its
/// value once the stream ended.
#[rstest]
fn a_future_read_stays_pending_until_the_future_has_its_value(mut with_db: db::RootDatabase) {
    let gate = Arc::new(Gate::default());
    let mut plc = load(&mut with_db, &text_reader(true), held_text(&gate));
    let held: Vec<i32> = (0..3).map(|_| plc.scan(0)).collect();
    assert_eq!(
        held,
        [0, 0, 0],
        "neither read ended while the host held the input back"
    );
    gate.open();
    let done = verdict(3, 3, false);
    assert_eq!(scan_until(&mut plc, done, 20), done);
}

/// An input that breaks ends the stream, and its future says how: the
/// future's value comes back in the block's outputs.
#[rstest]
fn a_broken_input_ends_with_a_failure(mut with_db: db::RootDatabase) {
    let mut plc = load(&mut with_db, &text_reader(false), Stdin::Broken);
    let done = verdict(0, 0, true);
    assert_eq!(scan_until(&mut plc, done, 20), done);
}
