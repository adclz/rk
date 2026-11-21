// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! WASI's own interfaces, as WASI's repository writes them: every interface
//! of every package generates, rk takes the files, each import has the
//! canonical ABI's core type, and they build into a component, whose build
//! checks each import against WASI's WIT. What each file declares, and
//! leaves out, is the snapshot of what the generator covers. Std's
//! interfaces likewise, from Std's own WIT folder, whose files are WASI's
//! own.

use std::collections::BTreeMap;
use std::path::Path;

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::{assert_abi, built, generated, generated_std, summary, wrapped, wrapped_with};
use crate::tests::utils::{stdlib_wit, wasi_packages, wasi_repository, with_db};

/// The `.wit` files of the folder `dir`, by name, a checkout's line
/// endings left out.
fn wit_files(dir: &Path) -> BTreeMap<String, String> {
    std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let name = path.file_name()?.to_str()?.to_string();
            name.ends_with(".wit").then(|| {
                let text = std::fs::read_to_string(&path).expect("a WIT file");
                (name, text.replace("\r\n", "\n"))
            })
        })
        .collect()
}

/// The name and version of the package `files` write, as they declare it.
fn declared(files: &BTreeMap<String, String>) -> String {
    files
        .values()
        .flat_map(|text| text.lines())
        .find_map(|line| line.trim().strip_prefix("package ")?.strip_suffix(';'))
        .expect("a package's name")
        .to_string()
}

/// WASI's packages, as WASI's repository writes them: each one's name and
/// version, as its files declare them, and its files.
fn wasi() -> Vec<(String, BTreeMap<String, String>)> {
    wasi_packages()
        .iter()
        .map(|folder| {
            let files = wit_files(folder);
            (declared(&files), files)
        })
        .collect()
}

