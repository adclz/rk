// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! WASI's own interfaces, from the WIT rk carries: every interface of every
//! package generates, rk takes the files, each import has the canonical
//! ABI's core type, and they build into a component, whose build checks
//! each import against WASI's WIT. What each file declares, and leaves out,
//! is the snapshot of what the generator covers. Std's interfaces likewise.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::{assert_abi, built, generated, summary, wrapped};
use crate::tests::utils::with_db;

/// A world that imports every interface of every WASI package rk carries,
/// in the order their files declare them.
fn every_interface() -> String {
    let mut imports = String::new();
    for package in debug_format::wit::WASI {
        let (name, version) = package.id.split_once('@').expect("a version");
        for (_, text) in package.files {
            for line in text.lines() {
                if let Some(rest) = line.trim().strip_prefix("interface ") {
                    let interface = rest.trim_end_matches('{').trim();
                    imports.push_str(&format!("    import {name}/{interface}@{version};\n"));
                }
            }
        }
    }
    format!("package rk:every-wasi;\n\nworld every {{\n{imports}}}\n")
}

/// Every interface of the WASI rk carries builds into a component.
#[rstest]
fn every_wasi_interface_builds_into_a_component(mut with_db: RootDatabase) {
    let bindings = generated(&every_interface());
    let (mir, core) = built(&mut with_db, &bindings.files, &[]);
    assert_abi(&bindings, &mir, &core);
    let component = wrapped(&with_db, &mir, &core);
    for package in debug_format::wit::WASI {
        let (name, _) = package.id.split_once('@').expect("a version");
        assert!(
            component.world.contains(&format!("    import {name}/")),
            "{name} in {}",
            component.world
        );
    }
}

