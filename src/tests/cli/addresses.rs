// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `[addresses]` in `config.toml`: how a located variable's number names its
//! bytes (`numbering`) and which byte of a wider value comes first
//! (`byte-order`). Each size counting in its own units, little-endian, unless
//! the workspace says otherwise.

use db::config_file::{AddressesConfig, ByteOrder, Numbering, parse_config};
use rk::workspace::init_db;

use super::{disable_stdlib, temp_workspace};
use crate::tests::codegen::execute_wasm;

const PROJECT: &str = "[project]\nname = \"T\"\nversion = \"0.0\"\n";

/// The two settings, each its default when left out, and a value of
/// neither refused.
#[test]
fn the_settings_read_as_written_or_as_today() {
    let read = |section: &str| parse_config(&format!("{PROJECT}{section}")).map(|c| c.addresses());
    assert_eq!(read("").unwrap(), AddressesConfig::default());
    assert_eq!(
        read("").unwrap(),
        AddressesConfig {
            numbering: Numbering::Width,
            byte_order: ByteOrder::Little,
        }
    );
    assert_eq!(
        read("[addresses]\nnumbering = \"byte\"\nbyte-order = \"big\"\n").unwrap(),
        AddressesConfig {
            numbering: Numbering::Byte,
            byte_order: ByteOrder::Big,
        }
    );
    assert_eq!(
        read("[addresses]\nnumbering = \"byte\"\n").unwrap(),
        AddressesConfig {
            numbering: Numbering::Byte,
            byte_order: ByteOrder::Little,
        }
    );
    let refused = read("[addresses]\nnumbering = \"word\"\n").unwrap_err();
    assert!(refused.to_string().contains("width"), "{refused}");
    assert!(read("[addresses]\norder = \"big\"\n").is_err());
}

/// An exported FUNCTION that writes two addresses and reads back two
/// narrower ones, which are their parts when the workspace counts bytes:
/// `%MB209` is byte 209 of `%MW208`, `%MW101` bytes 101 and 102 of
/// `%MD100`.
const PROBE: &str = "
{export}
FUNCTION byte_of_word : BYTE
    %MW208 := WORD#16#1234;
    byte_of_word := %MB209;
END_FUNCTION

{export}
FUNCTION word_of_dword : WORD
    %MD100 := DWORD#16#11223344;
    word_of_dword := %MW101;
END_FUNCTION
";

/// The module of a workspace holding [`PROBE`] under `section`.
fn built(section: &str) -> Vec<u8> {
    let config = format!("{PROJECT}{section}");
    let (_ws, root) = temp_workspace(&[("config.toml", &config), ("main.st", PROBE)]);
    disable_stdlib();
    let db = init_db(&root, false, true).expect("init db");
    rk::compiler::build_core_quiet(&db, &root, false)
        .expect("it builds")
        .0
}

/// Counted by width, `%MB209` and `%MW101` are cells of their own, which
/// nothing wrote; counted by byte, they are inside what was written, its
/// bytes little-endian.
#[test]
fn counting_bytes_puts_a_narrower_address_inside_a_wider_one() {
    let by_width = built("");
    assert_eq!(execute_wasm::<(), i32>(&by_width, "byte_of_word", ()), 0);
    assert_eq!(execute_wasm::<(), i32>(&by_width, "word_of_dword", ()), 0);

    let by_byte = built("[addresses]\nnumbering = \"byte\"\n");
    assert_eq!(execute_wasm::<(), i32>(&by_byte, "byte_of_word", ()), 0x12);
    assert_eq!(
        execute_wasm::<(), i32>(&by_byte, "word_of_dword", ()),
        0x2233
    );
}