/// A world that imports every interface of every WASI package, in the
/// order their files declare them.
fn every_interface() -> String {
    let mut imports = String::new();
    for (id, files) in wasi() {
        let (name, version) = id.split_once('@').expect("a version");
        for text in files.values() {
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

/// Std ships the files of WASI's packages that declare the interfaces its
/// world imports, and no other, as WASI's repository, the git submodule at
/// `wasi/`, writes them at the release Std imports,
/// each package with WASI's license; and `package.wit`, which names the
/// command line's package, as WASI does only in the files that take the
/// rest of WASI in. After the submodule moves to another release, the
/// files are copied from it again.
#[test]
fn std_ships_wasi_s_own_files() {
    let wasi = wasi_repository();
    let license = std::fs::read_to_string(wasi.join("LICENSE.md"))
        .expect("WASI's license")
        .replace("\r\n", "\n");
    let deps = stdlib_wit().join("deps");
    let names = |dir: &Path| {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
            .map(|entry| {
                entry
                    .expect("an entry")
                    .file_name()
                    .into_string()
                    .expect("a name")
            })
            .collect();
        names.sort();
        names
    };
    let shipped: Vec<(String, Vec<String>)> = names(&deps)
        .into_iter()
        .map(|folder| {
            let files = names(&deps.join(&folder));
            (folder, files)
        })
        .collect();
    assert_eq!(
        shipped,
        [
            ("wasi-cli", &["LICENSE.md", "package.wit", "stdio.wit"][..]),
            (
                "wasi-clocks",
                &[
                    "LICENSE.md",
                    "monotonic-clock.wit",
                    "system-clock.wit",
                    "timezone.wit",
                    "types.wit"
                ]
            ),
        ]
        .map(|(folder, files)| (
            folder.to_string(),
            files
                .iter()
                .map(|file| file.to_string())
                .collect::<Vec<_>>()
        ))
    );
    for (folder, _) in &shipped {
        let proposal = folder.strip_prefix("wasi-").expect("a WASI package");
        let published = wasi.join("proposals").join(proposal).join("wit");
        let written = wit_files(&published);
        for (name, text) in wit_files(&deps.join(folder)) {
            if name == "package.wit" {
                assert_eq!(text, format!("package {};\n", declared(&written)));
                continue;
            }
            assert!(
                written.get(&name) == Some(&text),
                "stdlib/wit/deps/{folder}/{name} is not WASI's: copy {} into it",
                published.join(&name).display()
            );
        }
        let shipped_license = std::fs::read_to_string(deps.join(folder).join("LICENSE.md"))
            .expect("WASI's license beside its files")
            .replace("\r\n", "\n");
        assert!(
            shipped_license == license,
            "stdlib/wit/deps/{folder}/LICENSE.md is not WASI's"
        );
    }
}

/// Every interface of WASI's packages builds into a component.
#[rstest]
fn every_wasi_interface_builds_into_a_component(mut with_db: RootDatabase) {
    let bindings = generated(&every_interface());
    let (mir, core) = built(&mut with_db, &bindings.files, &[]);
    assert_abi(&bindings, &mir, &core);
    let component = wrapped(&with_db, &mir, &core);
    for (id, _) in wasi() {
        let (name, _) = id.split_once('@').expect("a version");
        assert!(
            component.world.contains(&format!("  import {name}/")),
            "{name} in {}",
            component.world
        );
    }
}

/// What every WASI interface declares, and what it leaves out.
#[test]
fn what_every_wasi_interface_declares() {
    assert_snapshot!(summary(&generated(&every_interface()).files), @r"
    wasi-cli-environment.st: GET_ENVIRONMENT_RAW, GET_ENVIRONMENT, GET_ARGUMENTS_RAW, GET_ARGUMENTS, GET_INITIAL_CWD_RAW, GET_INITIAL_CWD
    wasi-cli-exit.st: EXIT__RAW, EXIT_, EXIT_WITH_CODE
    wasi-cli-run.st: RUN
    wasi-cli-types.st: ERROR_CODE
    wasi-cli-stdin.st: READ_VIA_STREAM, BYTES_STREAM_READ, BYTES_STREAM_READ_ARRAY, BYTES_STREAM_DROP_READABLE, RESULT_FUTURE_READ, RESULT_FUTURE_DROP_READABLE, RESULT_FUTURE_GET
        left out `BYTES_STREAM_CANCEL_READ`: a cancel blocks the scan
        left out `RESULT_FUTURE_CANCEL_READ`: a cancel blocks the scan
    wasi-cli-stdout.st: WRITE_VIA_STREAM, BYTES_STREAM_NEW, BYTES_STREAM_WRITE, BYTES_STREAM_WRITE_ARRAY, BYTES_STREAM_DROP_READABLE, BYTES_STREAM_DROP_WRITABLE, BYTES_STREAM_TEXT_LEN, BYTES_STREAM_TEXT_AFTER, BYTES_STREAM_WRITER, RESULT_FUTURE_READ, RESULT_FUTURE_DROP_READABLE, RESULT_FUTURE_GET, WRITE_VIA_STREAM_WRITER
        left out `BYTES_STREAM_CANCEL_WRITE`: a cancel blocks the scan
        left out `RESULT_FUTURE_CANCEL_READ`: a cancel blocks the scan
    wasi-cli-stderr.st: WRITE_VIA_STREAM, BYTES_STREAM_NEW, BYTES_STREAM_WRITE, BYTES_STREAM_WRITE_ARRAY, BYTES_STREAM_DROP_READABLE, BYTES_STREAM_DROP_WRITABLE, BYTES_STREAM_TEXT_LEN, BYTES_STREAM_TEXT_AFTER, BYTES_STREAM_WRITER, RESULT_FUTURE_READ, RESULT_FUTURE_DROP_READABLE, RESULT_FUTURE_GET, WRITE_VIA_STREAM_WRITER
        left out `BYTES_STREAM_CANCEL_WRITE`: a cancel blocks the scan
        left out `RESULT_FUTURE_CANCEL_READ`: a cancel blocks the scan
    wasi-cli-terminal-input.st: TERMINAL_INPUT, TERMINAL_INPUT_DROP
    wasi-cli-terminal-output.st: TERMINAL_OUTPUT, TERMINAL_OUTPUT_DROP
    wasi-cli-terminal-stdin.st: GET_TERMINAL_STDIN
    wasi-cli-terminal-stdout.st: GET_TERMINAL_STDOUT
    wasi-cli-terminal-stderr.st: GET_TERMINAL_STDERR
    wasi-clocks-types.st: DURATION
    wasi-clocks-monotonic-clock.st: MARK, NOW, GET_RESOLUTION, WAIT_UNTIL, WAIT_FOR
    wasi-clocks-system-clock.st: INSTANT, NOW_RAW, NOW, GET_RESOLUTION
    wasi-clocks-timezone.st: IANA_ID_RAW, IANA_ID, UTC_OFFSET_RAW, UTC_OFFSET, TO_DEBUG_STRING_RAW, TO_DEBUG_STRING
    wasi-filesystem-types.st: FILESIZE, DESCRIPTOR_TYPE_KIND, DESCRIPTOR_TYPE, DESCRIPTOR_FLAGS, PATH_FLAGS, OPEN_FLAGS, LINK_COUNT, DESCRIPTOR_STAT, NEW_TIMESTAMP_KIND, NEW_TIMESTAMP, DIRECTORY_ENTRY, ERROR_CODE_KIND, ERROR_CODE, ADVICE, METADATA_HASH_VALUE, DESCRIPTOR, DESCRIPTOR_DROP, DESCRIPTOR_READ_VIA_STREAM, BYTES_STREAM_NEW, BYTES_STREAM_READ, BYTES_STREAM_READ_ARRAY, BYTES_STREAM_WRITE, BYTES_STREAM_WRITE_ARRAY, BYTES_STREAM_DROP_READABLE, BYTES_STREAM_DROP_WRITABLE, BYTES_STREAM_TEXT_LEN, BYTES_STREAM_TEXT_AFTER, BYTES_STREAM_WRITER, RESULT_FUTURE_READ_RAW, RESULT_FUTURE_READ, RESULT_FUTURE_DROP_READABLE, RESULT_FUTURE_GET, DESCRIPTOR_WRITE_VIA_STREAM, DESCRIPTOR_APPEND_VIA_STREAM, DESCRIPTOR_ADVISE_RAW, DESCRIPTOR_ADVISE, DESCRIPTOR_SYNC_DATA_RAW, DESCRIPTOR_SYNC_DATA, DESCRIPTOR_GET_FLAGS_RAW, DESCRIPTOR_GET_FLAGS, DESCRIPTOR_GET_TYPE_RAW, DESCRIPTOR_GET_TYPE, DESCRIPTOR_SET_SIZE_RAW, DESCRIPTOR_SET_SIZE, DESCRIPTOR_SET_TIMES_RAW, DESCRIPTOR_SET_TIMES, DESCRIPTOR_READ_DIRECTORY, DIRECTORY_ENTRY_STREAM_DROP_READABLE, DESCRIPTOR_SYNC_RAW, DESCRIPTOR_SYNC, DESCRIPTOR_CREATE_DIRECTORY_AT_RAW, DESCRIPTOR_CREATE_DIRECTORY_AT, DESCRIPTOR_STAT__RAW, DESCRIPTOR_STAT_, DESCRIPTOR_STAT_AT_RAW, DESCRIPTOR_STAT_AT, DESCRIPTOR_SET_TIMES_AT_RAW, DESCRIPTOR_SET_TIMES_AT, DESCRIPTOR_LINK_AT_RAW, DESCRIPTOR_LINK_AT, DESCRIPTOR_OPEN_AT_RAW, DESCRIPTOR_OPEN_AT, DESCRIPTOR_REMOVE_DIRECTORY_AT_RAW, DESCRIPTOR_REMOVE_DIRECTORY_AT, DESCRIPTOR_RENAME_AT_RAW, DESCRIPTOR_RENAME_AT, DESCRIPTOR_SYMLINK_AT_RAW, DESCRIPTOR_SYMLINK_AT, DESCRIPTOR_UNLINK_FILE_AT_RAW, DESCRIPTOR_UNLINK_FILE_AT, DESCRIPTOR_IS_SAME_OBJECT, DESCRIPTOR_METADATA_HASH_RAW, DESCRIPTOR_METADATA_HASH, DESCRIPTOR_METADATA_HASH_AT_RAW, DESCRIPTOR_METADATA_HASH_AT, DESCRIPTOR_WRITE_VIA_STREAM_WRITER, DESCRIPTOR_APPEND_VIA_STREAM_WRITER
        left out what `other` of `descriptor-type` holds: a string, which an async call cannot give back yet
        left out what `other` of `error-code` holds: a string, which an async call cannot give back yet
        left out `BYTES_STREAM_CANCEL_READ` and `BYTES_STREAM_CANCEL_WRITE`: a cancel blocks the scan
        left out `RESULT_FUTURE_CANCEL_READ`: a cancel blocks the scan
        left out `DIRECTORY_ENTRY_STREAM_READ`: a stream's read moves bytes or handles only
        left out `DIRECTORY_ENTRY_STREAM_CANCEL_READ`: a cancel blocks the scan
        left out `[method]descriptor.readlink-at`: its result holds a string, which an import cannot give back yet
    wasi-filesystem-preopens.st: GET_DIRECTORIES_RAW, GET_DIRECTORIES
    wasi-random-insecure-seed.st: GET_INSECURE_SEED
    wasi-random-insecure.st: GET_INSECURE_RANDOM_BYTES_RAW, GET_INSECURE_RANDOM_BYTES, GET_INSECURE_RANDOM_U64
    wasi-random-random.st: GET_RANDOM_BYTES_RAW, GET_RANDOM_BYTES, GET_RANDOM_U64
    wasi-sockets-types.st: ERROR_CODE_KIND, ERROR_CODE, IP_ADDRESS_FAMILY, IPV4_ADDRESS, IPV6_ADDRESS, IP_ADDRESS_KIND, IP_ADDRESS, IPV4_SOCKET_ADDRESS, IPV6_SOCKET_ADDRESS, IP_SOCKET_ADDRESS_KIND, IP_SOCKET_ADDRESS, TCP_SOCKET, TCP_SOCKET_DROP, UDP_SOCKET, UDP_SOCKET_DROP, TCP_SOCKET_CREATE_RAW, TCP_SOCKET_CREATE, TCP_SOCKET_BIND_RAW, TCP_SOCKET_BIND, TCP_SOCKET_CONNECT_RAW, TCP_SOCKET_CONNECT, TCP_SOCKET_LISTEN_RAW, TCP_SOCKET_LISTEN, TCP_SOCKET_STREAM_READ, TCP_SOCKET_STREAM_DROP_READABLE, TCP_SOCKET_SEND, BYTES_STREAM_NEW, BYTES_STREAM_READ, BYTES_STREAM_READ_ARRAY, BYTES_STREAM_WRITE, BYTES_STREAM_WRITE_ARRAY, BYTES_STREAM_DROP_READABLE, BYTES_STREAM_DROP_WRITABLE, BYTES_STREAM_TEXT_LEN, BYTES_STREAM_TEXT_AFTER, BYTES_STREAM_WRITER, RESULT_FUTURE_READ_RAW, RESULT_FUTURE_READ, RESULT_FUTURE_DROP_READABLE, RESULT_FUTURE_GET, TCP_SOCKET_RECEIVE, TCP_SOCKET_GET_LOCAL_ADDRESS_RAW, TCP_SOCKET_GET_LOCAL_ADDRESS, TCP_SOCKET_GET_REMOTE_ADDRESS_RAW, TCP_SOCKET_GET_REMOTE_ADDRESS, TCP_SOCKET_GET_IS_LISTENING, TCP_SOCKET_GET_ADDRESS_FAMILY, TCP_SOCKET_SET_LISTEN_BACKLOG_SIZE_RAW, TCP_SOCKET_SET_LISTEN_BACKLOG_SIZE, TCP_SOCKET_GET_KEEP_ALIVE_ENABLED_RAW, TCP_SOCKET_GET_KEEP_ALIVE_ENABLED, TCP_SOCKET_SET_KEEP_ALIVE_ENABLED_RAW, TCP_SOCKET_SET_KEEP_ALIVE_ENABLED, TCP_SOCKET_GET_KEEP_ALIVE_IDLE_TIME_RAW, TCP_SOCKET_GET_KEEP_ALIVE_IDLE_TIME, TCP_SOCKET_SET_KEEP_ALIVE_IDLE_TIME_RAW, TCP_SOCKET_SET_KEEP_ALIVE_IDLE_TIME, TCP_SOCKET_GET_KEEP_ALIVE_INTERVAL_RAW, TCP_SOCKET_GET_KEEP_ALIVE_INTERVAL, TCP_SOCKET_SET_KEEP_ALIVE_INTERVAL_RAW, TCP_SOCKET_SET_KEEP_ALIVE_INTERVAL, TCP_SOCKET_GET_KEEP_ALIVE_COUNT_RAW, TCP_SOCKET_GET_KEEP_ALIVE_COUNT, TCP_SOCKET_SET_KEEP_ALIVE_COUNT_RAW, TCP_SOCKET_SET_KEEP_ALIVE_COUNT, TCP_SOCKET_GET_HOP_LIMIT_RAW, TCP_SOCKET_GET_HOP_LIMIT, TCP_SOCKET_SET_HOP_LIMIT_RAW, TCP_SOCKET_SET_HOP_LIMIT, TCP_SOCKET_GET_RECEIVE_BUFFER_SIZE_RAW, TCP_SOCKET_GET_RECEIVE_BUFFER_SIZE, TCP_SOCKET_SET_RECEIVE_BUFFER_SIZE_RAW, TCP_SOCKET_SET_RECEIVE_BUFFER_SIZE, TCP_SOCKET_GET_SEND_BUFFER_SIZE_RAW, TCP_SOCKET_GET_SEND_BUFFER_SIZE, TCP_SOCKET_SET_SEND_BUFFER_SIZE_RAW, TCP_SOCKET_SET_SEND_BUFFER_SIZE, UDP_SOCKET_CREATE_RAW, UDP_SOCKET_CREATE, UDP_SOCKET_BIND_RAW, UDP_SOCKET_BIND, UDP_SOCKET_CONNECT_RAW, UDP_SOCKET_CONNECT, UDP_SOCKET_DISCONNECT_RAW, UDP_SOCKET_DISCONNECT, UDP_SOCKET_SEND_RAW, UDP_SOCKET_SEND, UDP_SOCKET_GET_LOCAL_ADDRESS_RAW, UDP_SOCKET_GET_LOCAL_ADDRESS, UDP_SOCKET_GET_REMOTE_ADDRESS_RAW, UDP_SOCKET_GET_REMOTE_ADDRESS, UDP_SOCKET_GET_ADDRESS_FAMILY, UDP_SOCKET_GET_UNICAST_HOP_LIMIT_RAW, UDP_SOCKET_GET_UNICAST_HOP_LIMIT, UDP_SOCKET_SET_UNICAST_HOP_LIMIT_RAW, UDP_SOCKET_SET_UNICAST_HOP_LIMIT, UDP_SOCKET_GET_RECEIVE_BUFFER_SIZE_RAW, UDP_SOCKET_GET_RECEIVE_BUFFER_SIZE, UDP_SOCKET_SET_RECEIVE_BUFFER_SIZE_RAW, UDP_SOCKET_SET_RECEIVE_BUFFER_SIZE, UDP_SOCKET_GET_SEND_BUFFER_SIZE_RAW, UDP_SOCKET_GET_SEND_BUFFER_SIZE, UDP_SOCKET_SET_SEND_BUFFER_SIZE_RAW, UDP_SOCKET_SET_SEND_BUFFER_SIZE, TCP_SOCKET_SEND_WRITER
        left out what `other` of `error-code` holds: a string, which an async call cannot give back yet
        left out `TCP_SOCKET_STREAM_CANCEL_READ`: a cancel blocks the scan
        left out `BYTES_STREAM_CANCEL_READ` and `BYTES_STREAM_CANCEL_WRITE`: a cancel blocks the scan
        left out `RESULT_FUTURE_CANCEL_READ`: a cancel blocks the scan
        left out `[method]udp-socket.receive`: its result holds a list, which an import cannot give back yet
    wasi-sockets-ip-name-lookup.st: ERROR_CODE_KIND, ERROR_CODE
        left out what `other` of `error-code` holds: a string, which an async call cannot give back yet
        left out `resolve-addresses`: its result holds a list, which an import cannot give back yet
    ");
}

/// What the interfaces `Std` reaches WASI through declare.
#[test]
fn std_s_interfaces() {
    assert_snapshot!(summary(&generated_std().files), @r"
    wasi-cli-types.st: ERROR_CODE
    wasi-cli-stdin.st: READ_VIA_STREAM, BYTES_STREAM_READ, BYTES_STREAM_READ_ARRAY, BYTES_STREAM_DROP_READABLE, RESULT_FUTURE_READ, RESULT_FUTURE_DROP_READABLE, RESULT_FUTURE_GET
        left out `BYTES_STREAM_CANCEL_READ`: a cancel blocks the scan
        left out `RESULT_FUTURE_CANCEL_READ`: a cancel blocks the scan
    wasi-cli-stdout.st: WRITE_VIA_STREAM, BYTES_STREAM_NEW, BYTES_STREAM_WRITE, BYTES_STREAM_WRITE_ARRAY, BYTES_STREAM_DROP_READABLE, BYTES_STREAM_DROP_WRITABLE, BYTES_STREAM_TEXT_LEN, BYTES_STREAM_TEXT_AFTER, BYTES_STREAM_WRITER, RESULT_FUTURE_READ, RESULT_FUTURE_DROP_READABLE, RESULT_FUTURE_GET, WRITE_VIA_STREAM_WRITER
        left out `BYTES_STREAM_CANCEL_WRITE`: a cancel blocks the scan
        left out `RESULT_FUTURE_CANCEL_READ`: a cancel blocks the scan
    wasi-cli-stderr.st: WRITE_VIA_STREAM, BYTES_STREAM_NEW, BYTES_STREAM_WRITE, BYTES_STREAM_WRITE_ARRAY, BYTES_STREAM_DROP_READABLE, BYTES_STREAM_DROP_WRITABLE, BYTES_STREAM_TEXT_LEN, BYTES_STREAM_TEXT_AFTER, BYTES_STREAM_WRITER, RESULT_FUTURE_READ, RESULT_FUTURE_DROP_READABLE, RESULT_FUTURE_GET, WRITE_VIA_STREAM_WRITER
        left out `BYTES_STREAM_CANCEL_WRITE`: a cancel blocks the scan
        left out `RESULT_FUTURE_CANCEL_READ`: a cancel blocks the scan
    wasi-clocks-types.st: DURATION
    wasi-clocks-monotonic-clock.st: MARK, NOW, GET_RESOLUTION, WAIT_UNTIL, WAIT_FOR
    wasi-clocks-system-clock.st: INSTANT, NOW_RAW, NOW, GET_RESOLUTION
    wasi-clocks-timezone.st: IANA_ID_RAW, IANA_ID, UTC_OFFSET_RAW, UTC_OFFSET, TO_DEBUG_STRING_RAW, TO_DEBUG_STRING
    ");
}

/// Std's interfaces build into a component, typed by Std's WIT folder
/// alone, which imports each of them.
#[rstest]
fn std_s_interfaces_build_into_a_component(mut with_db: RootDatabase) {
    let bindings = generated_std();
    let (mir, core) = built(&mut with_db, &bindings.files, &[]);
    assert_abi(&bindings, &mir, &core);
    let component = wrapped_with(&with_db, &mir, &core, &[stdlib_wit()]);
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
                .contains(&format!("  import {interface};\n")),
            "{interface} in {}",
            component.world
        );
    }
}

/// `stdlib/generated/` is what rk-bindgen writes for Std's WIT folder, file
/// for file, and holds nothing else: after a change to the generator or to
/// that folder, `rk bindgen`, run in `stdlib/`, writes it again. A checkout's line
/// endings do not count.
#[test]
fn std_s_generated_folder_is_what_rk_bindgen_writes() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("stdlib/generated");
    let files = generated_std().files;
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
    assert_eq!(
        on_disk, written,
        "run `cargo run -p rk -- bindgen --workspace stdlib`"
    );
    for file in &files {
        let text = std::fs::read_to_string(folder.join(&file.name))
            .expect("a generated file")
            .replace("\r\n", "\n");
        assert!(
            text == file.text,
            "`stdlib/generated/{}` is not what rk bindgen writes: run `cargo run -p rk -- bindgen --workspace stdlib`",
            file.name
        );
    }
}

/// A generated file says where it comes from, and that it is not edited.
#[test]
fn a_file_says_it_is_generated() {
    let bindings = generated_std();
    let header: Vec<&str> = bindings.files[0].text.lines().take(2).collect();
    assert_snapshot!(header.join("\n"), @r"
    /* Generated from the WIT of wasi:cli@0.3.0. DO NOT EDIT! */
    NAMESPACE INTERNAL Wasi
    ");
}