/// What every WASI interface declares, and what it leaves out.
#[test]
fn what_every_wasi_interface_declares() {
    assert_snapshot!(summary(&generated(&every_interface()).files), @r"
    wasi-cli-environment.st: 
        left out `get-environment`: its result holds a list, which an import cannot give back yet
        left out `get-arguments`: its result holds a list, which an import cannot give back yet
        left out `get-initial-cwd`: its result holds a string, which an import cannot give back yet
    wasi-cli-exit.st: EXIT_WITH_CODE
        left out `exit`: its parameter `status` holds a result, which an import cannot take yet
    wasi-cli-run.st: RUN
    wasi-cli-types.st: ERROR_CODE
    wasi-cli-stdin.st: READ_VIA_STREAM, BYTES_STREAM_NEW, BYTES_STREAM_READ, BYTES_STREAM_READ_ARRAY, BYTES_STREAM_WRITE, BYTES_STREAM_DROP_READABLE, BYTES_STREAM_DROP_WRITABLE, RESULT_FUTURE_NEW, RESULT_FUTURE_READ, RESULT_FUTURE_DROP_READABLE, RESULT_FUTURE_DROP_WRITABLE
        left out `BYTES_STREAM_CANCEL_READ` and `BYTES_STREAM_CANCEL_WRITE`: a cancel blocks the scan
        left out `RESULT_FUTURE_WRITE`: a future's write is not imported yet
        left out `RESULT_FUTURE_CANCEL_READ` and `RESULT_FUTURE_CANCEL_WRITE`: a cancel blocks the scan
    wasi-cli-stdout.st: WRITE_VIA_STREAM, BYTES_STREAM_NEW, BYTES_STREAM_READ, BYTES_STREAM_READ_ARRAY, BYTES_STREAM_WRITE, BYTES_STREAM_DROP_READABLE, BYTES_STREAM_DROP_WRITABLE, RESULT_FUTURE_NEW, RESULT_FUTURE_READ, RESULT_FUTURE_DROP_READABLE, RESULT_FUTURE_DROP_WRITABLE
        left out `BYTES_STREAM_CANCEL_READ` and `BYTES_STREAM_CANCEL_WRITE`: a cancel blocks the scan
        left out `RESULT_FUTURE_WRITE`: a future's write is not imported yet
        left out `RESULT_FUTURE_CANCEL_READ` and `RESULT_FUTURE_CANCEL_WRITE`: a cancel blocks the scan
    wasi-cli-stderr.st: WRITE_VIA_STREAM, BYTES_STREAM_NEW, BYTES_STREAM_READ, BYTES_STREAM_READ_ARRAY, BYTES_STREAM_WRITE, BYTES_STREAM_DROP_READABLE, BYTES_STREAM_DROP_WRITABLE, RESULT_FUTURE_NEW, RESULT_FUTURE_READ, RESULT_FUTURE_DROP_READABLE, RESULT_FUTURE_DROP_WRITABLE
        left out `BYTES_STREAM_CANCEL_READ` and `BYTES_STREAM_CANCEL_WRITE`: a cancel blocks the scan
        left out `RESULT_FUTURE_WRITE`: a future's write is not imported yet
        left out `RESULT_FUTURE_CANCEL_READ` and `RESULT_FUTURE_CANCEL_WRITE`: a cancel blocks the scan
    wasi-cli-terminal-input.st: TERMINAL_INPUT_DROP
    wasi-cli-terminal-output.st: TERMINAL_OUTPUT_DROP
    wasi-cli-terminal-stdin.st: GET_TERMINAL_STDIN
    wasi-cli-terminal-stdout.st: GET_TERMINAL_STDOUT
    wasi-cli-terminal-stderr.st: GET_TERMINAL_STDERR
    wasi-clocks-types.st: DURATION
    wasi-clocks-monotonic-clock.st: MARK, NOW, GET_RESOLUTION, WAIT_UNTIL, WAIT_FOR
    wasi-clocks-system-clock.st: NOW, GET_RESOLUTION
    wasi-clocks-timezone.st: UTC_OFFSET
        left out `iana-id`: its result holds a string, which an import cannot give back yet
        left out `to-debug-string`: its result holds a string, which an import cannot give back yet
    wasi-filesystem-types.st: FILESIZE, DESCRIPTOR_TYPE, DESCRIPTOR_FLAGS, PATH_FLAGS, OPEN_FLAGS, LINK_COUNT, NEW_TIMESTAMP, ERROR_CODE, ADVICE, DESCRIPTOR_DROP, DESCRIPTOR_READ_VIA_STREAM, BYTES_STREAM_NEW, BYTES_STREAM_READ, BYTES_STREAM_READ_ARRAY, BYTES_STREAM_WRITE, BYTES_STREAM_DROP_READABLE, BYTES_STREAM_DROP_WRITABLE, RESULT_FUTURE_NEW, RESULT_FUTURE_READ_RAW, RESULT_FUTURE_READ, RESULT_FUTURE_DROP_READABLE, RESULT_FUTURE_DROP_WRITABLE, DESCRIPTOR_WRITE_VIA_STREAM, DESCRIPTOR_APPEND_VIA_STREAM, DESCRIPTOR_ADVISE_RAW, DESCRIPTOR_ADVISE, DESCRIPTOR_SYNC_DATA_RAW, DESCRIPTOR_SYNC_DATA, DESCRIPTOR_GET_FLAGS_RAW, DESCRIPTOR_GET_FLAGS, DESCRIPTOR_GET_TYPE_RAW, DESCRIPTOR_GET_TYPE, DESCRIPTOR_SET_SIZE_RAW, DESCRIPTOR_SET_SIZE, DESCRIPTOR_READ_DIRECTORY, DIRECTORY_ENTRY_STREAM_NEW, DIRECTORY_ENTRY_STREAM_DROP_READABLE, DIRECTORY_ENTRY_STREAM_DROP_WRITABLE, DESCRIPTOR_SYNC_RAW, DESCRIPTOR_SYNC, DESCRIPTOR_CREATE_DIRECTORY_AT_RAW, DESCRIPTOR_CREATE_DIRECTORY_AT, DESCRIPTOR_STAT_RAW, DESCRIPTOR_STAT, DESCRIPTOR_STAT_AT_RAW, DESCRIPTOR_STAT_AT, DESCRIPTOR_LINK_AT_RAW, DESCRIPTOR_LINK_AT, DESCRIPTOR_OPEN_AT_RAW, DESCRIPTOR_OPEN_AT, DESCRIPTOR_REMOVE_DIRECTORY_AT_RAW, DESCRIPTOR_REMOVE_DIRECTORY_AT, DESCRIPTOR_RENAME_AT_RAW, DESCRIPTOR_RENAME_AT, DESCRIPTOR_SYMLINK_AT_RAW, DESCRIPTOR_SYMLINK_AT, DESCRIPTOR_UNLINK_FILE_AT_RAW, DESCRIPTOR_UNLINK_FILE_AT, DESCRIPTOR_IS_SAME_OBJECT, DESCRIPTOR_METADATA_HASH_RAW, DESCRIPTOR_METADATA_HASH, DESCRIPTOR_METADATA_HASH_AT_RAW, DESCRIPTOR_METADATA_HASH_AT
        left out what `other` of `descriptor-type` holds: a string, which an import cannot give back yet
        left out what `other` of `error-code` holds: a string, which an import cannot give back yet
        left out `BYTES_STREAM_CANCEL_READ` and `BYTES_STREAM_CANCEL_WRITE`: a cancel blocks the scan
        left out `RESULT_FUTURE_WRITE`: a future's write is not imported yet
        left out `RESULT_FUTURE_CANCEL_READ` and `RESULT_FUTURE_CANCEL_WRITE`: a cancel blocks the scan
        left out `[method]descriptor.set-times`: its parameter `data-access-timestamp` holds a variant, which an import cannot take yet
        left out `DIRECTORY_ENTRY_STREAM_READ` and `DIRECTORY_ENTRY_STREAM_WRITE`: a stream's read or write moves bytes only
        left out `DIRECTORY_ENTRY_STREAM_CANCEL_READ` and `DIRECTORY_ENTRY_STREAM_CANCEL_WRITE`: a cancel blocks the scan
        left out `[method]descriptor.set-times-at`: its parameter `data-access-timestamp` holds a variant, which an import cannot take yet
        left out `[method]descriptor.readlink-at`: its result holds a string, which an import cannot give back yet
    wasi-filesystem-preopens.st: 
        left out `get-directories`: its result holds a list, which an import cannot give back yet
    wasi-random-insecure-seed.st: GET_INSECURE_SEED
    wasi-random-insecure.st: GET_INSECURE_RANDOM_U64
        left out `get-insecure-random-bytes`: its result holds a list, which an import cannot give back yet
    wasi-random-random.st: GET_RANDOM_U64
        left out `get-random-bytes`: its result holds a list, which an import cannot give back yet
    wasi-sockets-types.st: ERROR_CODE, IP_ADDRESS_FAMILY, IP_ADDRESS, IP_SOCKET_ADDRESS, TCP_SOCKET_DROP, UDP_SOCKET_DROP, TCP_SOCKET_CREATE_RAW, TCP_SOCKET_CREATE, TCP_SOCKET_LISTEN_RAW, TCP_SOCKET_LISTEN, TCP_SOCKET_STREAM_NEW, TCP_SOCKET_STREAM_DROP_READABLE, TCP_SOCKET_STREAM_DROP_WRITABLE, TCP_SOCKET_SEND, BYTES_STREAM_NEW, BYTES_STREAM_READ, BYTES_STREAM_READ_ARRAY, BYTES_STREAM_WRITE, BYTES_STREAM_DROP_READABLE, BYTES_STREAM_DROP_WRITABLE, RESULT_FUTURE_NEW, RESULT_FUTURE_READ_RAW, RESULT_FUTURE_READ, RESULT_FUTURE_DROP_READABLE, RESULT_FUTURE_DROP_WRITABLE, TCP_SOCKET_RECEIVE, TCP_SOCKET_GET_LOCAL_ADDRESS_RAW, TCP_SOCKET_GET_LOCAL_ADDRESS, TCP_SOCKET_GET_REMOTE_ADDRESS_RAW, TCP_SOCKET_GET_REMOTE_ADDRESS, TCP_SOCKET_GET_IS_LISTENING, TCP_SOCKET_GET_ADDRESS_FAMILY, TCP_SOCKET_SET_LISTEN_BACKLOG_SIZE_RAW, TCP_SOCKET_SET_LISTEN_BACKLOG_SIZE, TCP_SOCKET_GET_KEEP_ALIVE_ENABLED_RAW, TCP_SOCKET_GET_KEEP_ALIVE_ENABLED, TCP_SOCKET_SET_KEEP_ALIVE_ENABLED_RAW, TCP_SOCKET_SET_KEEP_ALIVE_ENABLED, TCP_SOCKET_GET_KEEP_ALIVE_IDLE_TIME_RAW, TCP_SOCKET_GET_KEEP_ALIVE_IDLE_TIME, TCP_SOCKET_SET_KEEP_ALIVE_IDLE_TIME_RAW, TCP_SOCKET_SET_KEEP_ALIVE_IDLE_TIME, TCP_SOCKET_GET_KEEP_ALIVE_INTERVAL_RAW, TCP_SOCKET_GET_KEEP_ALIVE_INTERVAL, TCP_SOCKET_SET_KEEP_ALIVE_INTERVAL_RAW, TCP_SOCKET_SET_KEEP_ALIVE_INTERVAL, TCP_SOCKET_GET_KEEP_ALIVE_COUNT_RAW, TCP_SOCKET_GET_KEEP_ALIVE_COUNT, TCP_SOCKET_SET_KEEP_ALIVE_COUNT_RAW, TCP_SOCKET_SET_KEEP_ALIVE_COUNT, TCP_SOCKET_GET_HOP_LIMIT_RAW, TCP_SOCKET_GET_HOP_LIMIT, TCP_SOCKET_SET_HOP_LIMIT_RAW, TCP_SOCKET_SET_HOP_LIMIT, TCP_SOCKET_GET_RECEIVE_BUFFER_SIZE_RAW, TCP_SOCKET_GET_RECEIVE_BUFFER_SIZE, TCP_SOCKET_SET_RECEIVE_BUFFER_SIZE_RAW, TCP_SOCKET_SET_RECEIVE_BUFFER_SIZE, TCP_SOCKET_GET_SEND_BUFFER_SIZE_RAW, TCP_SOCKET_GET_SEND_BUFFER_SIZE, TCP_SOCKET_SET_SEND_BUFFER_SIZE_RAW, TCP_SOCKET_SET_SEND_BUFFER_SIZE, UDP_SOCKET_CREATE_RAW, UDP_SOCKET_CREATE, UDP_SOCKET_DISCONNECT_RAW, UDP_SOCKET_DISCONNECT, UDP_SOCKET_GET_LOCAL_ADDRESS_RAW, UDP_SOCKET_GET_LOCAL_ADDRESS, UDP_SOCKET_GET_REMOTE_ADDRESS_RAW, UDP_SOCKET_GET_REMOTE_ADDRESS, UDP_SOCKET_GET_ADDRESS_FAMILY, UDP_SOCKET_GET_UNICAST_HOP_LIMIT_RAW, UDP_SOCKET_GET_UNICAST_HOP_LIMIT, UDP_SOCKET_SET_UNICAST_HOP_LIMIT_RAW, UDP_SOCKET_SET_UNICAST_HOP_LIMIT, UDP_SOCKET_GET_RECEIVE_BUFFER_SIZE_RAW, UDP_SOCKET_GET_RECEIVE_BUFFER_SIZE, UDP_SOCKET_SET_RECEIVE_BUFFER_SIZE_RAW, UDP_SOCKET_SET_RECEIVE_BUFFER_SIZE, UDP_SOCKET_GET_SEND_BUFFER_SIZE_RAW, UDP_SOCKET_GET_SEND_BUFFER_SIZE, UDP_SOCKET_SET_SEND_BUFFER_SIZE_RAW, UDP_SOCKET_SET_SEND_BUFFER_SIZE
        left out what `other` of `error-code` holds: a string, which an import cannot give back yet
        left out `[method]tcp-socket.bind`: its parameter `local-address` holds a variant, which an import cannot take yet
        left out `[method]tcp-socket.connect`: its parameter `remote-address` holds a variant, which an import cannot take yet
        left out `TCP_SOCKET_STREAM_READ` and `TCP_SOCKET_STREAM_WRITE`: a stream's read or write moves bytes only
        left out `TCP_SOCKET_STREAM_CANCEL_READ` and `TCP_SOCKET_STREAM_CANCEL_WRITE`: a cancel blocks the scan
        left out `BYTES_STREAM_CANCEL_READ` and `BYTES_STREAM_CANCEL_WRITE`: a cancel blocks the scan
        left out `RESULT_FUTURE_WRITE`: a future's write is not imported yet
        left out `RESULT_FUTURE_CANCEL_READ` and `RESULT_FUTURE_CANCEL_WRITE`: a cancel blocks the scan
        left out `[method]udp-socket.bind`: its parameter `local-address` holds a variant, which an import cannot take yet
        left out `[method]udp-socket.connect`: its parameter `remote-address` holds a variant, which an import cannot take yet
        left out `[method]udp-socket.send`: its parameter `data` holds a list, which an import cannot take yet
        left out `[method]udp-socket.receive`: its result holds a list, which an import cannot give back yet
    wasi-sockets-ip-name-lookup.st: ERROR_CODE
        left out what `other` of `error-code` holds: a string, which an import cannot give back yet
        left out `resolve-addresses`: its result holds a list, which an import cannot give back yet
    ");
}

/// What the interfaces `Std` reaches WASI through declare.
#[test]
fn std_s_interfaces() {
    assert_snapshot!(summary(&generated(rk_bindgen::STD_WORLD).files), @r"
    wasi-cli-types.st: ERROR_CODE
    wasi-cli-stdout.st: WRITE_VIA_STREAM, BYTES_STREAM_NEW, BYTES_STREAM_READ, BYTES_STREAM_READ_ARRAY, BYTES_STREAM_WRITE, BYTES_STREAM_DROP_READABLE, BYTES_STREAM_DROP_WRITABLE, RESULT_FUTURE_NEW, RESULT_FUTURE_READ, RESULT_FUTURE_DROP_READABLE, RESULT_FUTURE_DROP_WRITABLE
        left out `BYTES_STREAM_CANCEL_READ` and `BYTES_STREAM_CANCEL_WRITE`: a cancel blocks the scan
        left out `RESULT_FUTURE_WRITE`: a future's write is not imported yet
        left out `RESULT_FUTURE_CANCEL_READ` and `RESULT_FUTURE_CANCEL_WRITE`: a cancel blocks the scan
    wasi-cli-stderr.st: WRITE_VIA_STREAM, BYTES_STREAM_NEW, BYTES_STREAM_READ, BYTES_STREAM_READ_ARRAY, BYTES_STREAM_WRITE, BYTES_STREAM_DROP_READABLE, BYTES_STREAM_DROP_WRITABLE, RESULT_FUTURE_NEW, RESULT_FUTURE_READ, RESULT_FUTURE_DROP_READABLE, RESULT_FUTURE_DROP_WRITABLE
        left out `BYTES_STREAM_CANCEL_READ` and `BYTES_STREAM_CANCEL_WRITE`: a cancel blocks the scan
        left out `RESULT_FUTURE_WRITE`: a future's write is not imported yet
        left out `RESULT_FUTURE_CANCEL_READ` and `RESULT_FUTURE_CANCEL_WRITE`: a cancel blocks the scan
    wasi-clocks-types.st: DURATION
    wasi-clocks-monotonic-clock.st: MARK, NOW, GET_RESOLUTION, WAIT_UNTIL, WAIT_FOR
    wasi-clocks-system-clock.st: NOW, GET_RESOLUTION
    wasi-clocks-timezone.st: UTC_OFFSET
        left out `iana-id`: its result holds a string, which an import cannot give back yet
        left out `to-debug-string`: its result holds a string, which an import cannot give back yet
    ");
}

/// Std's interfaces build into a component, which imports each of them.
#[rstest]
fn std_s_interfaces_build_into_a_component(mut with_db: RootDatabase) {
    let bindings = generated(rk_bindgen::STD_WORLD);
    let (mir, core) = built(&mut with_db, &bindings.files, &[]);
    assert_abi(&bindings, &mir, &core);
    let component = wrapped(&with_db, &mir, &core);
    for interface in [
        "wasi:cli/stdout@0.3.0",
        "wasi:cli/stderr@0.3.0",
        "wasi:clocks/monotonic-clock@0.3.0",
        "wasi:clocks/system-clock@0.3.0",
        "wasi:clocks/timezone@0.3.0",
    ] {
        assert!(
            component
                .world
                .contains(&format!("    import {interface};\n")),
            "{interface} in {}",
            component.world
        );
    }
}

/// `stdlib/generated/` is what rk-bindgen writes for Std's world, file for
/// file, and holds nothing else: after a change to the generator or to its
/// world, `cargo run -p rk-bindgen` writes it again. A checkout's line
/// endings do not count.
#[test]
fn std_s_generated_folder_is_what_rk_bindgen_writes() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("stdlib/generated");
    let files = generated(rk_bindgen::STD_WORLD).files;
    let mut on_disk: Vec<String> = std::fs::read_dir(&folder)
        .expect("stdlib/generated")
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            name.ends_with(".st").then_some(name)
        })
        .collect();
    on_disk.sort();
    let mut written: Vec<String> = files.iter().map(|file| file.name.clone()).collect();
    written.sort();
    assert_eq!(on_disk, written, "run `cargo run -p rk-bindgen`");
    for file in &files {
        let text = std::fs::read_to_string(folder.join(&file.name))
            .expect("a generated file")
            .replace("\r\n", "\n");
        assert!(
            text == file.text,
            "`stdlib/generated/{}` is not what rk-bindgen writes: run `cargo run -p rk-bindgen`",
            file.name
        );
    }
}

/// A generated file says where it comes from, and that it is not edited.
#[test]
fn a_file_says_it_is_generated() {
    let bindings = generated(rk_bindgen::STD_WORLD);
    let header: Vec<&str> = bindings.files[0].text.lines().take(2).collect();
    assert_snapshot!(header.join("\n"), @r"
    /* Generated from the WIT of wasi:cli@0.3.0. DO NOT EDIT! */
    NAMESPACE INTERNAL Wasi
    ");
}
